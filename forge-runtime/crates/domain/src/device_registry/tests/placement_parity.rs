use super::*;
use serde::Deserialize;

const FIXTURE: &str = include_str!(
    "../../../../../../docs/contracts/fixtures/forge-device-placement-policy-parity-v1.json"
);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    evaluated_at_ms: u64,
    max_snapshot_age_ms: u64,
    owner: OwnerFixture,
    requirements: RequirementsFixture,
    candidates: Vec<CandidateFixture>,
    expected: Vec<ExpectedFixture>,
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
struct RequirementsFixture {
    os: String,
    architecture: String,
    min_cpu_cores: u32,
    min_memory_bytes: u64,
    min_storage_bytes: u64,
    runtime: String,
    gpu: GpuRequirementFixture,
    data_residency_zones: Vec<String>,
    minimum_trust_zone: String,
    sandbox_floor: String,
    concurrency_slots: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GpuRequirementFixture {
    required: bool,
    min_memory_bytes: u64,
    runtime: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateFixture {
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
    gpu: GpuDeclarationFixture,
    data_residency_zones: Vec<String>,
    trust_zone: String,
    sandbox_levels: Vec<String>,
    concurrency_limit: u16,
    active_concurrency: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GpuDeclarationFixture {
    present: bool,
    memory_bytes: u64,
    runtime: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedFixture {
    device_id: String,
    matches_requirements: bool,
    exclusion_reasons: Vec<String>,
}

#[test]
fn shared_policy_fixture_matches_offline_go_projection() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(
        fixture.schema_version,
        "forge.device-placement-policy-parity-test/v1"
    );
    assert!(fixture.max_snapshot_age_ms <= DEVICE_HEARTBEAT_STALE_AFTER_MS);
    assert!(
        !fixture.requirements.gpu.required
            && fixture.requirements.gpu.min_memory_bytes == 0
            && fixture.requirements.gpu.runtime.is_empty()
    );
    let request = DevicePlacementRequest::new(
        TenantId::parse(fixture.owner.tenant_id.clone()).unwrap(),
        build_requirements(&fixture.requirements),
    );
    let candidates = fixture
        .candidates
        .iter()
        .map(|candidate| build_candidate(candidate, &fixture.owner, &fixture.requirements))
        .collect::<Vec<_>>();
    let decisions =
        dry_run_device_placement(&candidates, &request, fixture.evaluated_at_ms).unwrap();

    assert_eq!(decisions.len(), fixture.expected.len());
    for (decision, expected) in decisions.iter().zip(&fixture.expected) {
        assert_eq!(decision.device_id().as_str(), expected.device_id);
        let (matches_requirements, reasons) = match decision.disposition() {
            DevicePlacementDisposition::Eligible => (true, Vec::new()),
            DevicePlacementDisposition::Excluded(reasons) => (
                false,
                reasons
                    .iter()
                    .map(|reason| common_reason_code(*reason))
                    .collect::<Vec<_>>(),
            ),
        };
        assert_eq!(matches_requirements, expected.matches_requirements);
        assert_eq!(reasons, expected.exclusion_reasons);
    }
}

fn build_requirements(fixture: &RequirementsFixture) -> DevicePlacementRequirements {
    let policy = DevicePlacementPolicy::new()
        .with_allowed_data_residency_zones(fixture.data_residency_zones.clone())
        .unwrap()
        .with_minimum_trust_zone(&fixture.minimum_trust_zone)
        .unwrap()
        .with_sandbox_floor(&fixture.sandbox_floor)
        .unwrap()
        .with_concurrency_slots(fixture.concurrency_slots)
        .unwrap();
    DevicePlacementRequirements::new(
        Some(&fixture.os),
        Some(&fixture.architecture),
        fixture.min_cpu_cores,
        fixture.min_memory_bytes,
        fixture.min_storage_bytes,
        vec![fixture.runtime.clone()],
        0,
        0,
    )
    .unwrap()
    .with_policy(policy)
}

fn build_candidate(
    fixture: &CandidateFixture,
    owner: &OwnerFixture,
    requirements: &RequirementsFixture,
) -> DevicePlacementCandidate {
    assert_common_candidate_fields(fixture, owner);
    let device_id = DeviceId::parse(fixture.device.device_id.clone()).unwrap();
    let tenant_id = TenantId::parse(fixture.device.owner.tenant_id.clone()).unwrap();
    let device = Device::register(device_id.clone(), tenant_id)
        .approve()
        .unwrap();
    let instance = RunnerInstance::restore(
        device_id,
        RunnerInstanceId::parse(fixture.instance_id.clone()).unwrap(),
        1,
        1,
        fixture.device.snapshot_observed_at_ms,
        fixture.device.lease_expires_at_ms,
        RunnerLiveness::Online,
        build_capabilities(&fixture.device, requirements),
    )
    .unwrap();
    DevicePlacementCandidate::new(device, instance)
        .unwrap()
        .with_attributes(build_attributes(&fixture.device))
}

fn assert_common_candidate_fields(fixture: &CandidateFixture, owner: &OwnerFixture) {
    assert_eq!(fixture.device.approval_state, "approved");
    assert_eq!(fixture.device.cordon_state, "clear");
    assert_eq!(fixture.device.liveness, "online");
    assert!(!fixture.device.gpu.present);
    assert_eq!(fixture.device.gpu.memory_bytes, 0);
    assert!(fixture.device.gpu.runtime.is_empty());
    assert_eq!(fixture.device.owner.tenant_id, owner.tenant_id);
    assert_eq!(fixture.device.owner.issuer, owner.issuer);
    assert_eq!(fixture.device.owner.subject, owner.subject);
}

fn build_capabilities(
    fixture: &DeviceFixture,
    requirements: &RequirementsFixture,
) -> CapabilitySnapshot {
    CapabilitySnapshot::new(
        fixture.os.clone(),
        fixture.architecture.clone(),
        fixture.available_cpu_cores.max(requirements.min_cpu_cores),
        fixture.available_cpu_cores,
        fixture
            .available_memory_bytes
            .max(requirements.min_memory_bytes),
        fixture.available_memory_bytes,
        fixture
            .available_storage_bytes
            .max(requirements.min_storage_bytes),
        fixture.available_storage_bytes,
        Vec::new(),
        fixture.runtimes.clone(),
    )
    .unwrap()
}

fn build_attributes(fixture: &DeviceFixture) -> DevicePlacementAttributes {
    DevicePlacementAttributes::new()
        .with_data_residency_zones(fixture.data_residency_zones.clone())
        .unwrap()
        .with_trust_zone(&fixture.trust_zone)
        .unwrap()
        .with_sandbox_levels(&fixture.sandbox_levels)
        .unwrap()
        .with_concurrency(fixture.concurrency_limit, fixture.active_concurrency)
}

fn common_reason_code(reason: DevicePlacementExclusion) -> String {
    match reason {
        DevicePlacementExclusion::MemoryCapacityInsufficient => "memory_insufficient".to_owned(),
        DevicePlacementExclusion::StorageCapacityInsufficient => "storage_insufficient".to_owned(),
        reason => reason.as_str().to_owned(),
    }
}
