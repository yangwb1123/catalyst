use super::*;
use serde::Deserialize;

const FIXTURE: &str =
    include_str!("../../../../../../docs/contracts/fixtures/forge-device-resource-summary-v1.json");
const NOTICE: &str = "Every owner, instance, resource, placement, and eligibility value is an unverified caller declaration. This read-only summary aggregates declarations, selects no target, and grants no execution authority.";

#[derive(Deserialize)]
#[allow(clippy::struct_field_names)]
#[serde(deny_unknown_fields)]
struct Fixture {
    api_version: String,
    inventory_contract_fixture: String,
    placement_contract_fixture: String,
    owner: OwnerFixture,
    inventory: InventoryFixture,
    placement_observation: PlacementFixture,
    expected: ExpectedFixture,
}

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
#[allow(dead_code)]
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

#[derive(Deserialize)]
#[allow(clippy::struct_excessive_bools)]
#[allow(dead_code)]
#[serde(deny_unknown_fields)]
struct PlacementFixture {
    schema_version: String,
    evaluation_mode: String,
    owner: OwnerFixture,
    conversation_id: String,
    run_id: String,
    evaluated_at_ms: u64,
    owner_declaration_unverified: bool,
    device_attributes_unverified: bool,
    decisions: Vec<DecisionFixture>,
    selected_device_id: Option<String>,
    selected_instance_id: Option<String>,
    authority: AuthorityFixture,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DecisionFixture {
    device_id: String,
    instance_id: String,
    matches_requirements: bool,
    exclusion_reasons: Vec<String>,
}

#[derive(Deserialize)]
#[allow(clippy::struct_excessive_bools)]
#[serde(deny_unknown_fields)]
struct AuthorityFixture {
    identity_verified: bool,
    heartbeat_persisted: bool,
    inventory_authoritative: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
}

#[derive(Deserialize)]
#[allow(clippy::struct_excessive_bools)]
#[serde(deny_unknown_fields)]
struct ExpectedFixture {
    schema_version: String,
    evaluation_mode: String,
    conversation_id: String,
    run_id: String,
    evaluated_at_ms: u64,
    owner_declaration_unverified: bool,
    inventory_declarations_unverified: bool,
    placement_declaration_unverified: bool,
    notice: String,
    device_count: usize,
    runner_instance_count: usize,
    available_cpu_cores: u64,
    available_memory_bytes: u64,
    available_storage_bytes: u64,
    available_gpu_count: usize,
    available_gpu_memory_bytes: u64,
    eligible_device_count: usize,
    eligible_instance_count: usize,
    selected_device_id: Option<String>,
    selected_instance_id: Option<String>,
    authority: AuthorityFixture,
}

#[test]
#[allow(clippy::too_many_lines)]
fn resource_summary_fixture_aggregates_unverified_declarations() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(
        fixture.api_version,
        "forgeos.device-resource-summary-contract/v1"
    );
    assert_eq!(
        fixture.inventory_contract_fixture,
        "forge-device-inventory-observation-v1"
    );
    assert_eq!(
        fixture.placement_contract_fixture,
        "forge-session-placement-observation-v1"
    );
    assert_eq!(
        fixture.inventory.schema_version,
        "forge.device-inventory-observation/v1"
    );
    assert_eq!(fixture.inventory.evaluation_mode, "offline_static_only");
    assert_eq!(fixture.inventory.evaluated_at_ms, 200_000);
    assert!(fixture.inventory.owner_declaration_unverified);
    assert!(fixture.inventory.inventory_declarations_unverified);
    assert!(!fixture.inventory.execution_authorized);
    assert!(!fixture.inventory.reservation_created);
    assert!(!fixture.inventory.dispatch_performed);
    assert_eq!(
        fixture.inventory.notice,
        "Every owner, instance, state, timestamp, resource, residency, trust, sandbox, and concurrency value is an unverified caller declaration. This read-only observation selects no target and grants no execution authority."
    );

    let owner = owner(&fixture.owner);
    assert_owner_equal(&fixture.inventory.owner_declaration, &fixture.owner);
    assert_owner_equal(&fixture.placement_observation.owner, &fixture.owner);
    let inventory = fixture
        .inventory
        .devices
        .iter()
        .map(|candidate| build_candidate(candidate, &fixture.owner))
        .collect::<Vec<_>>();
    let placement = SessionPlacementObservation {
        schema_version: SESSION_PLACEMENT_OBSERVATION_SCHEMA_VERSION,
        evaluation_mode: "offline_static_only",
        owner: owner.clone(),
        conversation_id: fixture.placement_observation.conversation_id.clone(),
        run_id: fixture.placement_observation.run_id.clone(),
        evaluated_at_ms: fixture.placement_observation.evaluated_at_ms,
        owner_declaration_unverified: fixture.placement_observation.owner_declaration_unverified,
        device_attributes_unverified: fixture.placement_observation.device_attributes_unverified,
        decisions: fixture
            .placement_observation
            .decisions
            .iter()
            .map(|decision| SessionPlacementDecision {
                device_id: decision.device_id.clone(),
                instance_id: decision.instance_id.clone(),
                matches_requirements: decision.matches_requirements,
                exclusion_reasons: decision.exclusion_reasons.clone(),
            })
            .collect(),
        selected_device_id: fixture.placement_observation.selected_device_id.clone(),
        selected_instance_id: fixture.placement_observation.selected_instance_id.clone(),
        authority: authority(&fixture.placement_observation.authority),
    };
    let observation = observe_device_resource_summary(DeviceResourceSummaryRequest {
        owner,
        inventory,
        placement,
    })
    .unwrap();
    let expected = &fixture.expected;
    assert_eq!(observation.schema_version, expected.schema_version);
    assert_eq!(observation.evaluation_mode, expected.evaluation_mode);
    assert_eq!(observation.conversation_id, expected.conversation_id);
    assert_eq!(observation.run_id, expected.run_id);
    assert_eq!(observation.evaluated_at_ms, expected.evaluated_at_ms);
    assert_eq!(
        observation.owner_declaration_unverified,
        expected.owner_declaration_unverified
    );
    assert_eq!(
        observation.inventory_declarations_unverified,
        expected.inventory_declarations_unverified
    );
    assert_eq!(
        observation.placement_declaration_unverified,
        expected.placement_declaration_unverified
    );
    assert_eq!(observation.notice, DEVICE_RESOURCE_SUMMARY_NOTICE);
    assert_eq!(NOTICE, expected.notice);
    assert_eq!(observation.device_count, expected.device_count);
    assert_eq!(
        observation.runner_instance_count,
        expected.runner_instance_count
    );
    assert_eq!(
        observation.available_cpu_cores,
        expected.available_cpu_cores
    );
    assert_eq!(
        observation.available_memory_bytes,
        expected.available_memory_bytes
    );
    assert_eq!(
        observation.available_storage_bytes,
        expected.available_storage_bytes
    );
    assert_eq!(
        observation.available_gpu_count,
        expected.available_gpu_count
    );
    assert_eq!(
        observation.available_gpu_memory_bytes,
        expected.available_gpu_memory_bytes
    );
    assert_eq!(
        observation.eligible_device_count,
        expected.eligible_device_count
    );
    assert_eq!(
        observation.eligible_instance_count,
        expected.eligible_instance_count
    );
    assert_eq!(observation.selected_device_id, expected.selected_device_id);
    assert_eq!(
        observation.selected_instance_id,
        expected.selected_instance_id
    );
    assert_eq!(observation.authority, authority(&expected.authority));
    assert_eq!(observation.authority, SessionPlacementAuthority::default());
}

#[allow(clippy::too_many_lines)]
fn build_candidate(
    candidate: &InventoryCandidateFixture,
    owner: &OwnerFixture,
) -> DevicePlacementCandidate {
    assert_owner_equal(&candidate.device.owner, owner);
    let device_id = DeviceId::parse(candidate.device.device_id.clone()).unwrap();
    let tenant_id = TenantId::parse(owner.tenant_id.clone()).unwrap();
    let approval = match candidate.device.approval_state.as_str() {
        "approved" => DeviceApprovalState::Approved,
        "pending" => DeviceApprovalState::Pending,
        "revoked" => DeviceApprovalState::Revoked,
        value => panic!("unexpected approval state {value:?}"),
    };
    let device = Device::restore(
        device_id.clone(),
        tenant_id,
        approval,
        candidate.device.cordon_state == "cordoned",
    );
    let gpus = if candidate.device.gpu.present {
        vec![
            GpuCapability::new(
                format!("{}-gpu", candidate.device.device_id),
                candidate.device.gpu.runtime.clone(),
                candidate.device.gpu.memory_bytes,
                candidate.device.gpu.memory_bytes,
            )
            .unwrap(),
        ]
    } else {
        Vec::new()
    };
    let capabilities = CapabilitySnapshot::new(
        candidate.device.os.clone(),
        candidate.device.architecture.clone(),
        candidate.device.available_cpu_cores.max(1),
        candidate.device.available_cpu_cores,
        candidate.device.available_memory_bytes.max(1),
        candidate.device.available_memory_bytes,
        candidate.device.available_storage_bytes,
        candidate.device.available_storage_bytes,
        gpus,
        candidate.device.runtimes.clone(),
    )
    .unwrap();
    let instance = RunnerInstance::restore(
        device_id,
        RunnerInstanceId::parse(candidate.instance_id.clone()).unwrap(),
        1,
        1,
        candidate.device.snapshot_observed_at_ms,
        candidate.device.lease_expires_at_ms,
        match candidate.device.liveness.as_str() {
            "online" => RunnerLiveness::Online,
            "offline" => RunnerLiveness::Offline,
            value => panic!("unexpected liveness {value:?}"),
        },
        capabilities,
    )
    .unwrap();
    DevicePlacementCandidate::new(device, instance).unwrap()
}

fn owner(value: &OwnerFixture) -> SessionPlacementOwner {
    SessionPlacementOwner {
        issuer: value.issuer.clone(),
        subject: value.subject.clone(),
        tenant_id: TenantId::parse(value.tenant_id.clone()).unwrap(),
    }
}

fn assert_owner_equal(left: &OwnerFixture, right: &OwnerFixture) {
    assert_eq!(left.issuer, right.issuer);
    assert_eq!(left.subject, right.subject);
    assert_eq!(left.tenant_id, right.tenant_id);
}

fn authority(value: &AuthorityFixture) -> SessionPlacementAuthority {
    SessionPlacementAuthority {
        identity_verified: value.identity_verified,
        heartbeat_persisted: value.heartbeat_persisted,
        inventory_authoritative: value.inventory_authoritative,
        reservation_created: value.reservation_created,
        execution_authorized: value.execution_authorized,
        dispatch_performed: value.dispatch_performed,
    }
}
