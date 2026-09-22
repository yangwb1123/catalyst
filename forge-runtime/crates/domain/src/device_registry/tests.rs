use super::*;

fn tenant(value: &str) -> TenantId {
    TenantId::parse(value).unwrap()
}

fn device(value: &str, tenant_id: &str) -> Device {
    Device::register(DeviceId::parse(value).unwrap(), tenant(tenant_id))
        .approve()
        .unwrap()
}

#[allow(clippy::too_many_arguments)]
fn capabilities(
    cpu_cores: u32,
    available_cpu_cores: u32,
    memory_bytes: u64,
    available_memory_bytes: u64,
    storage_bytes: u64,
    available_storage_bytes: u64,
    gpus: Vec<GpuCapability>,
    runtimes: &[&str],
) -> CapabilitySnapshot {
    CapabilitySnapshot::new(
        "Linux",
        "X86_64",
        cpu_cores,
        available_cpu_cores,
        memory_bytes,
        available_memory_bytes,
        storage_bytes,
        available_storage_bytes,
        gpus,
        runtimes
            .iter()
            .map(|runtime| (*runtime).to_owned())
            .collect(),
    )
    .unwrap()
}

fn gpu(id: &str, memory: u64, available: u64) -> GpuCapability {
    GpuCapability::new(id, "NVIDIA Corporation", memory, available).unwrap()
}

fn full_capabilities() -> CapabilitySnapshot {
    capabilities(
        8,
        8,
        32 * 1024 * 1024 * 1024,
        16 * 1024 * 1024 * 1024,
        100 * 1024 * 1024 * 1024,
        50 * 1024 * 1024 * 1024,
        vec![
            gpu("gpu-1", 16 * 1024 * 1024 * 1024, 14 * 1024 * 1024 * 1024),
            gpu("gpu-0", 16 * 1024 * 1024 * 1024, 14 * 1024 * 1024 * 1024),
        ],
        &["Docker", "python"],
    )
}

fn heartbeat(
    device_id: &DeviceId,
    instance_id: &str,
    generation: u64,
    sequence: u64,
    snapshot: CapabilitySnapshot,
) -> RunnerHeartbeat {
    RunnerHeartbeat::new(
        device_id.clone(),
        RunnerInstanceId::parse(instance_id).unwrap(),
        generation,
        sequence,
        snapshot,
    )
    .unwrap()
}

fn candidate(
    id: &str,
    tenant_id: &str,
    observed_at_ms: u64,
    lease_ttl_ms: u64,
    snapshot: CapabilitySnapshot,
    status: &str,
) -> DevicePlacementCandidate {
    let mut registered = Device::register(DeviceId::parse(id).unwrap(), tenant(tenant_id));
    if status != "pending" {
        registered = registered.approve().unwrap();
    }
    let beat = heartbeat(registered.id(), &format!("{id}-runner"), 1, 1, snapshot);
    let instance =
        apply_device_heartbeat(&registered, None, &beat, observed_at_ms, lease_ttl_ms).unwrap();
    if status == "revoked" {
        registered = registered.revoke();
    } else if status == "cordoned" {
        registered = registered.cordon();
    }
    let instance = if status == "offline" {
        instance.mark_offline()
    } else {
        instance
    };
    DevicePlacementCandidate::new(registered, instance).unwrap()
}

fn requirements() -> DevicePlacementRequirements {
    DevicePlacementRequirements::new(
        Some("linux"),
        Some("x86_64"),
        4,
        8 * 1024 * 1024 * 1024,
        1024 * 1024 * 1024,
        vec!["docker".to_owned()],
        2,
        12 * 1024 * 1024 * 1024,
    )
    .unwrap()
}

#[test]
fn identifiers_and_capabilities_are_bounded_and_canonicalized() {
    assert!(DeviceId::parse("not/a/device").is_err());
    assert!(DeviceId::parse("x".repeat(MAX_DEVICE_IDENTIFIER_BYTES + 1)).is_err());
    assert!(
        CapabilitySnapshot::new(
            "linux",
            "x86_64",
            1,
            1,
            MAX_DEVICE_CAPABILITY_BYTES + 1,
            1,
            0,
            0,
            vec![],
            vec![],
        )
        .is_err()
    );
    assert!(
        CapabilitySnapshot::new("linux", "x86_64", 1, 1, 10, 11, 0, 0, vec![], vec![],).is_err()
    );
    assert!(
        CapabilitySnapshot::new(
            "linux",
            "x86_64",
            1,
            1,
            10,
            10,
            0,
            0,
            vec![],
            vec!["Docker".to_owned(), "docker".to_owned()],
        )
        .is_err()
    );
    let snapshot = full_capabilities();
    assert_eq!(snapshot.operating_system(), "linux");
    assert_eq!(snapshot.architecture(), "x86_64");
    assert_eq!(snapshot.runtimes(), &["docker", "python"]);
    assert_eq!(snapshot.gpus()[0].id(), "gpu-0");
    assert_capability_collection_limits_are_enforced();
}

fn assert_capability_collection_limits_are_enforced() {
    let excessive_gpus = (0..=MAX_DEVICE_GPU_COUNT)
        .map(|index| gpu(&format!("gpu-{index}"), 8, 4))
        .collect();
    assert!(
        CapabilitySnapshot::new("linux", "x86_64", 1, 1, 8, 4, 0, 0, excessive_gpus, vec![])
            .is_err()
    );
    let excessive_runtimes = (0..=MAX_DEVICE_RUNTIME_COUNT)
        .map(|index| format!("runtime-{index}"))
        .collect();
    assert!(
        CapabilitySnapshot::new(
            "linux",
            "x86_64",
            MAX_DEVICE_CPU_CORES + 1,
            MAX_DEVICE_CPU_CORES + 1,
            8,
            4,
            0,
            0,
            vec![],
            excessive_runtimes,
        )
        .is_err()
    );
    assert!(
        DevicePlacementRequirements::new(None, None, MAX_DEVICE_CPU_CORES + 1, 0, 0, vec![], 0, 0,)
            .is_err()
    );
}

#[test]
fn heartbeat_rejects_old_incarnations_and_non_monotonic_sequences() {
    let registered = device("device-1", "tenant-a");
    let first = heartbeat(registered.id(), "runner-a", 1, 1, full_capabilities());
    let initial = apply_device_heartbeat(&registered, None, &first, 10_000, 20_000).unwrap();
    assert_eq!(initial.server_observed_at_ms(), 10_000);
    assert_eq!(initial.capability_lease_expires_at_ms(), 30_000);
    assert_eq!(initial.generation(), 1);
    assert_eq!(initial.heartbeat_sequence(), 1);
    assert_stale_heartbeats_rejected(&registered, &initial);
    assert_new_generation_advances_and_old_is_rejected(&registered, &initial, &first);
}

#[test]
fn heartbeat_lease_duration_and_expiry_are_bounded() {
    let registered = device("device-lease", "tenant-a");
    let beat = heartbeat(registered.id(), "runner-lease", 1, 1, full_capabilities());
    let non_initial = heartbeat(registered.id(), "runner-lease", 1, 2, full_capabilities());
    assert_eq!(
        apply_device_heartbeat(&registered, None, &non_initial, 10_000, 10_000),
        Err(DeviceHeartbeatError::SequenceMustStartAtOne)
    );
    let skipped_initial = heartbeat(registered.id(), "runner-lease", 2, 1, full_capabilities());
    assert_eq!(
        apply_device_heartbeat(&registered, None, &skipped_initial, 10_000, 10_000),
        Err(DeviceHeartbeatError::GenerationMustStartAtOne)
    );
    assert_eq!(
        apply_device_heartbeat(
            &registered,
            None,
            &beat,
            10_000,
            MIN_DEVICE_CAPABILITY_LEASE_TTL_MS - 1,
        ),
        Err(DeviceHeartbeatError::InvalidLeaseDuration)
    );
    assert_eq!(
        apply_device_heartbeat(
            &registered,
            None,
            &beat,
            10_000,
            MAX_DEVICE_CAPABILITY_LEASE_TTL_MS + 1,
        ),
        Err(DeviceHeartbeatError::InvalidLeaseDuration)
    );
    assert_eq!(
        apply_device_heartbeat(
            &registered,
            None,
            &beat,
            u64::MAX - 500,
            MIN_DEVICE_CAPABILITY_LEASE_TTL_MS,
        ),
        Err(DeviceHeartbeatError::LeaseExpiryOverflow)
    );
}

fn assert_stale_heartbeats_rejected(registered: &Device, initial: &RunnerInstance) {
    let duplicate = heartbeat(registered.id(), "runner-a", 1, 1, full_capabilities());
    assert_eq!(
        apply_device_heartbeat(registered, Some(initial), &duplicate, 11_000, 20_000),
        Err(DeviceHeartbeatError::SequenceNotIncreasing)
    );
    let replaced_id = heartbeat(registered.id(), "runner-b", 1, 2, full_capabilities());
    assert_eq!(
        apply_device_heartbeat(registered, Some(initial), &replaced_id, 11_000, 20_000),
        Err(DeviceHeartbeatError::InstanceChangedWithinGeneration)
    );
    let skipped_generation = heartbeat(registered.id(), "runner-c", 3, 1, full_capabilities());
    assert_eq!(
        apply_device_heartbeat(
            registered,
            Some(initial),
            &skipped_generation,
            11_000,
            20_000
        ),
        Err(DeviceHeartbeatError::GenerationSkipped)
    );
}

fn assert_new_generation_advances_and_old_is_rejected(
    registered: &Device,
    initial: &RunnerInstance,
    first: &RunnerHeartbeat,
) {
    let next_same_instance = heartbeat(registered.id(), "runner-a", 1, 2, full_capabilities());
    assert_eq!(
        apply_device_heartbeat(
            registered,
            Some(initial),
            &next_same_instance,
            11_000,
            20_000,
        )
        .unwrap()
        .heartbeat_sequence(),
        2
    );
    let reboot = heartbeat(registered.id(), "runner-b", 2, 1, full_capabilities());
    let next = apply_device_heartbeat(registered, Some(initial), &reboot, 11_000, 20_000).unwrap();
    assert_eq!(next.generation(), 2);
    assert_eq!(next.heartbeat_sequence(), 1);
    assert_eq!(
        initial.heartbeat_sequence(),
        1,
        "transition leaves its input unchanged"
    );
    assert_eq!(
        apply_device_heartbeat(registered, Some(&next), first, 12_000, 20_000),
        Err(DeviceHeartbeatError::OldGeneration)
    );
    let newer_sequence = heartbeat(registered.id(), "runner-b", 2, 2, full_capabilities());
    assert_eq!(
        apply_device_heartbeat(registered, Some(&next), &newer_sequence, 10_999, 20_000),
        Err(DeviceHeartbeatError::ServerTimeWentBackwards)
    );
}

#[test]
fn restored_registry_rows_are_revalidated() {
    let registered = device("device-restore", "tenant-a");
    let beat = heartbeat(registered.id(), "runner-restore", 1, 1, full_capabilities());
    let instance = apply_device_heartbeat(&registered, None, &beat, 5_000, 10_000).unwrap();
    assert_eq!(
        RunnerInstance::restore(
            instance.device_id().clone(),
            instance.instance_id().clone(),
            0,
            1,
            5_000,
            15_000,
            RunnerLiveness::Online,
            instance.capabilities().clone(),
        ),
        Err(DeviceRegistryValidationError::InvalidPersistedState)
    );
    let restored_revoked = Device::restore(
        registered.id().clone(),
        registered.tenant_id().clone(),
        DeviceApprovalState::Revoked,
        false,
    );
    assert_eq!(restored_revoked.approval(), DeviceApprovalState::Revoked);
    assert!(!restored_revoked.is_cordoned());
    assert!(
        RunnerInstance::restore(
            instance.device_id().clone(),
            instance.instance_id().clone(),
            instance.generation(),
            instance.heartbeat_sequence(),
            instance.server_observed_at_ms(),
            instance.capability_lease_expires_at_ms(),
            instance.liveness(),
            instance.capabilities().clone(),
        )
        .is_ok()
    );
}

#[path = "tests/placement.rs"]
mod placement;

#[path = "tests/placement_parity.rs"]
mod placement_parity;

#[path = "tests/run_intent_observation.rs"]
mod run_intent_observation;

#[path = "tests/resource_summary_contract.rs"]
mod resource_summary_contract;

#[path = "tests/inventory_contract.rs"]
mod inventory_contract;

#[path = "tests/persistence_contract.rs"]
mod persistence_contract;

#[path = "tests/persisted_inventory_contract.rs"]
mod persisted_inventory_contract;

#[path = "tests/persisted_inventory_placement_input_contract.rs"]
mod persisted_inventory_placement_input_contract;

#[path = "tests/persisted_inventory_placement_batch_contract.rs"]
mod persisted_inventory_placement_batch_contract;

#[path = "tests/status_contract.rs"]
mod status_contract;

#[path = "tests/snapshot_contract.rs"]
mod snapshot_contract;

#[path = "tests/heartbeat_contract.rs"]
mod heartbeat_contract;

#[path = "tests/identity_contract.rs"]
mod identity_contract;

#[path = "tests/enrollment_heartbeat_lifecycle_contract.rs"]
mod enrollment_heartbeat_lifecycle_contract;

#[path = "tests/persisted_inventory_observation.rs"]
mod persisted_inventory_observation;
#[path = "tests/persisted_inventory_observation_v2.rs"]
mod persisted_inventory_observation_v2;
#[path = "tests/persisted_placement_v2.rs"]
mod persisted_placement_v2;

#[path = "tests/enrollment_heartbeat_lifecycle_persistence_contract.rs"]
mod enrollment_heartbeat_lifecycle_persistence_contract;
