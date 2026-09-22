use super::super::*;
use serde::Deserialize;

const FIXTURE: &str = include_str!(
    "../../../../../../docs/contracts/fixtures/forge-device-identity-proof-contract-v1.json"
);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    notice: String,
    owner_declaration: OwnerFixture,
    device: DeviceFixture,
    challenge: ChallengeFixture,
    authority: AuthorityFixture,
    cases: Vec<IdentityCaseFixture>,
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
struct DeviceFixture {
    device_id: String,
    owner: OwnerFixture,
    key_id: String,
    public_key_sha256: String,
    approval_state: String,
    credential_state: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChallengeFixture {
    challenge_id: String,
    challenge_sha256: String,
    issued_at_ms: u64,
    expires_at_ms: u64,
    consumed: bool,
}

#[derive(Deserialize)]
#[allow(clippy::struct_excessive_bools)]
#[serde(deny_unknown_fields)]
struct AuthorityFixture {
    identity_verified: bool,
    challenge_consumed: bool,
    enrollment_persisted: bool,
    owner_approval_recorded: bool,
    credential_issued: bool,
    inventory_authoritative: bool,
    execution_authorized: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdentityCaseFixture {
    name: String,
    now_ms: u64,
    challenge_consumed: bool,
    device_approval_state: String,
    device_credential_state: String,
    proof: ProofFixture,
    expected: ExpectedFixture,
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
struct ExpectedFixture {
    accepted: bool,
    reason: String,
    identity_bound: bool,
    approval_required: bool,
}

#[test]
fn identity_proof_contract_fixture() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
    assert_fixture_envelope(&fixture);
    let declared_owner = owner(&fixture.owner_declaration);
    for case in fixture.cases {
        let device = DeviceIdentityBinding {
            device_id: fixture.device.device_id.clone(),
            owner: owner(&fixture.device.owner),
            key_id: fixture.device.key_id.clone(),
            public_key_sha256: fixture.device.public_key_sha256.clone(),
            approval_state: case.device_approval_state,
            credential_state: case.device_credential_state,
        };
        let challenge = IdentityChallenge {
            challenge_id: fixture.challenge.challenge_id.clone(),
            challenge_sha256: fixture.challenge.challenge_sha256.clone(),
            issued_at_ms: fixture.challenge.issued_at_ms,
            expires_at_ms: fixture.challenge.expires_at_ms,
            consumed: case.challenge_consumed,
        };
        let proof = DeviceIdentityProof {
            device_id: case.proof.device_id,
            key_id: case.proof.key_id,
            public_key_sha256: case.proof.public_key_sha256,
            owner: owner(&case.proof.owner),
            challenge_id: case.proof.challenge_id,
            challenge_sha256: case.proof.challenge_sha256,
            proof_sha256: case.proof.proof_sha256,
            issued_at_ms: case.proof.issued_at_ms,
            expires_at_ms: case.proof.expires_at_ms,
        };
        let result =
            evaluate_identity_proof(&declared_owner, &device, &challenge, &proof, case.now_ms);
        assert_case(&case.name, &case.expected, result);
    }
}

fn assert_fixture_envelope(fixture: &Fixture) {
    assert_eq!(fixture.schema_version, IDENTITY_PROOF_SCHEMA_VERSION);
    assert_eq!(fixture.evaluation_mode, IDENTITY_PROOF_EVALUATION_MODE);
    assert!(fixture.notice.contains("no cryptographic verifier"));
    assert_eq!(fixture.owner_declaration.tenant_id, "tenant-1");
    assert_eq!(fixture.device.device_id, "device-a");
    assert_eq!(fixture.device.key_id, "key-a");
    assert_eq!(fixture.device.approval_state, "approved");
    assert_eq!(fixture.device.credential_state, "active");
    assert!(!fixture.challenge.consumed);
    assert!(!fixture.authority.identity_verified);
    assert!(!fixture.authority.challenge_consumed);
    assert!(!fixture.authority.enrollment_persisted);
    assert!(!fixture.authority.owner_approval_recorded);
    assert!(!fixture.authority.credential_issued);
    assert!(!fixture.authority.inventory_authoritative);
    assert!(!fixture.authority.execution_authorized);
    assert_eq!(fixture.cases.len(), 12);
}

fn owner(value: &OwnerFixture) -> IdentityOwner {
    IdentityOwner {
        issuer: value.issuer.clone(),
        subject: value.subject.clone(),
        tenant_id: value.tenant_id.clone(),
    }
}

fn assert_case(
    name: &str,
    expected: &ExpectedFixture,
    result: Result<IdentityProofDecision, IdentityProofError>,
) {
    match (expected.accepted, result) {
        (true, Ok(decision)) => {
            assert_eq!(decision.reason(), expected.reason, "{name}");
            assert_eq!(decision.identity_bound(), expected.identity_bound, "{name}");
            assert_eq!(
                decision.approval_required(),
                expected.approval_required,
                "{name}"
            );
        }
        (false, Err(error)) => assert_eq!(error.to_string(), expected.reason, "{name}"),
        (true, Err(error)) => panic!("{name}: proof rejected: {error}"),
        (false, Ok(decision)) => panic!("{name}: proof accepted: {decision:?}"),
    }
}
