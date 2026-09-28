use std::{
    collections::HashSet,
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use forge_runtime_domain::{
    DeviceIdentityBinding, DeviceIdentityProof, IdentityChallenge, IdentityOwner,
    IdentityProofDecision, evaluate_identity_proof,
};
use serde::{Deserialize, Serialize};

use crate::args::DeviceCommand;

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_CASE_NAME_BYTES: usize = 128;
const SCHEMA_VERSION: &str = "forge.device-identity-proof-contract/v1";
const EVALUATION_MODE: &str = "pure_binding_only";
const NOTICE: &str = "This fixture checks exact device-owner-key and one-time challenge binding only. The proof digest is a test-vector label; no cryptographic verifier, credential issuer, persistence, network, approval write, inventory authority, or execution authority is present.";
const EXPECTED_CASES: usize = 12;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    notice: String,
    owner_declaration: OwnerFixture,
    device: DeviceFixture,
    challenge: ChallengeFixture,
    authority: Authority,
    cases: Vec<Case>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct OwnerFixture {
    issuer: String,
    subject: String,
    tenant_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceFixture {
    device_id: String,
    owner: OwnerFixture,
    key_id: String,
    public_key_sha256: String,
    approval_state: String,
    credential_state: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChallengeFixture {
    challenge_id: String,
    challenge_sha256: String,
    issued_at_ms: u64,
    expires_at_ms: u64,
    consumed: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
struct Authority {
    identity_verified: bool,
    challenge_consumed: bool,
    enrollment_persisted: bool,
    owner_approval_recorded: bool,
    credential_issued: bool,
    inventory_authoritative: bool,
    execution_authorized: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    now_ms: u64,
    challenge_consumed: bool,
    device_approval_state: String,
    device_credential_state: String,
    proof: ProofFixture,
    expected: Expected,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    accepted: bool,
    reason: String,
    identity_bound: bool,
    approval_required: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct IdentityProofOutput {
    v: u16,
    #[serde(rename = "type")]
    output_type: &'static str,
    schema_version: &'static str,
    evaluation_mode: &'static str,
    device_id: String,
    owner: OwnerOutput,
    cases: Vec<CaseOutput>,
    authority: Authority,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct OwnerOutput {
    issuer: String,
    subject: String,
    tenant_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct CaseOutput {
    name: String,
    accepted: bool,
    reason: String,
    identity_bound: bool,
    approval_required: bool,
}

pub(crate) fn execute(command: &DeviceCommand) -> Result<IdentityProofOutput, Box<dyn Error>> {
    let DeviceCommand::IdentityProofPreview { input } = command else {
        return Err("device identity proof preview command is required".into());
    };
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes)
        .map_err(|error| format!("device identity proof input is invalid JSON: {error}"))?;
    let fixture: Fixture = serde_json::from_slice(&bytes)
        .map_err(|error| format!("device identity proof input is invalid JSON: {error}"))?;
    evaluate(fixture)
}

pub(crate) fn write_output(
    output: &IdentityProofOutput,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    writeln!(
        writer,
        "offline device identity proof preview [{}] device={} owner={}:{}:{}",
        output.schema_version,
        output.device_id,
        output.owner.issuer,
        output.owner.subject,
        output.owner.tenant_id
    )?;
    writeln!(
        writer,
        "evaluation: {} (binding comparison only; no cryptography, challenge consumption, persistence, network, or authority)",
        output.evaluation_mode
    )?;
    for case in &output.cases {
        writeln!(
            writer,
            "{}: accepted={} reason={} identity_bound={} approval_required={}",
            case.name, case.accepted, case.reason, case.identity_bound, case.approval_required
        )?;
    }
    writeln!(
        writer,
        "authority: identity_verified=false challenge_consumed=false enrollment_persisted=false owner_approval_recorded=false credential_issued=false inventory_authoritative=false execution_authorized=false"
    )
}

fn evaluate(fixture: Fixture) -> Result<IdentityProofOutput, Box<dyn Error>> {
    validate_fixture(&fixture)?;
    let declared_owner = owner(&fixture.owner_declaration);
    let device_owner = owner(&fixture.device.owner);
    let device_id = fixture.device.device_id.clone();
    if !matches!(
        fixture.device.approval_state.as_str(),
        "approved" | "pending"
    ) || !matches!(
        fixture.device.credential_state.as_str(),
        "active" | "revoked" | "expired"
    ) {
        return Err("device identity proof input has an invalid device state".into());
    }
    let mut names = HashSet::with_capacity(fixture.cases.len());
    let mut cases = Vec::with_capacity(fixture.cases.len());
    for case in fixture.cases {
        if case.name.trim().is_empty() || case.name.len() > MAX_CASE_NAME_BYTES {
            return Err("device identity proof case name is empty or too long".into());
        }
        if !names.insert(case.name.clone()) {
            return Err(format!("duplicate device identity proof case {:?}", case.name).into());
        }
        cases.push(evaluate_case(
            &fixture.device,
            &fixture.challenge,
            case,
            &declared_owner,
            &device_owner,
        )?);
    }
    Ok(IdentityProofOutput {
        v: 1,
        output_type: "device_identity_proof_preview",
        schema_version: SCHEMA_VERSION,
        evaluation_mode: EVALUATION_MODE,
        device_id,
        owner: OwnerOutput {
            issuer: declared_owner.issuer().to_owned(),
            subject: declared_owner.subject().to_owned(),
            tenant_id: declared_owner.tenant_id().to_owned(),
        },
        cases,
        authority: Authority::default(),
    })
}

fn evaluate_case(
    device: &DeviceFixture,
    challenge: &ChallengeFixture,
    case: Case,
    declared_owner: &IdentityOwner,
    device_owner: &IdentityOwner,
) -> Result<CaseOutput, Box<dyn Error>> {
    let binding = DeviceIdentityBinding::new(
        device.device_id.clone(),
        device_owner.clone(),
        device.key_id.clone(),
        device.public_key_sha256.clone(),
        case.device_approval_state,
        case.device_credential_state,
    );
    let identity_challenge = IdentityChallenge::new(
        challenge.challenge_id.clone(),
        challenge.challenge_sha256.clone(),
        challenge.issued_at_ms,
        challenge.expires_at_ms,
        case.challenge_consumed || challenge.consumed,
    );
    let proof = DeviceIdentityProof::new(
        case.proof.device_id,
        case.proof.key_id,
        case.proof.public_key_sha256,
        owner(&case.proof.owner),
        case.proof.challenge_id,
        case.proof.challenge_sha256,
        case.proof.proof_sha256,
        case.proof.issued_at_ms,
        case.proof.expires_at_ms,
    );
    let actual = evaluate_identity_proof(
        declared_owner,
        &binding,
        &identity_challenge,
        &proof,
        case.now_ms,
    );
    compare_case(case.name, &case.expected, actual)
}

fn validate_fixture(fixture: &Fixture) -> Result<(), Box<dyn Error>> {
    if fixture.schema_version != SCHEMA_VERSION
        || fixture.evaluation_mode != EVALUATION_MODE
        || fixture.notice != NOTICE
        || fixture.owner_declaration != fixture.device.owner
        || fixture.device.device_id != "device-a"
        || fixture.device.key_id != "key-a"
        || fixture.device.approval_state != "approved"
        || fixture.device.credential_state != "active"
        || fixture.challenge.consumed
        || fixture.cases.len() != EXPECTED_CASES
        || fixture.authority != Authority::default()
    {
        return Err("device identity proof input is not a bounded pure binding preview".into());
    }
    Ok(())
}

fn owner(value: &OwnerFixture) -> IdentityOwner {
    IdentityOwner::new(
        value.issuer.clone(),
        value.subject.clone(),
        value.tenant_id.clone(),
    )
}

fn compare_case(
    name: String,
    expected: &Expected,
    actual: Result<IdentityProofDecision, forge_runtime_domain::IdentityProofError>,
) -> Result<CaseOutput, Box<dyn Error>> {
    match (expected.accepted, actual) {
        (true, Ok(decision))
            if expected.reason == decision.reason()
                && expected.identity_bound == decision.identity_bound()
                && expected.approval_required == decision.approval_required() =>
        {
            Ok(CaseOutput {
                name,
                accepted: true,
                reason: decision.reason().to_owned(),
                identity_bound: decision.identity_bound(),
                approval_required: decision.approval_required(),
            })
        }
        (false, Err(error))
            if expected.reason == error.to_string()
                && !expected.identity_bound
                && !expected.approval_required =>
        {
            Ok(CaseOutput {
                name,
                accepted: false,
                reason: error.to_string(),
                identity_bound: false,
                approval_required: false,
            })
        }
        _ => Err(format!("device identity proof case {name:?} expectation mismatch").into()),
    }
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::with_capacity(MAX_INPUT_BYTES.min(64 * 1024));
    if input == "-" {
        io::stdin()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
    } else {
        File::open(Path::new(input))?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(format!("device identity proof input exceeds {MAX_INPUT_BYTES} bytes").into());
    }
    Ok(bytes)
}
