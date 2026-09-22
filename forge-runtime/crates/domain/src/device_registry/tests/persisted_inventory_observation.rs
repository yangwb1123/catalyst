use super::*;
use serde_json::Value;

fn make_owner(subject: &str) -> SnapshotOwner {
    SnapshotOwner {
        issuer: "issuer".to_owned(),
        subject: subject.to_owned(),
        tenant_id: "tenant".to_owned(),
    }
}

fn state(
    owner: &SnapshotOwner,
    device_id: &str,
    instance_id: &str,
    approval: DeviceApprovalState,
    cordoned: bool,
    liveness: RunnerLiveness,
    observed_at_ms: u64,
    lease_expires_at_ms: u64,
) -> PersistedInventoryState {
    let device = Device::restore(
        DeviceId::parse(device_id).unwrap(),
        TenantId::parse(owner.tenant_id.clone()).unwrap(),
        approval,
        cordoned,
    );
    let persisted_device = PersistedInventoryDevice::restore(device, owner.clone(), false);
    let capabilities = CapabilitySnapshot::new(
        "Linux",
        "AMD64",
        8,
        7,
        16 * 1024,
        8 * 1024,
        100 * 1024,
        50 * 1024,
        Vec::new(),
        vec!["Go".to_owned(), "Rust".to_owned()],
    )
    .unwrap();
    let runner = RunnerInstance::restore(
        DeviceId::parse(device_id).unwrap(),
        RunnerInstanceId::parse(instance_id).unwrap(),
        1,
        1,
        observed_at_ms,
        lease_expires_at_ms,
        liveness,
        capabilities,
    )
    .unwrap();
    restore_persisted_inventory(1, persisted_device, runner).unwrap()
}

#[test]
fn persisted_inventory_observation_is_owner_bound_sorted_and_read_only() {
    let owner = make_owner("user");
    let first = state(
        &owner,
        "device-b",
        "runner-b",
        DeviceApprovalState::Approved,
        false,
        RunnerLiveness::Online,
        100_000,
        200_000,
    );
    let second = state(
        &owner,
        "device-a",
        "runner-a",
        DeviceApprovalState::Pending,
        true,
        RunnerLiveness::Offline,
        100_000,
        200_000,
    );
    let value =
        build_persisted_inventory_observation(&[first.clone(), second.clone()], &owner, 200_000)
            .unwrap();

    assert_eq!(
        value.schema_version,
        PERSISTED_INVENTORY_OBSERVATION_SCHEMA_VERSION
    );
    assert_eq!(
        value.evaluation_mode,
        PERSISTED_INVENTORY_OBSERVATION_EVALUATION_MODE
    );
    assert_eq!(value.evaluated_at_ms, 200_000);
    assert_eq!(
        value.owner_declaration,
        PersistedInventoryObservationOwner::from(&owner)
    );
    assert!(value.owner_declaration_unverified);
    assert!(value.inventory_declarations_unverified);
    assert_eq!(value.notice, PERSISTED_INVENTORY_OBSERVATION_NOTICE);
    assert_eq!(value.devices.len(), 2);
    assert_eq!(value.devices[0].device.device_id, "device-a");
    assert_eq!(value.devices[0].instance_id, "runner-a");
    assert_eq!(value.devices[0].device.approval_state, "pending");
    assert_eq!(value.devices[0].device.cordon_state, "cordoned");
    assert_eq!(value.devices[0].device.liveness, "offline");
    assert_eq!(value.devices[0].device.runtimes, ["go", "rust"]);
    assert_eq!(
        value.devices[0].device.gpu,
        PersistedInventoryObservationGpu {
            present: false,
            memory_bytes: 0,
            runtime: String::new(),
        }
    );
    assert!(value.devices[0].device.data_residency_zones.is_empty());
    assert_eq!(value.devices[0].device.trust_zone, "unknown");
    assert!(value.devices[0].device.sandbox_levels.is_empty());
    assert_eq!(value.devices[0].device.concurrency_limit, 0);
    assert_eq!(value.devices[0].device.active_concurrency, 0);
    assert!(!value.execution_authorized);
    assert!(!value.reservation_created);
    assert!(!value.dispatch_performed);

    let encoded = serde_json::to_value(&value).unwrap();
    assert_eq!(
        encoded["schema_version"],
        "forge.device-inventory-observation/v1"
    );
    assert_eq!(encoded["devices"][0]["device"]["device_id"], "device-a");
    assert_eq!(encoded["devices"][1]["device"]["device_id"], "device-b");
    assert!(encoded["execution_authorized"] == Value::Bool(false));
}

#[test]
fn persisted_inventory_observation_rejects_unsafe_or_lossy_values() {
    let owner = make_owner("user");
    let base = state(
        &owner,
        "device-a",
        "runner-a",
        DeviceApprovalState::Approved,
        false,
        RunnerLiveness::Online,
        100_000,
        200_000,
    );
    assert_eq!(
        build_persisted_inventory_observation(&[base.clone()], &make_owner("other"), 200_000),
        Err(PersistedInventoryObservationError::Persistence(
            PersistenceError::InventoryOwnerMismatch,
        ))
    );
    assert_eq!(
        build_persisted_inventory_observation(&[base.clone()], &owner, 0),
        Err(PersistedInventoryObservationError::InvalidObservation)
    );
    assert_eq!(
        build_persisted_inventory_observation(
            &[base.clone()],
            &owner,
            MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER + 1,
        ),
        Err(PersistedInventoryObservationError::InvalidObservation)
    );

    let mut future = base.clone();
    future = restore_persisted_inventory(
        future.revision(),
        future.device().clone(),
        RunnerInstance::restore(
            future.runner().device_id().clone(),
            future.runner().instance_id().clone(),
            future.runner().generation(),
            future.runner().heartbeat_sequence(),
            300_000,
            400_000,
            future.runner().liveness(),
            future.runner().capabilities().clone(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        build_persisted_inventory_observation(&[future], &owner, 200_000),
        Err(PersistedInventoryObservationError::Persistence(
            PersistenceError::InventoryStatus(InventoryStatusError::SnapshotFromFuture),
        ))
    );

    let duplicate_device = state(
        &owner,
        "device-a",
        "runner-b",
        DeviceApprovalState::Approved,
        false,
        RunnerLiveness::Online,
        100_000,
        200_000,
    );
    assert_eq!(
        build_persisted_inventory_observation(&[base.clone(), duplicate_device], &owner, 200_000),
        Err(PersistedInventoryObservationError::DuplicateDevice)
    );
    let duplicate_instance = state(
        &owner,
        "device-b",
        "runner-a",
        DeviceApprovalState::Approved,
        false,
        RunnerLiveness::Online,
        100_000,
        200_000,
    );
    assert_eq!(
        build_persisted_inventory_observation(&[base.clone(), duplicate_instance], &owner, 200_000),
        Err(PersistedInventoryObservationError::DuplicateInstance)
    );

    let reserved_device =
        PersistedInventoryDevice::restore(base.device().device().clone(), owner.clone(), true);
    let reserved =
        restore_persisted_inventory(base.revision(), reserved_device, base.runner().clone())
            .unwrap();
    assert_eq!(
        build_persisted_inventory_observation(&[reserved], &owner, 200_000),
        Err(PersistedInventoryObservationError::UnsupportedObservation)
    );

    let gpu = GpuCapability::new("gpu-a", "vendor", 8, 4).unwrap();
    let capabilities = CapabilitySnapshot::new(
        "linux",
        "amd64",
        8,
        7,
        16 * 1024,
        8 * 1024,
        100 * 1024,
        50 * 1024,
        vec![gpu],
        vec!["go".to_owned()],
    )
    .unwrap();
    let gpu_runner = RunnerInstance::restore(
        base.runner().device_id().clone(),
        base.runner().instance_id().clone(),
        1,
        1,
        100_000,
        200_000,
        RunnerLiveness::Online,
        capabilities,
    )
    .unwrap();
    let gpu_state =
        restore_persisted_inventory(base.revision(), base.device().clone(), gpu_runner).unwrap();
    assert_eq!(
        build_persisted_inventory_observation(&[gpu_state], &owner, 200_000),
        Err(PersistedInventoryObservationError::UnsupportedObservation)
    );

    for (name, memory, storage) in [
        (
            "unsafe available memory",
            Some(MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER + 1),
            None,
        ),
        (
            "unsafe available storage",
            None,
            Some(MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER + 1),
        ),
    ] {
        let capabilities = CapabilitySnapshot::new(
            "Linux",
            "AMD64",
            8,
            7,
            memory.unwrap_or(16 * 1024),
            memory.unwrap_or(8 * 1024),
            storage.unwrap_or(100 * 1024),
            storage.unwrap_or(50 * 1024),
            Vec::new(),
            vec!["Go".to_owned()],
        )
        .unwrap();
        let runner = RunnerInstance::restore(
            base.runner().device_id().clone(),
            base.runner().instance_id().clone(),
            1,
            1,
            100_000,
            200_000,
            RunnerLiveness::Online,
            capabilities,
        )
        .unwrap();
        let value =
            restore_persisted_inventory(base.revision(), base.device().clone(), runner).unwrap();
        assert_eq!(
            build_persisted_inventory_observation(&[value], &owner, 200_000),
            Err(PersistedInventoryObservationError::InvalidObservation),
            "{name}"
        );
    }
}
