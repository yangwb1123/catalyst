use std::path::PathBuf;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::capability_grant_contract::{ApprovalRef, AuthorityClass, PrincipalType};

use super::{
    ApprovalAssessmentRequest, ApprovalDecision, ApprovalDeclaredAssessment, ApprovalReasonCode,
    ApprovalRecord, ApprovalRefRelation, ApprovalRequiredDistinction, ApprovalTemporalRelation,
    MAX_ASSESSMENT_BYTES, MAX_ASSESSMENT_REQUEST_BYTES, MAX_DECLARED_TARGET_BYTES,
    MAX_RECORD_BYTES, MaterialityLevel, approval_ref, approval_ref_relation, approval_sha256,
    assessment_request_sha256, assessment_sha256, canonical_approval_record_json,
    canonical_assessment_json, canonical_assessment_request_json, canonical_declared_target_json,
    declared_target, declared_target_sha256, decode_canonical_approval_record,
    decode_canonical_assessment, decode_canonical_assessment_request,
    decode_canonical_declared_target, evaluate_declared_assessment, validate_assessment,
};

const RECORD_HASH: &str = "a2c47ec0c9242d9088532ce58140643a11b3a28f43836134ed36c2c9e2ca09d4";
const TARGET_HASH: &str = "8402062537970279a1a2cff83913131656e9da341c593918281742850c646f6c";
const REQUEST_HASH: &str = "c90f6108ade8e9066e907bb09a4d5b7ace848e0b9da3be9ee718ccfbc39d9f33";
const ASSESSMENT_HASH: &str = "1719084506446d2979d4294e53f3a4541200b35d6ac103660b2861df75f786d4";
const SCHEMA_HASH: &str = "bc11d2b066bac35252bff6739798c3e30a508ed31fca0306b9cf1cdc0ef9ab64";
const FIXTURE_HASH: &str = "501320b9f65775091e67ba22c6e7faa5b5ecaa1f1b472a1a196da93c7ab81978";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GoldenFixture {
    approval_record: ApprovalRecord,
    assessment_request: ApprovalAssessmentRequest,
    expected_approval_ref: ApprovalRef,
    expected_assessment: ApprovalDeclaredAssessment,
}

fn fixture_bytes() -> Vec<u8> {
    std::fs::read(repo_root().join("docs/contracts/fixtures/approval-record-v1.json"))
        .expect("read ApprovalRecord golden")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn file_sha256(relative: &str) -> String {
    let bytes = std::fs::read(repo_root().join(relative)).expect("read frozen contract file");
    format!("{:x}", Sha256::digest(bytes))
}

fn fixture() -> GoldenFixture {
    serde_json::from_slice(&fixture_bytes()).expect("decode typed golden fixture")
}

fn reseal_request(request: &mut ApprovalAssessmentRequest) {
    request.expected_target_sha256 =
        declared_target_sha256(&request.expected_target).expect("target digest");
    request.request_sha256.clear();
    request.request_sha256 = assessment_request_sha256(request).expect("request digest");
}

fn reseal_record(record: &mut ApprovalRecord) {
    record.approval_id.clear();
    record.approval_sha256.clear();
    record.approval_sha256 = approval_sha256(record).expect("record digest");
    record.approval_id = format!("approval-record-{}", record.approval_sha256);
}

#[test]
fn golden_round_trips_all_four_documents_and_digests() {
    let fixture = fixture();
    let record_json = canonical_approval_record_json(&fixture.approval_record).unwrap();
    let target = declared_target(&fixture.approval_record).unwrap();
    let target_json = canonical_declared_target_json(&target).unwrap();
    let request_json = canonical_assessment_request_json(&fixture.assessment_request).unwrap();
    let assessment_json = canonical_assessment_json(&fixture.expected_assessment).unwrap();

    assert_eq!(
        approval_sha256(&fixture.approval_record).unwrap(),
        RECORD_HASH
    );
    assert_eq!(declared_target_sha256(&target).unwrap(), TARGET_HASH);
    assert_eq!(
        assessment_request_sha256(&fixture.assessment_request).unwrap(),
        REQUEST_HASH
    );
    assert_eq!(
        assessment_sha256(&fixture.expected_assessment).unwrap(),
        ASSESSMENT_HASH
    );
    assert_eq!(
        decode_canonical_approval_record(record_json.as_bytes()).unwrap(),
        fixture.approval_record
    );
    assert_eq!(
        decode_canonical_declared_target(target_json.as_bytes()).unwrap(),
        target
    );
    assert_eq!(
        decode_canonical_assessment_request(request_json.as_bytes()).unwrap(),
        fixture.assessment_request
    );
    assert_eq!(
        decode_canonical_assessment(assessment_json.as_bytes()).unwrap(),
        fixture.expected_assessment
    );
}

#[test]
fn frozen_schema_and_fixture_pins_match() {
    assert_eq!(
        file_sha256("docs/contracts/approval-record-v1.schema.json"),
        SCHEMA_HASH
    );
    assert_eq!(
        file_sha256("docs/contracts/fixtures/approval-record-v1.json"),
        FIXTURE_HASH
    );
}

#[test]
fn golden_reassembles_authority_neutral_assessment_and_reference() {
    let fixture = fixture();
    let assessment = evaluate_declared_assessment(&fixture.assessment_request).unwrap();
    assert_eq!(assessment, fixture.expected_assessment);
    validate_assessment(&fixture.assessment_request, &assessment).unwrap();
    assert!(!assessment.permission_attestation);
    assert!(!assessment.effect_attestation);
    assert!(!assessment.persistence_attestation);
    assert!(!assessment.transition_attestation);
    assert_eq!(
        approval_ref(&fixture.approval_record).unwrap(),
        fixture.expected_approval_ref
    );
    assert_eq!(
        approval_ref_relation(&fixture.approval_record, &fixture.expected_approval_ref).unwrap(),
        ApprovalRefRelation::SameDeclaredReference
    );
}

#[test]
fn approval_reference_match_never_upgrades_to_authority() {
    let fixture = fixture();
    let mut reference = fixture.expected_approval_ref;
    reference.authority_domain = "forgeos.different".into();
    assert_eq!(
        approval_ref_relation(&fixture.approval_record, &reference).unwrap(),
        ApprovalRefRelation::ReferenceMismatch
    );
    let assessment = evaluate_declared_assessment(&fixture.assessment_request).unwrap();
    assert_eq!(
        serde_json::to_value(assessment.authorization_decision).unwrap(),
        "none"
    );
    assert_eq!(
        serde_json::to_value(assessment.policy_decision).unwrap(),
        "none"
    );
}

#[test]
fn exact_decoder_rejects_wire_drift() {
    let record = canonical_approval_record_json(&fixture().approval_record).unwrap();
    let unknown = record.replacen("\"api_version\":", "\"alias\":0,\"api_version\":", 1);
    let duplicate = record.replacen(
        "\"api_version\":",
        "\"api_version\":\"forgeos.approval-record/v1\",\"api_version\":",
        1,
    );
    let whitespace = format!(" {record}");
    let float = record.replacen("\"trust_epoch\":1", "\"trust_epoch\":1.0", 1);
    let exponent = record.replacen("\"trust_epoch\":1", "\"trust_epoch\":1e0", 1);
    let overflow = record.replacen(
        "\"trust_epoch\":1",
        "\"trust_epoch\":9223372036854775808",
        1,
    );
    let escaped = record.replacen("fixture-revision-0059", "fixture\\u002drevision-0059", 1);
    let bidi = record.replacen("fixture-revision-0059", "fixture-revision-0059\\u202e", 1);
    for raw in [
        unknown,
        duplicate,
        whitespace,
        float,
        exponent,
        overflow,
        escaped,
        bidi,
        format!("{record}\n"),
    ] {
        assert!(decode_canonical_approval_record(raw.as_bytes()).is_err());
    }
}

#[test]
fn byte_decoders_reject_inputs_past_each_document_ceiling() {
    assert!(decode_canonical_approval_record(&vec![b' '; MAX_RECORD_BYTES + 1]).is_err());
    assert!(decode_canonical_declared_target(&vec![b' '; MAX_DECLARED_TARGET_BYTES + 1]).is_err());
    assert!(
        decode_canonical_assessment_request(&vec![b' '; MAX_ASSESSMENT_REQUEST_BYTES + 1]).is_err()
    );
    assert!(decode_canonical_assessment(&vec![b' '; MAX_ASSESSMENT_BYTES + 1]).is_err());
}

#[test]
fn programmatic_record_limits_fail_closed_without_mutation() {
    let golden = fixture();
    let original = golden.approval_record;
    let mut long = original.clone();
    long.bindings.source_revision = "x".repeat(161);
    assert!(canonical_approval_record_json(&long).is_err());
    let mut wide = original.clone();
    wide.conditions = vec![wide.conditions[0].clone(); 33];
    assert!(canonical_approval_record_json(&wide).is_err());
    let mut proof = original.clone();
    proof.authority_proof.proof_base64url = "A".repeat(16_385);
    assert!(canonical_approval_record_json(&proof).is_err());
    assert_eq!(original, fixture().approval_record);
}

#[test]
fn semantic_invariants_reject_declared_identity_and_lifecycle_drift() {
    let original = fixture().approval_record;
    let mut sod = original.clone();
    sod.separation_of_duty.requester = sod.approver.clone();
    assert!(canonical_approval_record_json(&sod).is_err());
    let mut validity = original.clone();
    validity.validity.expires_at_unix_ms = validity.validity.issued_at_unix_ms + 86_400_001;
    assert!(canonical_approval_record_json(&validity).is_err());
    let mut digest = original;
    digest.approval_sha256 = "0".repeat(64);
    assert!(canonical_approval_record_json(&digest).is_err());
}

#[test]
fn declared_target_rejects_all_four_cross_field_contradictions() {
    let target = fixture().assessment_request.expected_target;
    let mut identity = target.clone();
    identity.separation_of_duty_declaration.requester = identity.approver.clone();
    assert!(canonical_declared_target_json(&identity).is_err());

    let mut materiality = target.clone();
    materiality
        .separation_of_duty_declaration
        .implementers
        .clear();
    assert!(canonical_declared_target_json(&materiality).is_err());

    let mut risk = target.clone();
    risk.scope.materiality_level = MaterialityLevel::L2;
    risk.separation_of_duty_declaration
        .required_distinctions
        .retain(|item| *item != ApprovalRequiredDistinction::ApproverNotRequester);
    assert!(canonical_declared_target_json(&risk).is_err());

    let mut production = target;
    production
        .authority_binding
        .authority_source
        .authority_class = AuthorityClass::ForgeosKernel;
    production.authority_binding.authority_source.principal_type = PrincipalType::Service;
    assert!(canonical_declared_target_json(&production).is_err());
}

#[test]
fn relation_mismatches_are_declared_reasons_not_decisions() {
    let mut request = fixture().assessment_request;
    request.expected_target.bindings.context_sha256 = "0".repeat(64);
    request.evaluated_at_unix_ms = request.approval_record.validity.expires_at_unix_ms;
    reseal_request(&mut request);
    let assessment = evaluate_declared_assessment(&request).unwrap();
    assert!(
        assessment
            .reason_codes
            .contains(&ApprovalReasonCode::BindingMismatch)
    );
    assert!(
        assessment
            .reason_codes
            .contains(&ApprovalReasonCode::TemporalWindowMismatch)
    );
    assert_eq!(
        assessment.relations.temporal,
        ApprovalTemporalRelation::OutsideDeclaredWindow
    );
    assert!(!assessment.permission_attestation);
    assert!(!assessment.effect_attestation);
}

#[test]
fn every_declared_mismatch_remains_authority_neutral() {
    let mut request = fixture().assessment_request;
    request.approval_record.validity.revoked_at_unix_ms =
        Some(request.approval_record.validity.issued_at_unix_ms);
    reseal_record(&mut request.approval_record);
    mutate_every_target_relation(&mut request);
    request.evaluated_at_unix_ms = request.approval_record.validity.expires_at_unix_ms;
    reseal_request(&mut request);
    let assessment = evaluate_declared_assessment(&request).unwrap();
    assert_eq!(assessment.reason_codes, all_reason_codes());
    assert!(!assessment.permission_attestation);
    assert!(!assessment.effect_attestation);
    assert!(!assessment.persistence_attestation);
    assert!(!assessment.transition_attestation);
    assert_eq!(
        serde_json::to_value(assessment.policy_decision).unwrap(),
        "none"
    );
    assert_eq!(
        serde_json::to_value(assessment.authorization_decision).unwrap(),
        "none"
    );
}

fn mutate_every_target_relation(request: &mut ApprovalAssessmentRequest) {
    let target = &mut request.expected_target;
    target.approver.principal_id = "different-approver".into();
    target.authority_binding.key_id = "different-key".into();
    target.bindings.source_revision = "different-revision".into();
    target.conditions.clear();
    target.decision = ApprovalDecision::Reject;
    target.risk_acceptance_refs.clear();
    target.scope.change_id = "different-change".into();
    target.separation_of_duty_declaration.requester.principal_id = "different-requester".into();
    target.subject.principal_id = "different-subject".into();
}

fn all_reason_codes() -> Vec<ApprovalReasonCode> {
    vec![
        ApprovalReasonCode::ApproverMismatch,
        ApprovalReasonCode::AuthorityBindingMismatch,
        ApprovalReasonCode::BindingMismatch,
        ApprovalReasonCode::ConditionsMismatch,
        ApprovalReasonCode::DecisionMismatch,
        ApprovalReasonCode::DeclaredRevocationTimeReached,
        ApprovalReasonCode::RiskAcceptanceMismatch,
        ApprovalReasonCode::ScopeMismatch,
        ApprovalReasonCode::SeparationOfDutyMismatch,
        ApprovalReasonCode::SubjectMismatch,
        ApprovalReasonCode::TemporalWindowMismatch,
    ]
}

#[test]
fn digest_preimages_have_frozen_proof_and_self_field_rules() {
    let fixture = fixture();
    let mut record = fixture.approval_record;
    record.authority_proof.proof_base64url = "ZGlmZmVyZW50LXByb29mLWJ5dGVz".into();
    record.separation_of_duty.proof_base64url = "ZGlmZmVyZW50LXNvZC1wcm9vZg".into();
    assert_eq!(approval_sha256(&record).unwrap(), RECORD_HASH);
    let mut request = fixture.assessment_request;
    request.approval_record.authority_proof.proof_base64url = "ZGlmZmVyZW50LXByb29mLWJ5dGVz".into();
    request.request_sha256.clear();
    assert_ne!(assessment_request_sha256(&request).unwrap(), REQUEST_HASH);
}

#[test]
fn assessment_shape_cannot_claim_authority() {
    let mut assessment = fixture().expected_assessment;
    assessment.permission_attestation = true;
    assessment.assessment_sha256.clear();
    assert!(canonical_assessment_json(&assessment).is_err());
}

#[test]
fn all_self_digests_and_target_binding_reject_tampering() {
    let golden = fixture();
    let mut record = golden.approval_record;
    record.approval_sha256 = "f".repeat(64);
    assert!(canonical_approval_record_json(&record).is_err());
    let mut request = golden.assessment_request;
    request.request_sha256 = "f".repeat(64);
    assert!(canonical_assessment_request_json(&request).is_err());
    request.expected_target_sha256 = "e".repeat(64);
    assert!(canonical_assessment_request_json(&request).is_err());
    let mut assessment = golden.expected_assessment;
    assessment.assessment_sha256 = "f".repeat(64);
    assert!(canonical_assessment_json(&assessment).is_err());
}

#[test]
fn approval_ref_rejects_identity_substitution_and_accepts_valid_mismatch() {
    let golden = fixture();
    let mut inconsistent = golden.expected_approval_ref.clone();
    inconsistent.approval_sha256 = "d".repeat(64);
    assert!(approval_ref_relation(&golden.approval_record, &inconsistent).is_err());
    inconsistent.approval_id = format!("approval-record-{}", inconsistent.approval_sha256);
    assert_eq!(
        approval_ref_relation(&golden.approval_record, &inconsistent).unwrap(),
        ApprovalRefRelation::ReferenceMismatch
    );
}

#[test]
fn assessment_digest_bounds_full_input_before_blanking_self_digest() {
    let mut assessment = fixture().expected_assessment;
    assessment.assessment_sha256 = "a".repeat(super::MAX_STRING_BYTES + 1);
    assert!(assessment_sha256(&assessment).is_err());
}
