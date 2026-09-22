use super::*;
use serde::Deserialize;

const FIXTURE: &str = include_str!(
    "../../../../../../docs/contracts/fixtures/forge-device-inventory-observation-v1.json"
);
const NOTICE: &str = "Every owner, instance, state, timestamp, resource, residency, trust, sandbox, and concurrency value is an unverified caller declaration. This read-only observation selects no target and grants no execution authority.";

#[derive(Deserialize)]
#[allow(clippy::struct_excessive_bools)]
#[serde(deny_unknown_fields)]
struct InventoryFixture {
    schema_version: String,
    evaluation_mode: String,
    evaluated_at_ms: u64,
    owner_declaration: OwnerFixture,
    owner_declaration_unverified: bool,
    inventory_declarations_unverified: bool,
    notice: String,
    devices: Vec<InventoryCandidateFixture>,
    execution_authorized: bool,
    reservation_created: bool,
    dispatch_performed: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerFixture {
    issuer: String,
    subject: String,
    tenant_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InventoryCandidateFixture {
    instance_id: String,
    device: DeviceFixture,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceFixture {
    device_id: String,
    owner: OwnerFixture,
    approval_state: String,
    cordon_state: String,
    liveness: String,
    snapshot_observed_at_ms: u64,
    lease_expires_at_ms: u64,
    os: String,
    architecture: String,
    available_cpu_cores: u32,
    available_memory_bytes: u64,
    available_storage_bytes: u64,
    runtimes: Vec<String>,
    gpu: GpuFixture,
    data_residency_zones: Vec<String>,
    trust_zone: String,
    sandbox_levels: Vec<String>,
    concurrency_limit: u16,
    active_concurrency: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GpuFixture {
    present: bool,
    memory_bytes: u64,
    runtime: String,
}

#[test]
fn inventory_observation_contract_is_read_only_and_maps_to_reference_model() {
    let fixture: InventoryFixture = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(
        fixture.schema_version,
        "forge.device-inventory-observation/v1"
    );
    assert_eq!(fixture.evaluation_mode, "offline_static_only");
    assert_eq!(fixture.evaluated_at_ms, 200_000);
    assert!(fixture.owner_declaration_unverified);
    assert!(fixture.inventory_declarations_unverified);
    assert_eq!(fixture.notice, NOTICE);
    assert!(!fixture.execution_authorized);
    assert!(!fixture.reservation_created);
    assert!(!fixture.dispatch_performed);
    assert_eq!(fixture.devices.len(), 2);
    assert_owner(&fixture.owner_declaration);
    assert_sorted_devices(&fixture.devices);

    let tenant_id = TenantId::parse(fixture.owner_declaration.tenant_id.clone()).unwrap();
    let candidates = fixture
        .devices
        .iter()
        .map(|candidate| build_candidate(candidate, &fixture.owner_declaration))
        .collect::<Vec<_>>();
    let request = DevicePlacementRequest::new(tenant_id, DevicePlacementRequirements::any());
    let decisions =
        dry_run_device_placement(&candidates, &request, fixture.evaluated_at_ms).unwrap();

    assert_eq!(decisions.len(), 2);
    assert_eq!(decisions[0].device_id().as_str(), "device-a");
    assert_eq!(decisions[0].instance_id().as_str(), "runner-a");
    assert_eq!(
        decisions[0].disposition(),
        &DevicePlacementDisposition::Eligible
    );
    assert_eq!(decisions[1].device_id().as_str(), "device-b");
    assert_eq!(decisions[1].instance_id().as_str(), "runner-b");
    assert_eq!(
        decisions[1].disposition(),
        &DevicePlacementDisposition::Excluded(vec![DevicePlacementExclusion::ApprovalPending])
    );
}

fn assert_owner(owner: &OwnerFixture) {
    assert_eq!(owner.issuer, "https://id.example");
    assert_eq!(owner.subject, "user-1");
    assert_eq!(owner.tenant_id, "tenant-1");
}

fn assert_sorted_devices(devices: &[InventoryCandidateFixture]) {
    for pair in devices.windows(2) {
        assert!(pair[0].device.device_id < pair[1].device.device_id);
        assert_ne!(pair[0].instance_id, pair[1].instance_id);
    }
}

fn build_candidate(
    fixture: &InventoryCandidateFixture,
    owner: &OwnerFixture,
) -> DevicePlacementCandidate {
    assert_owner(&fixture.device.owner);
    assert_eq!(fixture.device.owner.issuer, owner.issuer);
    assert_eq!(fixture.device.owner.subject, owner.subject);
    assert_eq!(fixture.device.owner.tenant_id, owner.tenant_id);
    let device_id = DeviceId::parse(fixture.device.device_id.clone()).unwrap();
    let tenant_id = TenantId::parse(fixture.device.owner.tenant_id.clone()).unwrap();
    let approval = approval_state(&fixture.device.approval_state);
    let cordoned = cordon_state(&fixture.device.cordon_state);
    let liveness = liveness_state(&fixture.device.liveness);
    assert!(!fixture.device.gpu.present);
    assert_eq!(fixture.device.gpu.memory_bytes, 0);
    assert!(fixture.device.gpu.runtime.is_empty());
    let capabilities = capabilities(&fixture.device);
    let instance_id = RunnerInstanceId::parse(fixture.instance_id.clone()).unwrap();
    let instance = RunnerInstance::restore(
        device_id.clone(),
        instance_id,
        1,
        1,
        fixture.device.snapshot_observed_at_ms,
        fixture.device.lease_expires_at_ms,
        liveness,
        capabilities,
    )
    .unwrap();
    let attributes = DevicePlacementAttributes::new()
        .with_data_residency_zones(fixture.device.data_residency_zones.clone())
        .unwrap()
        .with_trust_zone(&fixture.device.trust_zone)
        .unwrap()
        .with_sandbox_levels(&fixture.device.sandbox_levels)
        .unwrap()
        .with_concurrency(
            fixture.device.concurrency_limit,
            fixture.device.active_concurrency,
        );
    DevicePlacementCandidate::new(
        Device::restore(device_id, tenant_id, approval, cordoned),
        instance,
    )
    .unwrap()
    .with_attributes(attributes)
}

fn approval_state(value: &str) -> DeviceApprovalState {
    match value {
        "approved" => DeviceApprovalState::Approved,
        "pending" => DeviceApprovalState::Pending,
        "revoked" => DeviceApprovalState::Revoked,
        value => panic!("unsupported approval state {value:?}"),
    }
}

fn cordon_state(value: &str) -> bool {
    match value {
        "clear" => false,
        "cordoned" => true,
        value => panic!("unsupported cordon state {value:?}"),
    }
}

fn liveness_state(value: &str) -> RunnerLiveness {
    match value {
        "online" => RunnerLiveness::Online,
        "offline" => RunnerLiveness::Offline,
        value => panic!("unsupported liveness {value:?}"),
    }
}

fn capabilities(device: &DeviceFixture) -> CapabilitySnapshot {
    CapabilitySnapshot::new(
        device.os.clone(),
        device.architecture.clone(),
        device.available_cpu_cores,
        device.available_cpu_cores,
        device.available_memory_bytes,
        device.available_memory_bytes,
        device.available_storage_bytes,
        device.available_storage_bytes,
        Vec::new(),
        device.runtimes.clone(),
    )
    .unwrap()
}
