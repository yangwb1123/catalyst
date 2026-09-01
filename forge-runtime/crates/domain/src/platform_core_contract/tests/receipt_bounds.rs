use crate::platform_core_contract::{
    AttemptState, MAX_EVIDENCE_REFS, MAX_REASON_CODES, MAX_RECEIPT_ARTIFACTS,
    MAX_VERIFICATION_CHECKS, RecordRef, RejectionCode, VerificationApplicability,
    VerificationCheckRequest, VerificationCheckResult, VerificationReceipt, VerificationStatus,
    validate_execution_receipt, validate_verification_exchange, validate_verification_receipt,
    validate_verification_request,
};

use super::receipt_fixture;

#[test]
fn verification_check_count_boundary_is_exact() {
    let mut request = receipt_fixture().verification_request;
    request.checks = (0..MAX_VERIFICATION_CHECKS)
        .map(|index| VerificationCheckRequest {
            check_id: format!("check_{index:02}"),
            check_name: "forge.check.test".into(),
            declared_required: true,
        })
        .collect();
    validate_verification_request(&request).unwrap();
    request.checks.push(VerificationCheckRequest {
        check_id: "check_extra".into(),
        check_name: "forge.check.test".into(),
        declared_required: true,
    });
    assert!(validate_verification_request(&request).is_err());
}

#[test]
fn execution_artifact_count_boundary_is_exact() {
    let mut receipt = receipt_fixture().execution_receipt;
    let source = receipt.output_artifact_refs[0].clone();
    receipt.output_artifact_refs = (0..MAX_RECEIPT_ARTIFACTS)
        .map(|index| numbered_artifact(&source, index))
        .collect();
    validate_execution_receipt(&receipt).unwrap();
    receipt
        .output_artifact_refs
        .push(numbered_artifact(&source, MAX_RECEIPT_ARTIFACTS));
    assert!(validate_execution_receipt(&receipt).is_err());
}

#[test]
fn execution_reason_count_boundary_is_exact() {
    let mut receipt = receipt_fixture().execution_receipt;
    receipt.terminal_state = AttemptState::Failed;
    receipt.reason_codes = (0..MAX_REASON_CODES)
        .map(|index| format!("reason_{index:02}"))
        .collect();
    validate_execution_receipt(&receipt).unwrap();
    receipt.reason_codes.push("reason_extra".into());
    assert!(validate_execution_receipt(&receipt).is_err());
}

#[test]
fn verification_receipt_whole_document_bound_precedes_state() {
    let mut receipt = oversized_verification_receipt();
    receipt.overall_status = VerificationStatus::Unknown("unknown".into());
    let error = validate_verification_receipt(&receipt).unwrap_err();
    assert_eq!(error.code, RejectionCode::ValueInvalid);
}

#[test]
fn verification_exchange_bounds_both_documents_before_values() {
    let mut request = receipt_fixture().verification_request;
    request.verification_id = "atm_0000000000000000000000000g".into();
    let receipt = oversized_verification_receipt();
    let error = validate_verification_exchange(&request, &receipt).unwrap_err();
    assert_eq!(error.code, RejectionCode::ValueInvalid);
}

fn oversized_verification_receipt() -> VerificationReceipt {
    let mut receipt = receipt_fixture().verification_receipt;
    let evidence_refs = maximal_evidence_refs();
    receipt.results = (0..MAX_VERIFICATION_CHECKS)
        .map(|index| VerificationCheckResult {
            applicability: VerificationApplicability::Applicable,
            applicability_reason: None,
            check_id: format!("check_{index:02}"),
            evidence_refs: evidence_refs.clone(),
            reason_codes: vec![],
            status: VerificationStatus::Pass,
        })
        .collect();
    receipt
}

fn maximal_evidence_refs() -> Vec<RecordRef> {
    let record_type = format!(
        "{}.{}.{}.{}",
        "a".repeat(31),
        "b".repeat(31),
        "c".repeat(31),
        "d".repeat(32)
    );
    (0..MAX_EVIDENCE_REFS)
        .map(|index| {
            let prefix = format!("evidence_{index:04}_");
            RecordRef {
                record_id: format!("{prefix}{}", "a".repeat(160 - prefix.len())),
                record_sha256: "a".repeat(64),
                record_type: record_type.clone(),
            }
        })
        .collect()
}

fn numbered_artifact(
    source: &crate::platform_core_contract::ArtifactRef,
    number: usize,
) -> crate::platform_core_contract::ArtifactRef {
    const ALPHABET: &[u8] = b"0123456789abcdefghjkmnpqrstvwxyz";
    let mut value = source.clone();
    value.logical_id = format!(
        "art_{}{}{}",
        "0".repeat(24),
        char::from(ALPHABET[number / ALPHABET.len()]),
        char::from(ALPHABET[number % ALPHABET.len()])
    );
    value
}
