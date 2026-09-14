use super::*;
use crate::{
    CapabilitySnapshot, DeviceApprovalState, DeviceId, DevicePlacementDisposition,
    DevicePlacementRequirements, RunnerInstanceId, RunnerLiveness, TenantId,
};

fn tenant() -> TenantId {
    TenantId::parse("tenant-a").unwrap()
}

fn snapshot() -> CapabilitySnapshot {
    CapabilitySnapshot::new(
        "linux",
        "x86_64",
        4,
        3,
        4096,
        3072,
        2048,
        1024,
        vec![],
        vec!["docker".to_owned()],
    )
    .unwrap()
}

fn registered_device(id: &str) -> Device {
    Device::register(DeviceId::parse(id).unwrap(), tenant())
}

fn heartbeat(device: &Device, seq: u64) -> RunnerHeartbeat {
    RunnerHeartbeat::new(
        device.id().clone(),
        RunnerInstanceId::parse(format!("runner-{}", device.id())).unwrap(),
        1,
        seq,
        snapshot(),
    )
    .unwrap()
}

#[test]
fn service_uses_server_clock_and_bounded_default_lease() {
    let service = DeviceRegistryService::new();
    let device = registered_device("device-1");
    let beat = heartbeat(&device, 1);
    let instance = service.heartbeat(&device, None, &beat, 50_000).unwrap();
    assert_eq!(instance.server_observed_at_ms(), 50_000);
    assert_eq!(
        instance.capability_lease_expires_at_ms(),
        50_000 + DEVICE_CAPABILITY_LEASE_TTL_MS
    );
    assert_eq!(instance.liveness(), RunnerLiveness::Online);
    assert_eq!(
        service.heartbeat(&device, Some(&instance), &beat, 51_000),
        Err(DeviceHeartbeatError::SequenceNotIncreasing)
    );
    assert_eq!(instance.heartbeat_sequence(), 1);
}

#[test]
fn service_device_transitions_keep_revocation_terminal() {
    let service = DeviceRegistryService::new();
    let pending = registered_device("device-2");
    let approved = service.approve(&pending).unwrap();
    assert_eq!(approved.approval(), DeviceApprovalState::Approved);
    assert!(service.approve(&approved).is_err());
    let cordoned = service.cordon(&approved);
    assert!(cordoned.is_cordoned());
    assert!(!service.uncordon(&cordoned).is_cordoned());
    let revoked = service.revoke(&approved);
    assert_eq!(revoked.approval(), DeviceApprovalState::Revoked);
    assert!(!revoked.is_cordoned());
    assert_eq!(
        service.uncordon(&revoked).approval(),
        DeviceApprovalState::Revoked
    );
}

#[test]
fn service_dry_run_returns_a_repeatable_sorted_view_only() {
    let service = DeviceRegistryService::new();
    let first_device = service.approve(&registered_device("device-z")).unwrap();
    let second_device = service.approve(&registered_device("device-a")).unwrap();
    let first_instance = service
        .heartbeat(&first_device, None, &heartbeat(&first_device, 1), 10_000)
        .unwrap();
    let second_instance = service
        .heartbeat(&second_device, None, &heartbeat(&second_device, 1), 10_000)
        .unwrap();
    let candidates = vec![
        DevicePlacementCandidate::new(first_device, first_instance).unwrap(),
        DevicePlacementCandidate::new(second_device, second_instance).unwrap(),
    ];
    let requirements = DevicePlacementRequirements::new(
        Some("linux"),
        Some("x86_64"),
        2,
        2048,
        512,
        vec!["docker".to_owned()],
        0,
        0,
    )
    .unwrap();
    let request = DevicePlacementRequest::new(tenant(), requirements);
    let first = service
        .dry_run_placement(&candidates, &request, 20_000)
        .unwrap();
    let second = service
        .dry_run_placement(&candidates, &request, 20_000)
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(first[0].device_id().as_str(), "device-a");
    assert!(
        first
            .iter()
            .all(|decision| decision.disposition() == &DevicePlacementDisposition::Eligible)
    );
}
