use super::*;
use serde::Deserialize;

const FIXTURE: &str = include_str!(
    "../../../../../../docs/contracts/fixtures/forge-device-inventory-placement-input-v1.json"
);
const EVALUATION_FIXTURE: &str = include_str!(
    "../../../../../../docs/contracts/fixtures/forge-device-inventory-placement-evaluation-v1.json"
);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    evaluation_owner: OwnerFixture,
    policy_requirements: PolicyFixture,
    authority: Authority,
    state: StateFixture,
    cases: Vec<Case>,
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
struct PolicyFixture {
    data_residency_zones: Vec<String>,
    minimum_trust_zone: String,
    sandbox_floor: String,
    concurrency_slots: u16,
}

#[derive(Deserialize)]
#[allow(clippy::struct_excessive_bools)]
#[serde(deny_unknown_fields)]
struct Authority {
    identity_verified: bool,
    heartbeat_persisted: bool,
    inventory_authoritative: bool,
    placement_selected: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StateFixture {
    revision: u64,
    device: DeviceFixture,
    runner: RunnerFixture,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceFixture {
    device_id: String,
    owner: OwnerFixture,
    approval_state: String,
    cordon_state: String,
    reservation_state: String,
}

#[derive(Deserialize)]
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

#[derive(Deserialize)]
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GpuFixture {
    id: String,
    vendor: String,
    memory_bytes: u64,
    available_memory_bytes: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
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
    expected: Expected,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    accepted: bool,
    error: String,
    #[serde(default)]
    revision: u64,
    #[serde(default)]
    device_id: String,
    #[serde(default)]
    instance_id: String,
    #[serde(default)]
    generation: u64,
    #[serde(default)]
    heartbeat_sequence: u64,
    #[serde(default)]
    approval_state: String,
    #[serde(default)]
    cordon_state: String,
    #[serde(default)]
    reservation_state: String,
    #[serde(default)]
    liveness: String,
    #[serde(default)]
    snapshot_observed_at_ms: u64,
    #[serde(default)]
    lease_expires_at_ms: u64,
    #[serde(default)]
    owner_declaration_unverified: bool,
    #[serde(default)]
    policy_attributes_unverified: bool,
    #[serde(default)]
    data_residency_zones: Vec<String>,
    #[serde(default)]
    trust_zone: String,
    #[serde(default)]
    sandbox_levels: Vec<String>,
    #[serde(default)]
    concurrency_limit: u16,
    #[serde(default)]
    active_concurrency: u16,
    #[serde(default)]
    policy_requirements_met: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EvaluationFixture {
    schema_version: String,
    evaluation_mode: String,
    source_fixture: String,
    source_case: String,
    evaluated_at_ms: u64,
    policy_requirements: EvaluationPolicyFixture,
    authority: EvaluationAuthority,
    expected: EvaluationExpected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EvaluationPolicyFixture {
    os: String,
    architecture: String,
    min_cpu_cores: u32,
    min_memory_bytes: u64,
    min_storage_bytes: u64,
    runtime: String,
    gpu: EvaluationGpuFixture,
    data_residency_zones: Vec<String>,
    minimum_trust_zone: String,
    sandbox_floor: String,
    concurrency_slots: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EvaluationGpuFixture {
    required: bool,
    min_memory_bytes: u64,
    runtime: String,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EvaluationAuthority {
    placement_evaluated: bool,
    placement_selected: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EvaluationExpected {
    accepted: bool,
    error: String,
    revision: u64,
    device_id: String,
    instance_id: String,
    matches_requirements: bool,
    exclusion_reasons: Vec<String>,
    owner_declaration_unverified: bool,
    device_attributes_unverified: bool,
}

#[test]
fn persisted_inventory_placement_input_contract_fixture() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
    assert_envelope(&fixture);
    for case in &fixture.cases {
        let state = state_for_case(&fixture.state, case);
        let evaluation_owner = case
            .evaluation_owner
            .as_ref()
            .map_or_else(|| owner(&fixture.evaluation_owner), owner);
        let result = state.map_err(|error| error.to_string()).and_then(|state| {
            build_persisted_inventory_placement_input(&state, &evaluation_owner)
                .map_err(|error| error.to_string())
        });
        match (case.expected.accepted, result) {
            (true, Ok(input)) => assert_input(case, &input, &fixture.policy_requirements),
            (false, Err(error)) => {
                assert_eq!(
                    error.as_str(),
                    case.expected.error.as_str(),
                    "{}",
                    case.name
                );
            }
            (true, Err(error)) => panic!("{} rejected: {error}", case.name),
            (false, Ok(_)) => panic!("{} unexpectedly accepted", case.name),
        }
    }
}

#[allow(clippy::too_many_lines)]
#[test]
fn persisted_inventory_offline_placement_evaluation_contract_fixture() {
    let evaluation: EvaluationFixture = serde_json::from_str(EVALUATION_FIXTURE).unwrap();
    assert_eq!(
        evaluation.schema_version,
        "forge.device-inventory-placement-evaluation/v1"
    );
    assert_eq!(
        evaluation.evaluation_mode,
        "pure_persisted_inventory_offline_evaluation"
    );
    assert_eq!(
        evaluation.source_fixture,
        "forge-device-inventory-placement-input-v1.json"
    );
    assert_eq!(evaluation.source_case, "online");
    assert!(!evaluation.authority.placement_evaluated);
    assert!(!evaluation.authority.placement_selected);
    assert!(!evaluation.authority.reservation_created);
    assert!(!evaluation.authority.execution_authorized);
    assert!(!evaluation.authority.dispatch_performed);

    let source: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let source_case = source
        .cases
        .iter()
        .find(|case| case.name == evaluation.source_case)
        .unwrap();
    let state = state_for_case(&source.state, source_case).unwrap();
    let input = build_persisted_inventory_placement_input(&state, &owner(&source.evaluation_owner))
        .unwrap();
    let policy = &evaluation.policy_requirements;
    let requirements = DevicePlacementRequirements::new(
        Some(&policy.os),
        Some(&policy.architecture),
        policy.min_cpu_cores,
        policy.min_memory_bytes,
        policy.min_storage_bytes,
        vec![policy.runtime.clone()],
        usize::from(policy.gpu.required),
        policy.gpu.min_memory_bytes,
    )
    .unwrap()
    .with_policy(
        DevicePlacementPolicy::new()
            .with_allowed_data_residency_zones(policy.data_residency_zones.clone())
            .unwrap()
            .with_minimum_trust_zone(&policy.minimum_trust_zone)
            .unwrap()
            .with_sandbox_floor(&policy.sandbox_floor)
            .unwrap()
            .with_concurrency_slots(policy.concurrency_slots)
            .unwrap(),
    );
    assert!(!evaluation.policy_requirements.gpu.required);
    assert_eq!(evaluation.policy_requirements.gpu.runtime, "");
    let actual = evaluate_persisted_inventory_placement_input(
        &input,
        requirements,
        evaluation.evaluated_at_ms,
    )
    .map_err(|error| error.to_string());
    let expected = &evaluation.expected;
    assert!(expected.accepted, "fixture error: {}", expected.error);
    let actual = actual.unwrap();
    assert_eq!(actual.revision, expected.revision);
    assert_eq!(actual.device_id, expected.device_id);
    assert_eq!(actual.instance_id, expected.instance_id);
    assert_eq!(actual.matches_requirements, expected.matches_requirements);
    assert_eq!(actual.exclusion_reasons, expected.exclusion_reasons);
    assert_eq!(
        actual.owner_declaration_unverified,
        expected.owner_declaration_unverified
    );
    assert_eq!(
        actual.device_attributes_unverified,
        expected.device_attributes_unverified
    );
}

#[test]
fn persisted_inventory_offline_placement_evaluation_rejects_gpu_capability() {
    let source: Fixture = serde_json::from_str(FIXTURE).unwrap();
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
    let requirements = DevicePlacementRequirements::any();
    let error =
        evaluate_persisted_inventory_placement_input(&input, requirements, 200_500).unwrap_err();
    assert_eq!(
        error.to_string(),
        "unsupported_persisted_placement_capability"
    );
}

fn assert_envelope(fixture: &Fixture) {
    assert_eq!(
        fixture.schema_version,
        "forge.device-inventory-placement-input/v1"
    );
    assert_eq!(
        fixture.evaluation_mode,
        "pure_persisted_inventory_to_placement_input"
    );
    assert!(!fixture.authority.identity_verified);
    assert!(!fixture.authority.heartbeat_persisted);
    assert!(!fixture.authority.inventory_authoritative);
    assert!(!fixture.authority.placement_selected);
    assert!(!fixture.authority.reservation_created);
    assert!(!fixture.authority.execution_authorized);
    assert!(!fixture.authority.dispatch_performed);
    assert_eq!(fixture.cases.len(), 12);
    assert!(!fixture.policy_requirements.data_residency_zones.is_empty());
    assert!(!fixture.policy_requirements.minimum_trust_zone.is_empty());
    assert!(!fixture.policy_requirements.sandbox_floor.is_empty());
    assert_ne!(fixture.policy_requirements.concurrency_slots, 0);
}

fn state_for_case(
    base: &StateFixture,
    case: &Case,
) -> Result<PersistedInventoryState, PersistenceError> {
    let approval = approval(
        case.approval_state
            .as_deref()
            .unwrap_or(&base.device.approval_state),
    );
    let cordoned = case
        .cordon_state
        .as_deref()
        .unwrap_or(&base.device.cordon_state)
        == "cordoned";
    let device = Device::restore(
        DeviceId::parse(base.device.device_id.clone()).unwrap(),
        TenantId::parse(base.device.owner.tenant_id.clone()).unwrap(),
        approval,
        cordoned,
    );
    let persisted = PersistedInventoryDevice::restore(
        device,
        owner(&base.device.owner),
        base.device.reservation_state == "reserved",
    );
    let runner = RunnerInstance::restore(
        DeviceId::parse(
            case.runner_device_id
                .as_deref()
                .unwrap_or(&base.runner.device_id),
        )
        .unwrap(),
        RunnerInstanceId::parse(base.runner.instance_id.clone()).unwrap(),
        base.runner.generation,
        base.runner.heartbeat_sequence,
        case.server_observed_at_ms
            .unwrap_or(base.runner.server_observed_at_ms),
        case.capability_lease_expires_at_ms
            .unwrap_or(base.runner.capability_lease_expires_at_ms),
        liveness(case.liveness.as_deref().unwrap_or(&base.runner.liveness)),
        capabilities(&base.runner.capabilities),
    )
    .unwrap();
    restore_persisted_inventory(base.revision, persisted, runner)
}

#[allow(clippy::too_many_lines)]
fn assert_input(case: &Case, input: &PersistedInventoryPlacementInput, policy: &PolicyFixture) {
    let expected = &case.expected;
    let candidate = input.candidate();
    let device = candidate.device();
    let runner = candidate.instance();
    let attributes = candidate.attributes();
    assert_eq!(input.revision(), expected.revision, "{}", case.name);
    assert_eq!(device.id().as_str(), expected.device_id, "{}", case.name);
    assert_eq!(
        runner.instance_id().as_str(),
        expected.instance_id,
        "{}",
        case.name
    );
    assert_eq!(runner.generation(), expected.generation, "{}", case.name);
    assert_eq!(
        runner.heartbeat_sequence(),
        expected.heartbeat_sequence,
        "{}",
        case.name
    );
    assert_eq!(
        approval_string(device.approval()),
        expected.approval_state,
        "{}",
        case.name
    );
    assert_eq!(
        if device.is_cordoned() {
            "cordoned"
        } else {
            "clear"
        },
        expected.cordon_state,
        "{}",
        case.name
    );
    assert_eq!(
        if input.reserved() { "reserved" } else { "none" },
        expected.reservation_state,
        "{}",
        case.name
    );
    assert_eq!(
        liveness_string(runner.liveness()),
        expected.liveness,
        "{}",
        case.name
    );
    assert_eq!(
        runner.server_observed_at_ms(),
        expected.snapshot_observed_at_ms,
        "{}",
        case.name
    );
    assert_eq!(
        runner.capability_lease_expires_at_ms(),
        expected.lease_expires_at_ms,
        "{}",
        case.name
    );
    assert_eq!(
        input.owner_declaration_unverified(),
        expected.owner_declaration_unverified,
        "{}",
        case.name
    );
    assert_eq!(
        input.policy_attributes_unverified(),
        expected.policy_attributes_unverified,
        "{}",
        case.name
    );
    assert_eq!(
        attributes.data_residency_zones(),
        expected.data_residency_zones,
        "{}",
        case.name
    );
    assert_eq!(
        attributes.trust_zone().as_str(),
        expected.trust_zone,
        "{}",
        case.name
    );
    assert!(
        attributes.sandbox_levels().is_empty() && expected.sandbox_levels.is_empty(),
        "{}",
        case.name
    );
    assert_eq!(
        attributes.concurrency_limit(),
        expected.concurrency_limit,
        "{}",
        case.name
    );
    assert_eq!(
        attributes.active_concurrency(),
        expected.active_concurrency,
        "{}",
        case.name
    );
    let request = DevicePlacementRequest::new(
        device.tenant_id().clone(),
        DevicePlacementRequirements::any().with_policy(policy_value(policy)),
    );
    let decision = dry_run_device_placement(
        std::slice::from_ref(candidate),
        &request,
        runner.server_observed_at_ms(),
    )
    .unwrap();
    assert_eq!(
        matches!(
            decision[0].disposition(),
            DevicePlacementDisposition::Eligible
        ),
        expected.policy_requirements_met,
        "{}",
        case.name
    );
}

fn policy_value(value: &PolicyFixture) -> DevicePlacementPolicy {
    DevicePlacementPolicy::new()
        .with_allowed_data_residency_zones(value.data_residency_zones.clone())
        .unwrap()
        .with_minimum_trust_zone(&value.minimum_trust_zone)
        .unwrap()
        .with_sandbox_floor(&value.sandbox_floor)
        .unwrap()
        .with_concurrency_slots(value.concurrency_slots)
        .unwrap()
}

fn owner(value: &OwnerFixture) -> SnapshotOwner {
    SnapshotOwner {
        issuer: value.issuer.clone(),
        subject: value.subject.clone(),
        tenant_id: value.tenant_id.clone(),
    }
}

fn approval(value: &str) -> DeviceApprovalState {
    match value {
        "approved" => DeviceApprovalState::Approved,
        "pending" => DeviceApprovalState::Pending,
        "revoked" => DeviceApprovalState::Revoked,
        _ => panic!("invalid approval {value}"),
    }
}

fn approval_string(value: DeviceApprovalState) -> &'static str {
    match value {
        DeviceApprovalState::Approved => "approved",
        DeviceApprovalState::Pending => "pending",
        DeviceApprovalState::Revoked => "revoked",
    }
}

fn liveness(value: &str) -> RunnerLiveness {
    match value {
        "online" => RunnerLiveness::Online,
        "offline" => RunnerLiveness::Offline,
        _ => panic!("invalid liveness {value}"),
    }
}

fn liveness_string(value: RunnerLiveness) -> &'static str {
    match value {
        RunnerLiveness::Online => "online",
        RunnerLiveness::Offline => "offline",
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
