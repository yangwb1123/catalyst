use super::super::*;
use serde::Deserialize;

const FIXTURE: &str = include_str!(
    "../../../../../../docs/contracts/fixtures/forge-device-enrollment-heartbeat-lifecycle-v1.json"
);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    notice: String,
    owner: OwnerFixture,
    device: DeviceFixture,
    challenge: ChallengeFixture,
    proof: ProofFixture,
    capabilities: CapabilitiesFixture,
    authority: AuthorityFixture,
    cases: Vec<CaseFixture>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerFixture {
    issuer: String,
    subject: String,
    tenant_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceFixture {
    device_id: String,
    key_id: String,
    public_key_sha256: String,
    approval_state: String,
    credential_state: String,
    cordon_state: String,
    reservation_state: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChallengeFixture {
    challenge_id: String,
    challenge_sha256: String,
    issued_at_ms: u64,
    expires_at_ms: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProofFixture {
    device_id: String,
    key_id: String,
    public_key_sha256: String,
    owner: OwnerFixture,
    challenge_id: String,
    challenge_sha256: String,
    proof_sha256: String,
    issued_at_ms: u64,
    expires_at_ms: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CapabilitiesFixture {
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
struct AuthorityFixture {
    identity_verified: bool,
    challenge_consumed: bool,
    heartbeat_persisted: bool,
    inventory_authoritative: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CaseFixture {
    name: String,
    approval_state: String,
    credential_state: String,
    challenge_consumed: bool,
    proof_owner: String,
    heartbeat: HeartbeatFixture,
    expected_heartbeat_revision: u64,
    expected_inventory_revision: u64,
    server_observed_at_ms: u64,
    lease_ttl_ms: u64,
    identity_now_ms: u64,
    evaluated_at_ms: u64,
    stale_after_ms: u64,
    expected: ExpectedFixture,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HeartbeatFixture {
    device_id: String,
    instance_id: String,
    generation: u64,
    sequence: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedFixture {
    accepted: bool,
    error: Option<String>,
    heartbeat_revision: Option<u64>,
    inventory_revision: Option<u64>,
    generation: Option<u64>,
    heartbeat_sequence: Option<u64>,
    status: Option<String>,
    fresh: Option<bool>,
    declared_eligible: Option<bool>,
}

#[test]
fn enrollment_heartbeat_lifecycle_contract_fixture() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(
        fixture.schema_version,
        "forge.device-enrollment-heartbeat-lifecycle/v1"
    );
    assert_eq!(fixture.evaluation_mode, "pure_joined_value_transition");
    assert!(fixture.notice.contains("does not verify cryptography"));
    assert_eq!(fixture.owner.tenant_id, "tenant-1");
    assert_eq!(fixture.device.device_id, "device-a");
    assert_eq!(fixture.device.key_id, "key-a");
    assert_eq!(fixture.device.approval_state, "approved");
    assert_eq!(fixture.device.credential_state, "active");
    assert_eq!(fixture.device.cordon_state, "clear");
    assert_eq!(fixture.device.reservation_state, "none");
    assert!(!fixture.authority.identity_verified);
    assert!(!fixture.authority.challenge_consumed);
    assert!(!fixture.authority.heartbeat_persisted);
    assert!(!fixture.authority.inventory_authoritative);
    assert!(!fixture.authority.reservation_created);
    assert!(!fixture.authority.execution_authorized);
    assert!(!fixture.authority.dispatch_performed);
    assert_eq!(fixture.cases.len(), 10);

    let declared_owner = owner(&fixture.owner);
    let device_id = DeviceId::parse(&fixture.device.device_id).unwrap();
    let tenant_id = TenantId::parse(&fixture.owner.tenant_id).unwrap();
    let capabilities = capabilities(&fixture.capabilities);
    let mut current_runner: Option<PersistedRunnerInstance> = None;
    let mut current_inventory: Option<PersistedInventoryState> = None;

    for case in fixture.cases {
        let mut proof_owner = owner(&fixture.proof.owner);
        if case.proof_owner == "foreign" {
            proof_owner = IdentityOwner::new("https://id.example", "user-foreign", "tenant-1");
        }
        let identity_device = DeviceIdentityBinding::new(
            fixture.device.device_id.clone(),
            declared_owner.clone(),
            fixture.device.key_id.clone(),
            fixture.device.public_key_sha256.clone(),
            case.approval_state.clone(),
            case.credential_state.clone(),
        );
        let challenge = IdentityChallenge::new(
            fixture.challenge.challenge_id.clone(),
            fixture.challenge.challenge_sha256.clone(),
            fixture.challenge.issued_at_ms,
            fixture.challenge.expires_at_ms,
            case.challenge_consumed,
        );
        let proof = DeviceIdentityProof::new(
            fixture.proof.device_id.clone(),
            fixture.proof.key_id.clone(),
            fixture.proof.public_key_sha256.clone(),
            proof_owner,
            fixture.proof.challenge_id.clone(),
            fixture.proof.challenge_sha256.clone(),
            fixture.proof.proof_sha256.clone(),
            fixture.proof.issued_at_ms,
            fixture.proof.expires_at_ms,
        );
        let identity = evaluate_identity_proof(
            &declared_owner,
            &identity_device,
            &challenge,
            &proof,
            case.identity_now_ms,
        );
        let decision = match identity {
            Ok(decision) => decision,
            Err(error) => {
                if !case.expected.accepted {
                    assert_eq!(
                        error.to_string(),
                        case.expected.error.as_deref().unwrap(),
                        "{}",
                        case.name
                    );
                    continue;
                }
                panic!("{}: identity proof rejected: {error}", case.name);
            }
        };
        if decision.approval_required() {
            assert!(
                !case.expected.accepted,
                "{}: pending approval accepted",
                case.name
            );
            assert_eq!(
                case.expected.error.as_deref(),
                Some("approval_required"),
                "{}",
                case.name
            );
            continue;
        }
        assert!(!decision.approval_required(), "{}", case.name);
        let device = Device::restore(
            device_id.clone(),
            tenant_id.clone(),
            DeviceApprovalState::Approved,
            false,
        );
        let heartbeat = RunnerHeartbeat::new(
            DeviceId::parse(&case.heartbeat.device_id).unwrap(),
            RunnerInstanceId::parse(&case.heartbeat.instance_id).unwrap(),
            case.heartbeat.generation,
            case.heartbeat.sequence,
            capabilities.clone(),
        )
        .unwrap();
        let next_runner = match commit_device_heartbeat(
            &device,
            current_runner.as_ref(),
            case.expected_heartbeat_revision,
            &heartbeat,
            case.server_observed_at_ms,
            case.lease_ttl_ms,
        ) {
            Ok(value) => value,
            Err(error) => {
                assert_eq!(
                    error.to_string(),
                    case.expected.error.as_deref().unwrap(),
                    "{}",
                    case.name
                );
                continue;
            }
        };
        let inventory_device = PersistedInventoryDevice::restore(
            device,
            SnapshotOwner {
                issuer: fixture.owner.issuer.clone(),
                subject: fixture.owner.subject.clone(),
                tenant_id: fixture.owner.tenant_id.clone(),
            },
            false,
        );
        let next_inventory = match commit_persisted_inventory(
            current_inventory.as_ref(),
            case.expected_inventory_revision,
            inventory_device,
            next_runner.instance().clone(),
        ) {
            Ok(value) => value,
            Err(error) => {
                assert_eq!(
                    error.to_string(),
                    case.expected.error.as_deref().unwrap(),
                    "{}",
                    case.name
                );
                continue;
            }
        };
        let projection = project_persisted_inventory(
            &next_inventory,
            &SnapshotOwner {
                issuer: fixture.owner.issuer.clone(),
                subject: fixture.owner.subject.clone(),
                tenant_id: fixture.owner.tenant_id.clone(),
            },
            case.evaluated_at_ms,
            case.stale_after_ms,
        )
        .unwrap();
        assert_eq!(
            next_runner.revision(),
            case.expected.heartbeat_revision.unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            next_inventory.revision(),
            case.expected.inventory_revision.unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            next_runner.instance().generation(),
            case.expected.generation.unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            next_runner.instance().heartbeat_sequence(),
            case.expected.heartbeat_sequence.unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            projection.status().as_str(),
            case.expected.status.unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            projection.fresh,
            case.expected.fresh.unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            projection.declared_eligible,
            case.expected.declared_eligible.unwrap(),
            "{}",
            case.name
        );
        current_runner = Some(next_runner);
        current_inventory = Some(next_inventory);
    }
    let inventory = current_inventory.unwrap();
    assert!(
        restore_persisted_inventory(
            inventory.revision(),
            inventory.device().clone(),
            inventory.runner().clone(),
        )
        .is_ok()
    );
}

fn owner(value: &OwnerFixture) -> IdentityOwner {
    IdentityOwner::new(&value.issuer, &value.subject, &value.tenant_id)
}

fn capabilities(value: &CapabilitiesFixture) -> CapabilitySnapshot {
    let gpus = value
        .gpus
        .iter()
        .map(|gpu| {
            GpuCapability::new(
                &gpu.id,
                &gpu.vendor,
                gpu.memory_bytes,
                gpu.available_memory_bytes,
            )
            .unwrap()
        })
        .collect();
    CapabilitySnapshot::new(
        &value.os,
        &value.architecture,
        value.cpu_cores,
        value.available_cpu_cores,
        value.memory_bytes,
        value.available_memory_bytes,
        value.storage_bytes,
        value.available_storage_bytes,
        gpus,
        value.runtimes.clone(),
    )
    .unwrap()
}
