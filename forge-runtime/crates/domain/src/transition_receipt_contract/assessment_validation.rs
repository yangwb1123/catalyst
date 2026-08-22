use super::{
    ASSESSMENT_API_VERSION, ASSESSMENT_MODE, ASSESSMENT_RESULT, CANONICALIZATION,
    MAX_ASSESSMENT_BYTES, TRANSITION_VOCABULARY_SHA256, TransitionDeclaredAssessment,
    TransitionReceiptContractError, codec, invalid, primitives,
};

pub(super) fn validate_assessment_shape(
    value: &TransitionDeclaredAssessment,
) -> Result<(), TransitionReceiptContractError> {
    let envelope = value.api_version == ASSESSMENT_API_VERSION
        && value.assessment_mode == ASSESSMENT_MODE
        && value.canonicalization == CANONICALIZATION
        && value.result == ASSESSMENT_RESULT
        && value.transition_vocabulary_sha256 == TRANSITION_VOCABULARY_SHA256;
    let no_attestation = !value.completion_attestation
        && !value.effect_attestation
        && !value.execution_attestation
        && !value.permission_attestation
        && !value.persistence_attestation
        && !value.transition_attestation;
    if !envelope || !no_attestation {
        return Err(invalid(
            "Transition assessment authority-neutral envelope drifted",
        ));
    }
    validate_digests(value)?;
    validate_reason_codes(value)?;
    codec::bounded(
        value,
        MAX_ASSESSMENT_BYTES,
        "Transition declared assessment",
    )?;
    if codec::assessment_sha256_unchecked(value)? == value.assessment_sha256 {
        Ok(())
    } else {
        Err(invalid("Transition assessment self digest does not match"))
    }
}

fn validate_digests(
    value: &TransitionDeclaredAssessment,
) -> Result<(), TransitionReceiptContractError> {
    for (label, digest) in [
        ("assessment_sha256", value.assessment_sha256.as_str()),
        (
            "expected_target_sha256",
            value.expected_target_sha256.as_str(),
        ),
        ("receipt_sha256", value.receipt_sha256.as_str()),
        ("request_sha256", value.request_sha256.as_str()),
        (
            "transition_vocabulary_sha256",
            value.transition_vocabulary_sha256.as_str(),
        ),
    ] {
        primitives::sha256(digest, label)?;
    }
    if value.receipt_id == format!("transition-receipt-{}", value.receipt_sha256) {
        Ok(())
    } else {
        Err(invalid(
            "assessment TransitionReceipt identity is inconsistent",
        ))
    }
}

fn validate_reason_codes(
    value: &TransitionDeclaredAssessment,
) -> Result<(), TransitionReceiptContractError> {
    let reasons = &value.reason_codes;
    if reasons.len() > 8
        || !reasons
            .windows(2)
            .all(|pair| pair[0].as_str() < pair[1].as_str())
    {
        return Err(invalid(
            "assessment reason_codes must be strictly sorted and unique",
        ));
    }
    if *reasons == super::evaluator::expected_reason_codes(&value.relations) {
        Ok(())
    } else {
        Err(invalid("assessment reason_codes do not match relations"))
    }
}
