use super::*;

const V2_FIXTURE: &str = include_str!(
    "../../../../../../docs/contracts/fixtures/forge-device-inventory-observation-v2.json"
);

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
        tenant("tenant"),
        approval,
        cordoned,
    );
    let persisted_device = PersistedInventoryDevice::restore(device, owner.clone(), false);
    let runner = RunnerInstance::restore(
        DeviceId::parse(device_id).unwrap(),
        RunnerInstanceId::parse(instance_id).unwrap(),
        1,
        1,
        observed_at_ms,
        lease_expires_at_ms,
        liveness,
        capabilities(
            8,
            7,
            16 * 1024,
            8 * 1024,
            100 * 1024,
            50 * 1024,
            Vec::new(),
            &["Go", "Rust"],
        ),
    )
    .unwrap();
    restore_persisted_inventory(1, persisted_device, runner).unwrap()
}

#[test]
fn persisted_inventory_observation_v2_retains_reservation_and_gpus() {
    let owner = make_owner("user");
    let device = Device::register(DeviceId::parse("device-a").unwrap(), tenant("tenant"))
        .approve()
        .unwrap();
    let persisted_device = PersistedInventoryDevice::restore(device, owner.clone(), true);
    let capabilities = capabilities(
        8,
        7,
        16 * 1024 * 1024 * 1024,
        8 * 1024 * 1024 * 1024,
        100 * 1024 * 1024 * 1024,
        50 * 1024 * 1024 * 1024,
        vec![
            gpu("gpu-b", 8 * 1024 * 1024 * 1024, 4 * 1024 * 1024 * 1024),
            gpu("gpu-a", 16 * 1024 * 1024 * 1024, 12 * 1024 * 1024 * 1024),
        ],
        &["Go", "Rust"],
    );
    let runner = RunnerInstance::restore(
        DeviceId::parse("device-a").unwrap(),
        RunnerInstanceId::parse("runner-a").unwrap(),
        1,
        1,
        100_000,
        200_000,
        RunnerLiveness::Online,
        capabilities,
    )
    .unwrap();
    let value = restore_persisted_inventory(1, persisted_device, runner).unwrap();
    let actual = build_persisted_inventory_observation_v2(&[value], &owner, 200_000).unwrap();
    assert_eq!(
        actual.schema_version,
        PERSISTED_INVENTORY_OBSERVATION_V2_SCHEMA_VERSION
    );
    let device = &actual.devices[0].device;
    assert_eq!(actual.devices[0].revision, 1);
    assert_eq!(actual.devices[0].generation, 1);
    assert_eq!(actual.devices[0].heartbeat_sequence, 1);
    assert_eq!(device.reservation_state, "reserved");
    assert_eq!(device.gpus.len(), 2);
    assert_eq!(device.gpus[0].id, "gpu-a");
    assert_eq!(device.gpus[1].id, "gpu-b");
    assert_eq!(
        device.gpus[0].available_memory_bytes,
        12 * 1024 * 1024 * 1024
    );
    assert!(!actual.execution_authorized);
    assert!(!actual.reservation_created);
    assert!(!actual.dispatch_performed);
}

#[test]
fn persisted_inventory_observation_v2_preserves_owner_and_rejects_foreign_values() {
    let owner = make_owner("user");
    let value = state(
        &owner,
        "device-a",
        "runner-a",
        DeviceApprovalState::Approved,
        false,
        RunnerLiveness::Online,
        100_000,
        200_000,
    );
    let encoded = serde_json::to_value(
        build_persisted_inventory_observation_v2(&[value.clone()], &owner, 200_000).unwrap(),
    )
    .unwrap();
    assert_eq!(encoded["devices"][0]["device"]["reservation_state"], "none");
    assert_eq!(
        encoded["devices"][0]["device"]["gpus"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        build_persisted_inventory_observation_v2(&[value], &make_owner("other"), 200_000),
        Err(PersistedInventoryObservationV2Error::Persistence(
            PersistenceError::InventoryOwnerMismatch,
        ))
    );
}

#[test]
fn persisted_inventory_observation_v2_matches_shared_fixture() {
    let owner = make_owner("user");
    let first_device = Device::register(DeviceId::parse("device-a").unwrap(), tenant("tenant"))
        .approve()
        .unwrap();
    let first_persisted = PersistedInventoryDevice::restore(first_device, owner.clone(), true);
    let first_capabilities = CapabilitySnapshot::new(
        "Linux",
        "AMD64",
        8,
        7,
        16 * 1024 * 1024 * 1024,
        8 * 1024 * 1024 * 1024,
        100 * 1024 * 1024 * 1024,
        50 * 1024 * 1024 * 1024,
        vec![
            GpuCapability::new(
                "gpu-b",
                "NVIDIA",
                8 * 1024 * 1024 * 1024,
                4 * 1024 * 1024 * 1024,
            )
            .unwrap(),
            GpuCapability::new(
                "gpu-a",
                "NVIDIA",
                16 * 1024 * 1024 * 1024,
                12 * 1024 * 1024 * 1024,
            )
            .unwrap(),
        ],
        vec!["Go".to_owned(), "Rust".to_owned()],
    )
    .unwrap();
    let first_runner = RunnerInstance::restore(
        DeviceId::parse("device-a").unwrap(),
        RunnerInstanceId::parse("runner-a").unwrap(),
        1,
        1,
        100_000,
        200_000,
        RunnerLiveness::Online,
        first_capabilities,
    )
    .unwrap();
    let first = restore_persisted_inventory(1, first_persisted, first_runner).unwrap();

    let second_device =
        Device::register(DeviceId::parse("device-b").unwrap(), tenant("tenant")).cordon();
    let second_persisted = PersistedInventoryDevice::restore(second_device, owner.clone(), false);
    let second_runner = RunnerInstance::restore(
        DeviceId::parse("device-b").unwrap(),
        RunnerInstanceId::parse("runner-b").unwrap(),
        2,
        4,
        100_000,
        200_000,
        RunnerLiveness::Offline,
        CapabilitySnapshot::new(
            "Linux",
            "AMD64",
            8,
            7,
            16 * 1024 * 1024 * 1024,
            8 * 1024 * 1024 * 1024,
            100 * 1024 * 1024 * 1024,
            50 * 1024 * 1024 * 1024,
            Vec::new(),
            vec!["Go".to_owned(), "Rust".to_owned()],
        )
        .unwrap(),
    )
    .unwrap();
    let second = restore_persisted_inventory(2, second_persisted, second_runner).unwrap();
    let actual =
        build_persisted_inventory_observation_v2(&[second, first], &owner, 200_000).unwrap();
    let expected: serde_json::Value = serde_json::from_str(V2_FIXTURE).unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
