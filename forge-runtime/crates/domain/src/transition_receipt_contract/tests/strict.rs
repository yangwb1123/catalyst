use super::super::{
    ApplicabilityDecision, EdgeRelation, TransitionReasonCode, TransitionState, assessment_sha256,
    canonical_assessment_json, canonical_assessment_request_json, decode_canonical_assessment,
    decode_canonical_assessment_request, evaluate_declared_assessment, validate_assessment,
};
use super::{fixture, reseal_receipt};

#[test]
fn intrinsic_sequence_and_predecessor_invariants_fail_closed() {
    let mut receipt = fixture().transition_receipt;
    receipt.previous_receipt_sha256 = Some("a".repeat(64));
    receipt.previous_receipt_id = Some(format!("transition-receipt-{}", "a".repeat(64)));
    assert!(super::super::receipt_sha256(&receipt).is_err());
    let mut receipt = fixture().transition_receipt;
    receipt.sequence = 2;
    assert!(super::super::receipt_sha256(&receipt).is_err());
    let mut receipt = fixture().transition_receipt;
    receipt.transition.from_state = TransitionState::Baselined;
    assert!(super::super::receipt_sha256(&receipt).is_err());
}

#[test]
fn intrinsic_applicability_and_recovery_invariants_fail_closed() {
    let mut receipt = fixture().transition_receipt;
    receipt.applicability.stage_id = "BASELINED".into();
    assert!(super::super::receipt_sha256(&receipt).is_err());
    let mut receipt = fixture().transition_receipt;
    receipt.applicability.decision = ApplicabilityDecision::NotApplicable;
    assert!(super::super::receipt_sha256(&receipt).is_err());
    let mut receipt = fixture().transition_receipt;
    receipt.transition.resume_state = Some(TransitionState::Draft);
    assert!(super::super::receipt_sha256(&receipt).is_err());
    let mut receipt = fixture().transition_receipt;
    receipt.transition.rework_target = Some(TransitionState::Verifying);
    assert!(super::super::receipt_sha256(&receipt).is_err());

    let mut request = fixture().assessment_request;
    request.transition_receipt.applicability.stage_id = "BASELINED".into();
    assert!(evaluate_declared_assessment(&request).is_err());
}

#[test]
fn valid_not_applicable_requires_reason_and_evidence() {
    let mut receipt = fixture().transition_receipt;
    receipt.applicability.decision = ApplicabilityDecision::NotApplicable;
    receipt.applicability.reason_codes = vec!["stage_not_required".into()];
    receipt.applicability.evidence_refs = receipt.preconditions[0].evidence_refs.clone();
    reseal_receipt(&mut receipt);
    assert!(super::super::canonical_receipt_json(&receipt).is_ok());
}

#[test]
fn aliases_unknown_fields_duplicates_and_noncanonical_bytes_fail_closed() {
    let request = fixture().assessment_request;
    let canonical = canonical_assessment_request_json(&request).unwrap();
    let alias = canonical.replace(
        "\"kind\":\"TransitionReceipt\"",
        "\"kind\":\"WorkflowReceipt\"",
    );
    assert!(decode_canonical_assessment_request(alias.as_bytes()).is_err());
    let unknown = canonical.replacen('{', "{\"approved\":true,", 1);
    assert!(decode_canonical_assessment_request(unknown.as_bytes()).is_err());
    let duplicate = canonical.replacen(
        "{\"api_version\":",
        "{\"api_version\":\"duplicate\",\"api_version\":",
        1,
    );
    assert!(decode_canonical_assessment_request(duplicate.as_bytes()).is_err());
    let pretty = serde_json::to_string_pretty(&request).unwrap();
    assert!(decode_canonical_assessment_request(pretty.as_bytes()).is_err());
}

#[test]
fn floats_bidi_controls_and_digest_rewrite_fail_closed() {
    let request = fixture().assessment_request;
    let canonical = canonical_assessment_request_json(&request).unwrap();
    let float = canonical.replace(
        "\"evaluated_at_unix_ms\":1700000001000",
        "\"evaluated_at_unix_ms\":1.0",
    );
    assert!(decode_canonical_assessment_request(float.as_bytes()).is_err());
    let bidi = canonical.replace("fixture-revision-1", "fixture-\\u202erevision-1");
    assert!(decode_canonical_assessment_request(bidi.as_bytes()).is_err());
    let mut receipt = fixture().transition_receipt;
    receipt.bindings.context_sha256 = "d".repeat(64);
    assert!(super::super::canonical_receipt_json(&receipt).is_err());
}

#[test]
fn assessment_authority_escalation_and_reassembled_drift_fail_closed() {
    let golden = fixture();
    let mut escalated = golden.expected_assessment.clone();
    escalated.transition_attestation = true;
    escalated.assessment_sha256 = assessment_sha256(&escalated).unwrap();
    assert!(canonical_assessment_json(&escalated).is_err());

    let mut drifted = golden.expected_assessment.clone();
    drifted.relations.edge = EdgeRelation::UnlistedDeclaredEdge;
    drifted.reason_codes = vec![TransitionReasonCode::UnlistedDeclaredEdge];
    drifted.assessment_sha256 = assessment_sha256(&drifted).unwrap();
    assert!(canonical_assessment_json(&drifted).is_ok());
    assert!(validate_assessment(&golden.assessment_request, &drifted).is_err());
}

#[test]
fn applicability_mismatch_is_not_an_assessment_wire_value() {
    let assessment = canonical_assessment_json(&fixture().expected_assessment).unwrap();
    let mismatch = assessment.replace(
        "internally_consistent_declared_applicability",
        "applicability_declaration_mismatch",
    );
    assert!(decode_canonical_assessment(mismatch.as_bytes()).is_err());
}
