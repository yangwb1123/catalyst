use super::super::*;
use super::fixture;

const PREIMAGE_TARGET: usize = MAX_GRANT_BYTES - 32;
const BASE_COMMAND_BYTES: usize = 4_097;

#[test]
fn full_grant_ceiling_precedes_semantic_digest_and_validation() {
    let grant = boundary_grant();
    assert!(grant_sha256(&grant).is_err());
    assert!(super::super::validation::validate_grant(&grant).is_err());
    assert!(canonical_grant_json(&grant).is_err());
}

#[test]
fn nested_oversized_grant_is_rejected_by_request_and_evaluator() {
    let fixture = fixture();
    let mut request = fixture.assessment_request;
    request.grant = boundary_grant();
    request.requested_action.effect_id = EffectId::ProcessExec;
    request.requested_action.resources = vec![command_resource(999, 16)];
    request.request_sha256 = assessment_request_sha256(&request).expect("request fits outer limit");
    assert!(super::super::validation::validate_assessment_request(&request).is_err());
    assert!(evaluate_declared_assessment(&fixture.effect_vocabulary, &request).is_err());
}

#[test]
fn every_typed_document_digest_and_validator_applies_a_full_ceiling() {
    let fixture = fixture();
    let mut vocabulary = fixture.effect_vocabulary;
    let mut definition = vocabulary.effects[0].clone();
    definition.allowed_scope_kinds = vec![ScopeKind::GovernanceObject; 256];
    vocabulary.effects = vec![definition; 256];
    assert!(effect_vocabulary_sha256(&vocabulary).is_err());
    assert!(super::super::validation::validate_vocabulary(&vocabulary).is_err());

    let mut request = fixture.assessment_request;
    request.grant = boundary_grant();
    request.requested_action.effect_id = EffectId::ProcessExec;
    request.requested_action.resources = (0..32)
        .map(|index| command_resource(200 + index, 32_768))
        .collect();
    assert!(assessment_request_sha256(&request).is_err());

    let mut assessment = fixture.expected_assessment;
    assessment.result = "x".repeat(MAX_STRING_BYTES + 1);
    assert!(assessment_sha256(&assessment).is_err());
    assert!(super::super::validation::validate_assessment_shape(&assessment).is_err());
}

fn boundary_grant() -> CapabilityGrant {
    let baseline = grant_with_commands(BASE_COMMAND_BYTES, 0);
    let baseline_bytes = grant_preimage_json(&baseline).len();
    let remaining = PREIMAGE_TARGET - baseline_bytes;
    let common_bytes = BASE_COMMAND_BYTES + remaining / 128;
    let mut grant = grant_with_commands(common_bytes, remaining % 128);
    let preimage = grant_preimage_json(&grant);
    assert_eq!(preimage.len(), PREIMAGE_TARGET);
    let digest = super::super::canonical::domain_sha256(GRANT_DIGEST_DOMAIN, preimage.as_bytes());
    grant.grant_sha256 = digest;
    grant.grant_id = format!("capability-grant-{}", grant.grant_sha256);
    let full = super::super::canonical::encode(
        &grant,
        MAX_ASSESSMENT_REQUEST_BYTES,
        "boundary capability grant",
    )
    .expect("boundary grant fits request limit");
    assert_eq!(full.len(), 1_048_717);
    grant
}

fn grant_with_commands(common_bytes: usize, first_extra: usize) -> CapabilityGrant {
    let mut grant = fixture().grant;
    let allow = (0..64)
        .map(|index| ScopeClause {
            resources: vec![command_resource(
                index,
                common_bytes + usize::from(index == 0) * first_extra,
            )],
        })
        .collect();
    let deny = (64..128)
        .map(|index| command_resource(index, common_bytes))
        .collect();
    grant.scope = GrantScope {
        allow,
        deny,
        effect_id: EffectId::ProcessExec,
    };
    grant
}

fn grant_preimage_json(grant: &CapabilityGrant) -> String {
    let mut payload = grant.clone();
    payload.grant_id.clear();
    payload.grant_sha256.clear();
    payload.authority_proof.proof_base64url.clear();
    super::super::canonical::encode(
        &payload,
        MAX_ASSESSMENT_REQUEST_BYTES,
        "boundary grant preimage",
    )
    .expect("preimage fits request ceiling")
}

fn command_resource(index: usize, payload_bytes: usize) -> ScopeResource {
    let prefix = format!("{index:04}");
    assert!(payload_bytes >= prefix.len());
    let first_bytes = payload_bytes.min(4_096);
    let mut argv = vec![format!(
        "{prefix}{}",
        "x".repeat(first_bytes - prefix.len())
    )];
    let mut remaining = payload_bytes - first_bytes;
    while remaining > 0 {
        let next = remaining.min(4_096);
        argv.push("x".repeat(next));
        remaining -= next;
    }
    ScopeResource::Command {
        argv,
        cwd: ".".into(),
        environment_sha256: "d".repeat(64),
        stdin_bytes: 0,
        stdin_sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into(),
        timeout_ms: 1_000,
        tool_snapshot_sha256: "e".repeat(64),
    }
}
