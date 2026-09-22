use super::*;
use serde::Deserialize;

const INPUT_FIXTURE: &str = include_str!(
    "../../../../../../docs/contracts/fixtures/forge-device-inventory-placement-input-v1.json"
);
const BATCH_FIXTURE: &str = include_str!(
    "../../../../../../docs/contracts/fixtures/forge-device-inventory-placement-batch-evaluation-v1.json"
);

#[allow(dead_code)]
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceFixture {
    schema_version: String,
    evaluation_mode: String,
    evaluation_owner: OwnerFixture,
    policy_requirements: serde_json::Value,
    authority: serde_json::Value,
    state: StateFixture,
    cases: Vec<SourceCase>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerFixture {
    issuer: String,
    subject: String,
    tenant_id: String,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct StateFixture {
    revision: u64,
    device: DeviceFixture,
    runner: RunnerFixture,
}

#[allow(dead_code)]
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceFixture {
    device_id: String,
    owner: OwnerFixture,
    approval_state: String,
    cordon_state: String,
    reservation_state: String,
}

#[allow(dead_code)]
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunnerFixture {
    device_id: String,
    instance_id: String,
    generation: u64,
    heartbeat_sequence: u64,
    server_observed_at_ms: u64,
    capability_lease_expires_at_ms: u64,
    liveness: String,
    capabilities: CapabilityFixture,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CapabilityFixture {
    os: String,
    architecture: String,
    cpu_cores: u32,
    available_cpu_cores: u32,
    memory_bytes: u64,
    available_memory_bytes: u64,
    storage_bytes: u64,
    available_storage_bytes: u64,
    gpus: Vec<GpuFixture>,
    runtimes: Vec<String>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct GpuFixture {
    id: String,
    vendor: String,
    memory_bytes: u64,
    available_memory_bytes: u64,
}

#[allow(dead_code)]
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceCase {
    name: String,
    #[serde(default)]
    evaluation_owner: Option<OwnerFixture>,
    #[serde(default)]
    runner_device_id: Option<String>,
    #[serde(default)]
    approval_state: Option<String>,
    #[serde(default)]
    cordon_state: Option<String>,
    #[serde(default)]
    liveness: Option<String>,
    #[serde(default)]
    server_observed_at_ms: Option<u64>,
    #[serde(default)]
    capability_lease_expires_at_ms: Option<u64>,
    expected: serde_json::Value,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchFixture {
    schema_version: String,
    evaluation_mode: String,
    source_fixture: String,
    evaluation_owner: OwnerFixture,
    evaluated_at_ms: u64,
    requirements: RequirementsFixture,
    cases: Vec<BatchCase>,
    empty_inputs_allowed: bool,
    selected_device_id: Option<String>,
    selected_instance_id: Option<String>,
    authority: BatchAuthority,
    error_cases: Vec<ErrorCase>,
}

#[derive(Clone, Deserialize)]
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

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct GpuRequirementFixture {
    required: bool,
    min_memory_bytes: u64,
    runtime: String,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchCase {
    name: String,
    source_case: String,
    device_id: String,
    instance_id: String,
    #[serde(default)]
    snapshot_observed_at_ms: Option<u64>,
    #[serde(default)]
    capability_lease_expires_at_ms: Option<u64>,
    expected: ExpectedDecision,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedDecision {
    revision: u64,
    device_id: String,
    instance_id: String,
    matches_requirements: bool,
    exclusion_reasons: Vec<String>,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct BatchAuthority {
    identity_verified: bool,
    heartbeat_persisted: bool,
    inventory_authoritative: bool,
    placement_selected: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ErrorCase {
    name: String,
    error: String,
}

#[test]
#[allow(clippy::too_many_lines)]
fn persisted_inventory_placement_batch_contract_fixture() {
    let fixture: BatchFixture = serde_json::from_str(BATCH_FIXTURE).unwrap();
    assert_eq!(
        fixture.schema_version,
        "forge.device-inventory-placement-batch-evaluation/v1"
    );
    assert_eq!(
        fixture.evaluation_mode,
        PERSISTED_INVENTORY_PLACEMENT_EVALUATION_MODE
    );
    assert_eq!(
        fixture.source_fixture,
        "forge-device-inventory-placement-input-v1.json"
    );
    assert_eq!(fixture.evaluated_at_ms, 200_500);
    assert_eq!(fixture.cases.len(), 6);
    assert_eq!(fixture.selected_device_id, None);
    assert_eq!(fixture.selected_instance_id, None);
    assert_eq!(fixture.authority, BatchAuthority::default());

    let source: SourceFixture = serde_json::from_str(INPUT_FIXTURE).unwrap();
    let owner_value = owner(&fixture.evaluation_owner);
    assert!(fixture.requirements.gpu.runtime.is_empty());
    let inputs = fixture
        .cases
        .iter()
        .map(|case| build_input(&source, case, &owner_value))
        .collect::<Vec<_>>();
    let actual = evaluate_persisted_inventory_placement(
        &inputs,
        &owner_value,
        requirements(&fixture.requirements),
        fixture.evaluated_at_ms,
    )
    .unwrap();
    assert_eq!(
        actual.schema_version,
        PERSISTED_INVENTORY_PLACEMENT_EVALUATION_SCHEMA_VERSION
    );
    assert_eq!(actual.evaluation_mode, fixture.evaluation_mode);
    assert_eq!(actual.owner, owner_value);
    assert_eq!(actual.evaluated_at_ms, fixture.evaluated_at_ms);
    assert!(actual.owner_declaration_unverified);
    assert!(actual.device_attributes_unverified);
    assert_eq!(actual.selected_device_id, None);
    assert_eq!(actual.selected_instance_id, None);
    assert_eq!(
        actual.authority,
        PersistedInventoryPlacementBatchAuthority::default()
    );
    assert_eq!(actual.decisions.len(), fixture.cases.len());
    for (actual, case) in actual.decisions.iter().zip(&fixture.cases) {
        assert_eq!(actual.revision, case.expected.revision, "{}", case.name);
        assert_eq!(actual.device_id, case.expected.device_id, "{}", case.name);
        assert_eq!(
            actual.instance_id, case.expected.instance_id,
            "{}",
            case.name
        );
        assert_eq!(
            actual.matches_requirements, case.expected.matches_requirements,
            "{}",
            case.name
        );
        assert_eq!(
            actual.exclusion_reasons, case.expected.exclusion_reasons,
            "{}",
            case.name
        );
    }
}

#[test]
fn persisted_inventory_placement_batch_error_and_empty_contracts() {
    let fixture: BatchFixture = serde_json::from_str(BATCH_FIXTURE).unwrap();
    let source: SourceFixture = serde_json::from_str(INPUT_FIXTURE).unwrap();
    let owner_value = owner(&fixture.evaluation_owner);
    let inputs = fixture
        .cases
        .iter()
        .map(|case| build_input(&source, case, &owner_value))
        .collect::<Vec<_>>();
    for error_case in &fixture.error_cases {
        let mut owner_value = owner_value.clone();
        let mut candidates = inputs.clone();
        let mut evaluated_at_ms = fixture.evaluated_at_ms;
        match error_case.name.as_str() {
            "owner_mismatch" => owner_value.subject = "other-user".to_owned(),
            "duplicate_device" => candidates.push(candidates[0].clone()),
            "duplicate_instance" => {
                let mut case = fixture.cases[1].clone();
                case.instance_id = fixture.cases[0].instance_id.clone();
                candidates.push(build_input(&source, &case, &owner_value));
            }
            "invalid_evaluated_at" => evaluated_at_ms = 0,
            other => panic!("unknown error case {other}"),
        }
        let error = evaluate_persisted_inventory_placement(
            &candidates,
            &owner_value,
            requirements(&fixture.requirements),
            evaluated_at_ms,
        )
        .unwrap_err()
        .to_string();
        assert_eq!(error, error_case.error, "{}", error_case.name);
    }
    assert!(fixture.empty_inputs_allowed);
    let empty = evaluate_persisted_inventory_placement(
        &[],
        &owner_value,
        requirements(&fixture.requirements),
        fixture.evaluated_at_ms,
    )
    .unwrap();
    assert!(empty.decisions.is_empty());
    assert_eq!(
        empty.authority,
        PersistedInventoryPlacementBatchAuthority::default()
    );
    assert_eq!(empty.selected_device_id, None);
    assert_eq!(empty.selected_instance_id, None);
}

#[test]
fn persisted_inventory_placement_batch_rejects_nonempty_gpu() {
    let source: SourceFixture = serde_json::from_str(INPUT_FIXTURE).unwrap();
    let owner_value = owner(&source.evaluation_owner);
    let device = Device::restore(
        DeviceId::parse("device-a").unwrap(),
        TenantId::parse("tenant-1").unwrap(),
        DeviceApprovalState::Approved,
        false,
    );
    let persisted = PersistedInventoryDevice::restore(device, owner_value.clone(), false);
    let gpu = GpuCapability::new("gpu-a", "vendor", 1, 1).unwrap();
    let capability = CapabilitySnapshot::new(
        "linux",
        "amd64",
        8,
        6,
        16_384,
        12_288,
        8_192,
        4_096,
        vec![gpu],
        vec!["oci".to_owned()],
    )
    .unwrap();
    let runner = RunnerInstance::restore(
        DeviceId::parse("device-a").unwrap(),
        RunnerInstanceId::parse("runner-a").unwrap(),
        3,
        12,
        200_000,
        260_000,
        RunnerLiveness::Online,
        capability,
    )
    .unwrap();
    let state = restore_persisted_inventory(7, persisted, runner).unwrap();
    let input = build_persisted_inventory_placement_input(&state, &owner_value).unwrap();
    let error = evaluate_persisted_inventory_placement(
        &[input],
        &owner_value,
        DevicePlacementRequirements::any(),
        200_500,
    )
    .unwrap_err()
    .to_string();
    assert_eq!(error, "unsupported_persisted_placement_capability");
}

fn build_input(
    source: &SourceFixture,
    case: &BatchCase,
    owner_value: &SnapshotOwner,
) -> PersistedInventoryPlacementInput {
    let source_case = source
        .cases
        .iter()
        .find(|candidate| candidate.name == case.source_case)
        .unwrap();
    let approval = match source_case
        .approval_state
        .as_deref()
        .unwrap_or(&source.state.device.approval_state)
    {
        "approved" => DeviceApprovalState::Approved,
        "pending" => DeviceApprovalState::Pending,
        "revoked" => DeviceApprovalState::Revoked,
        value => panic!("invalid approval {value}"),
    };
    let device = Device::restore(
        DeviceId::parse(case.device_id.clone()).unwrap(),
        TenantId::parse(source.state.device.owner.tenant_id.clone()).unwrap(),
        approval,
        false,
    );
    let persisted = PersistedInventoryDevice::restore(device, owner_value.clone(), false);
    let runner = RunnerInstance::restore(
        DeviceId::parse(case.device_id.clone()).unwrap(),
        RunnerInstanceId::parse(case.instance_id.clone()).unwrap(),
        source.state.runner.generation,
        source.state.runner.heartbeat_sequence,
        case.snapshot_observed_at_ms
            .or(source_case.server_observed_at_ms)
            .unwrap_or(source.state.runner.server_observed_at_ms),
        case.capability_lease_expires_at_ms
            .or(source_case.capability_lease_expires_at_ms)
            .unwrap_or(source.state.runner.capability_lease_expires_at_ms),
        match source_case
            .liveness
            .as_deref()
            .unwrap_or(&source.state.runner.liveness)
        {
            "online" => RunnerLiveness::Online,
            "offline" => RunnerLiveness::Offline,
            value => panic!("invalid liveness {value}"),
        },
        capabilities(&source.state.runner.capabilities),
    )
    .unwrap();
    let state = restore_persisted_inventory(source.state.revision, persisted, runner).unwrap();
    build_persisted_inventory_placement_input(&state, owner_value).unwrap()
}

fn requirements(value: &RequirementsFixture) -> DevicePlacementRequirements {
    DevicePlacementRequirements::new(
        Some(&value.os),
        Some(&value.architecture),
        value.min_cpu_cores,
        value.min_memory_bytes,
        value.min_storage_bytes,
        vec![value.runtime.clone()],
        usize::from(value.gpu.required),
        value.gpu.min_memory_bytes,
    )
    .unwrap()
    .with_policy(
        DevicePlacementPolicy::new()
            .with_allowed_data_residency_zones(value.data_residency_zones.clone())
            .unwrap()
            .with_minimum_trust_zone(&value.minimum_trust_zone)
            .unwrap()
            .with_sandbox_floor(&value.sandbox_floor)
            .unwrap()
            .with_concurrency_slots(value.concurrency_slots)
            .unwrap(),
    )
}

fn owner(value: &OwnerFixture) -> SnapshotOwner {
    SnapshotOwner {
        issuer: value.issuer.clone(),
        subject: value.subject.clone(),
        tenant_id: value.tenant_id.clone(),
    }
}

fn capabilities(value: &CapabilityFixture) -> CapabilitySnapshot {
    CapabilitySnapshot::new(
        value.os.clone(),
        value.architecture.clone(),
        value.cpu_cores,
        value.available_cpu_cores,
        value.memory_bytes,
        value.available_memory_bytes,
        value.storage_bytes,
        value.available_storage_bytes,
        value
            .gpus
            .iter()
            .map(|gpu| {
                GpuCapability::new(
                    gpu.id.clone(),
                    gpu.vendor.clone(),
                    gpu.memory_bytes,
                    gpu.available_memory_bytes,
                )
                .unwrap()
            })
            .collect(),
        value.runtimes.clone(),
    )
    .unwrap()
}
