use super::receipt_fixture;
use crate::platform_core_contract::{
    AttemptState, RejectionCode, VerificationStatus, canonical_execution_receipt_json,
    decode_canonical_execution_receipt, validate_execution_receipt, validate_verification_exchange,
    validate_verification_receipt,
};

#[test]
fn execution_receipt_relations_fail_closed() {
    let mut values = Vec::new();
    let mut value = receipt_fixture().execution_receipt;
    value.session_ref.entity_id = "ses_0000000000000000000000000m".into();
    values.push(value);
    let mut value = receipt_fixture().execution_receipt;
    value.observed_usage.elapsed_ms -= 1;
    values.push(value);
    let mut value = receipt_fixture().execution_receipt;
    value.terminal_state = AttemptState::Running;
    values.push(value);
    let mut value = receipt_fixture().execution_receipt;
    value.output_artifact_refs[0].producer_attempt_id = "atm_0000000000000000000000000m".into();
    values.push(value);
    let mut value = receipt_fixture().execution_receipt;
    value.event_range.as_mut().unwrap().last_sequence = 8;
    values.push(value);
    for value in values {
        assert!(validate_execution_receipt(&value).is_err());
    }
}

#[test]
fn programmatic_execution_validation_uses_staged_rejections() {
    let mut value = receipt_fixture().execution_receipt;
    value.terminal_state = AttemptState::Running;
    value.attempt_ref.entity_id = "atm_0000000000000000000000000m".into();
    let error = validate_execution_receipt(&value).unwrap_err();
    assert_eq!(error.code, RejectionCode::ReferenceMismatch);

    let mut value = receipt_fixture().execution_receipt;
    value.observed_usage.input_tokens = -1;
    value.observed_usage.elapsed_ms -= 1;
    let error = validate_execution_receipt(&value).unwrap_err();
    assert_eq!(error.code, RejectionCode::ValueInvalid);
}

#[test]
fn verification_status_is_strictly_derived() {
    let mut value = receipt_fixture().verification_receipt;
    value.results[1].status = VerificationStatus::Fail;
    value.results[1].reason_codes = vec!["test_failed".into()];
    value.overall_status = VerificationStatus::Fail;
    validate_verification_receipt(&value).unwrap();
    value.overall_status = VerificationStatus::Pass;
    let error = validate_verification_receipt(&value).unwrap_err();
    assert_eq!(error.code, RejectionCode::RelationMismatch);
}

#[test]
fn all_not_applicable_derives_not_executed() {
    let mut value = receipt_fixture().verification_receipt;
    value.results.truncate(1);
    value.overall_status = VerificationStatus::NotExecuted;
    validate_verification_receipt(&value).unwrap();
    value.overall_status = VerificationStatus::Pass;
    assert!(validate_verification_receipt(&value).is_err());
}

#[test]
fn exchange_requires_exact_result_set() {
    let mut fixture = receipt_fixture();
    fixture.verification_receipt.results.truncate(1);
    fixture.verification_receipt.overall_status = VerificationStatus::NotExecuted;
    validate_verification_receipt(&fixture.verification_receipt).unwrap();
    let error = validate_verification_exchange(
        &fixture.verification_request,
        &fixture.verification_receipt,
    )
    .unwrap_err();
    assert_eq!(error.code, RejectionCode::RelationMismatch);
}

#[test]
fn exchange_uses_global_stage_order() {
    let mut fixture = receipt_fixture();
    fixture.verification_request.input_artifact_ref.content_id =
        format!("sha256:{}", "0".repeat(64));
    fixture.verification_receipt.overall_status = VerificationStatus::Unknown("unknown".into());
    let error = validate_verification_exchange(
        &fixture.verification_request,
        &fixture.verification_receipt,
    )
    .unwrap_err();
    assert_eq!(error.code, RejectionCode::StateInvalid);
}

#[test]
fn execution_decoder_rejects_framing_and_scalar_drift() {
    let fixture = receipt_fixture();
    let canonical = canonical_execution_receipt_json(&fixture.execution_receipt).unwrap();
    let cases = [
        format!(" {canonical}"),
        format!("{canonical}\n"),
        canonical.replacen(
            "\"execution_receipt_version\":1",
            "\"execution_receipt_version\":true",
            1,
        ),
        canonical.replacen(
            "{\"approval_ref\":",
            "{\"unexpected\":true,\"approval_ref\":",
            1,
        ),
    ];
    for raw in cases {
        assert!(decode_canonical_execution_receipt(raw.as_bytes()).is_err());
    }
}
