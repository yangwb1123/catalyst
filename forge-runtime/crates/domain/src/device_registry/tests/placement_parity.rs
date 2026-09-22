use super::*;
use serde::Deserialize;

const FIXTURE: &str = include_str!(
    "../../../../../../docs/contracts/fixtures/forge-device-placement-policy-parity-v1.json"
);
const GPU_FIXTURE: &str = include_str!(
    "../../../../../../docs/contracts/fixtures/forge-device-placement-gpu-policy-parity-v1.json"
);
const SESSION_FIXTURE: &str = include_str!(
    "../../../../../../docs/contracts/fixtures/forge-session-placement-observation-v1.json"
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionFixture {
    api_version: String,
    owner: OwnerFixture,
    conversation_id: String,
    run_id: String,
    placement: Fixture,
    expected: SessionExpectedFixture,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionExpectedFixture {
    evaluation_mode: String,
    evaluated_at_ms: u64,
    owner_declaration_unverified: bool,
    device_attributes_unverified: bool,
    decisions: Vec<SessionDecisionFixture>,
    selected_device_id: Option<String>,
    selected_instance_id: Option<String>,
    authority: SessionAuthorityFixture,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionDecisionFixture {
    device_id: String,
    instance_id: String,
    matches_requirements: bool,
    exclusion_reasons: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
struct SessionAuthorityFixture {
    identity_verified: bool,
    heartbeat_persisted: bool,
    inventory_authoritative: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
}

#[test]
fn shared_policy_fixture_matches_offline_go_projection() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
    assert_fixture_matches_offline_go_projection(&fixture);
}

#[test]
fn shared_gpu_policy_fixture_matches_offline_go_projection() {
    let fixture: Fixture = serde_json::from_str(GPU_FIXTURE).unwrap();
    assert_fixture_matches_offline_go_projection(&fixture);
}

#[test]
#[allow(clippy::too_many_lines)]
fn shared_session_placement_fixture_binds_run_without_authority() {
    let fixture: SessionFixture = serde_json::from_str(SESSION_FIXTURE).unwrap();
    assert_eq!(
        fixture.api_version,
        "forgeos.session-placement-observation-contract/v1"
    );
    assert_eq!(fixture.placement.owner.tenant_id, fixture.owner.tenant_id);
    let requirements = build_requirements(&fixture.placement.requirements);
    let candidates = fixture
        .placement
        .candidates
        .iter()
        .map(|candidate| {
            build_candidate(candidate, &fixture.owner, &fixture.placement.requirements)
        })
        .collect::<Vec<_>>();
    let owner = SessionPlacementOwner {
        issuer: fixture.owner.issuer.clone(),
        subject: fixture.owner.subject.clone(),
        tenant_id: TenantId::parse(fixture.owner.tenant_id.clone()).unwrap(),
    };
    let observation = observe_session_placement(SessionPlacementObservationRequest {
        owner: owner.clone(),
        placement_owner: SessionPlacementOwner {
            issuer: fixture.placement.owner.issuer.clone(),
            subject: fixture.placement.owner.subject.clone(),
            tenant_id: TenantId::parse(fixture.placement.owner.tenant_id.clone()).unwrap(),
        },
        conversation_id: fixture.conversation_id.clone(),
        run_id: fixture.run_id.clone(),
        placement: DevicePlacementRequest::new(
            TenantId::parse(fixture.placement.owner.tenant_id.clone()).unwrap(),
            requirements,
        ),
        evaluated_at_ms: fixture.placement.evaluated_at_ms,
        candidates,
    })
    .unwrap();

    assert_eq!(
        observation.schema_version,
        SESSION_PLACEMENT_OBSERVATION_SCHEMA_VERSION
    );
    assert_eq!(
        observation.evaluation_mode,
        fixture.expected.evaluation_mode
    );
    assert_eq!(observation.owner.issuer, fixture.owner.issuer);
    assert_eq!(observation.owner.subject, fixture.owner.subject);
    assert_eq!(
        observation.owner.tenant_id.as_str(),
        fixture.owner.tenant_id
    );
    assert_eq!(observation.conversation_id, fixture.conversation_id);
    assert_eq!(observation.run_id, fixture.run_id);
    assert_eq!(
        observation.evaluated_at_ms,
        fixture.expected.evaluated_at_ms
    );
    assert_eq!(
        observation.owner_declaration_unverified,
        fixture.expected.owner_declaration_unverified
    );
    assert_eq!(
        observation.device_attributes_unverified,
        fixture.expected.device_attributes_unverified
    );
    assert_eq!(
        observation.selected_device_id,
        fixture.expected.selected_device_id
    );
    assert_eq!(
        observation.selected_instance_id,
        fixture.expected.selected_instance_id
    );
    assert_eq!(
        observation.authority,
        SessionPlacementAuthority {
            identity_verified: fixture.expected.authority.identity_verified,
            heartbeat_persisted: fixture.expected.authority.heartbeat_persisted,
            inventory_authoritative: fixture.expected.authority.inventory_authoritative,
            reservation_created: fixture.expected.authority.reservation_created,
            execution_authorized: fixture.expected.authority.execution_authorized,
            dispatch_performed: fixture.expected.authority.dispatch_performed,
        }
    );
    assert!(
        !observation.authority.identity_verified
            && !observation.authority.heartbeat_persisted
            && !observation.authority.inventory_authoritative
            && !observation.authority.reservation_created
            && !observation.authority.execution_authorized
            && !observation.authority.dispatch_performed
    );
    assert_eq!(
        observation.decisions.len(),
        fixture.expected.decisions.len()
    );
    for (actual, expected) in observation
        .decisions
        .iter()
        .zip(&fixture.expected.decisions)
    {
        assert_eq!(actual.device_id, expected.device_id);
        assert_eq!(actual.instance_id, expected.instance_id);
        assert_eq!(actual.matches_requirements, expected.matches_requirements);
        assert_eq!(actual.exclusion_reasons, expected.exclusion_reasons);
    }
}

fn assert_fixture_matches_offline_go_projection(fixture: &Fixture) {
    // The Rust reference returns only pure per-device decisions; it has no API to
    // select a target, authorize execution, reserve capacity, or dispatch work.
    assert_eq!(
        fixture.schema_version,
        "forge.device-placement-policy-parity-test/v1"
    );
    assert!(fixture.max_snapshot_age_ms <= DEVICE_HEARTBEAT_STALE_AFTER_MS);
    assert!(fixture.requirements.gpu.runtime.is_empty());
    if !fixture.requirements.gpu.required {
        assert_eq!(fixture.requirements.gpu.min_memory_bytes, 0);
    }
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
        usize::from(fixture.gpu.required),
        if fixture.gpu.required {
            fixture.gpu.min_memory_bytes
        } else {
            0
        },
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
    let approval = match fixture.device.approval_state.as_str() {
        "approved" => DeviceApprovalState::Approved,
        "pending" => DeviceApprovalState::Pending,
        "revoked" => DeviceApprovalState::Revoked,
        value => panic!("unsupported fixture approval_state {value:?}"),
    };
    let cordoned = match fixture.device.cordon_state.as_str() {
        "clear" => false,
        "cordoned" => true,
        value => panic!("unsupported fixture cordon_state {value:?}"),
    };
    let device = Device::restore(device_id.clone(), tenant_id, approval, cordoned);
    let liveness = match fixture.device.liveness.as_str() {
        "online" => RunnerLiveness::Online,
        "offline" => RunnerLiveness::Offline,
        value => panic!("unsupported fixture liveness {value:?}"),
    };
    let instance_id = RunnerInstanceId::parse(fixture.instance_id.clone()).unwrap();
    let capabilities = build_capabilities(&fixture.device, requirements);
    let instance = RunnerInstance::restore(
        device_id,
        instance_id,
        1,
        1,
        fixture.device.snapshot_observed_at_ms,
        fixture.device.lease_expires_at_ms,
        liveness,
        capabilities,
    )
    .unwrap();
    DevicePlacementCandidate::new(device, instance)
        .unwrap()
        .with_attributes(build_attributes(&fixture.device))
}

fn assert_common_candidate_fields(fixture: &CandidateFixture, owner: &OwnerFixture) {
    assert!(matches!(
        fixture.device.approval_state.as_str(),
        "approved" | "pending" | "revoked"
    ));
    assert!(matches!(
        fixture.device.cordon_state.as_str(),
        "clear" | "cordoned"
    ));
    assert!(matches!(
        fixture.device.liveness.as_str(),
        "online" | "offline"
    ));
    assert!(fixture.device.gpu.runtime.is_empty());
    assert_eq!(fixture.device.owner.tenant_id, owner.tenant_id);
    assert_eq!(fixture.device.owner.issuer, owner.issuer);
    assert_eq!(fixture.device.owner.subject, owner.subject);
}

fn build_capabilities(
    fixture: &DeviceFixture,
    requirements: &RequirementsFixture,
) -> CapabilitySnapshot {
    let gpus = if fixture.gpu.present {
        vec![
            GpuCapability::new(
                "gpu-parity",
                "nvidia",
                fixture.gpu.memory_bytes,
                fixture.gpu.memory_bytes,
            )
            .unwrap(),
        ]
    } else {
        assert_eq!(fixture.gpu.memory_bytes, 0);
        Vec::new()
    };
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
        gpus,
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
        DevicePlacementExclusion::RunnerOffline => "declared_offline".to_owned(),
        DevicePlacementExclusion::HeartbeatObservedInFuture => {
            "snapshot_declared_from_future".to_owned()
        }
        DevicePlacementExclusion::HeartbeatStale => "snapshot_stale".to_owned(),
        DevicePlacementExclusion::CapabilityLeaseExpired => "declared_lease_expired".to_owned(),
        DevicePlacementExclusion::CapabilityLeaseInvalid => "declared_lease_invalid".to_owned(),
        DevicePlacementExclusion::GpuCountInsufficient => "gpu_missing".to_owned(),
        reason => reason.as_str().to_owned(),
    }
}
