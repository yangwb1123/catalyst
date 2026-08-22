use super::{
    ASSESSMENT_API_VERSION, ASSESSMENT_MODE, ASSESSMENT_REQUEST_API_VERSION, ASSESSMENT_RESULT,
    ApprovalAssessmentRequest, ApprovalDeclaredAssessment, ApprovalReasonCode,
    ApprovalRecordContractError, CANONICALIZATION, MAX_ASSESSMENT_BYTES,
    MAX_ASSESSMENT_REQUEST_BYTES, codec, invalid, primitives,
};

pub(super) fn validate_request(
    value: &ApprovalAssessmentRequest,
    allow_empty_digest: bool,
) -> Result<(), ApprovalRecordContractError> {
    if value.api_version != ASSESSMENT_REQUEST_API_VERSION
        || value.canonicalization != CANONICALIZATION
    {
        return Err(invalid(
            "approval assessment request API or canonicalization drifted",
        ));
    }
    primitives::nonnegative(value.evaluated_at_unix_ms, "evaluated_at_unix_ms")?;
    super::record_validation::validate_record(&value.approval_record, false)?;
    super::record_validation::validate_target(&value.expected_target)?;
    primitives::sha256(&value.expected_target_sha256, "expected_target_sha256")?;
    if codec::declared_target_sha256(&value.expected_target)? != value.expected_target_sha256 {
        return Err(invalid("expected target digest does not match"));
    }
    validate_request_digest(value, allow_empty_digest)?;
    let encoded = codec::canonical_unbounded(value)?;
    if encoded.len() > MAX_ASSESSMENT_REQUEST_BYTES {
        Err(invalid(
            "approval assessment request exceeds its canonical byte limit",
        ))
    } else {
        Ok(())
    }
}

fn validate_request_digest(
    value: &ApprovalAssessmentRequest,
    allow_empty: bool,
) -> Result<(), ApprovalRecordContractError> {
    if allow_empty && value.request_sha256.is_empty() {
        return Ok(());
    }
    primitives::sha256(&value.request_sha256, "request_sha256")?;
    if codec::assessment_request_sha256_unchecked(value)? == value.request_sha256 {
        Ok(())
    } else {
        Err(invalid(
            "approval assessment request self digest does not match",
        ))
    }
}

pub(super) fn validate_assessment_shape(
    value: &ApprovalDeclaredAssessment,
) -> Result<(), ApprovalRecordContractError> {
    if value.api_version != ASSESSMENT_API_VERSION
        || value.canonicalization != CANONICALIZATION
        || value.assessment_mode != ASSESSMENT_MODE
        || value.result != ASSESSMENT_RESULT
        || value.effect_attestation
        || value.permission_attestation
        || value.persistence_attestation
        || value.transition_attestation
    {
        return Err(invalid(
            "approval assessment authority-neutral envelope drifted",
        ));
    }
    validate_assessment_digests(value)?;
    validate_reason_codes(&value.reason_codes, value)?;
    let encoded = codec::canonical_unbounded(value)?;
    if encoded.len() > MAX_ASSESSMENT_BYTES {
        return Err(invalid(
            "approval assessment exceeds its canonical byte limit",
        ));
    }
    if codec::assessment_sha256_unchecked(value)? != value.assessment_sha256 {
        return Err(invalid("approval assessment self digest does not match"));
    }
    Ok(())
}

fn validate_assessment_digests(
    value: &ApprovalDeclaredAssessment,
) -> Result<(), ApprovalRecordContractError> {
    for (label, digest) in [
        ("approval_sha256", value.approval_sha256.as_str()),
        ("assessment_sha256", value.assessment_sha256.as_str()),
        (
            "expected_target_sha256",
            value.expected_target_sha256.as_str(),
        ),
        ("request_sha256", value.request_sha256.as_str()),
    ] {
        primitives::sha256(digest, label)?;
    }
    if value.approval_id == format!("approval-record-{}", value.approval_sha256) {
        Ok(())
    } else {
        Err(invalid(
            "assessment ApprovalRecord identity is inconsistent",
        ))
    }
}

fn validate_reason_codes(
    values: &[ApprovalReasonCode],
    assessment: &ApprovalDeclaredAssessment,
) -> Result<(), ApprovalRecordContractError> {
    if values.len() > 11
        || !values
            .windows(2)
            .all(|pair| pair[0].as_str().as_bytes() < pair[1].as_str().as_bytes())
    {
        return Err(invalid(
            "assessment reason_codes must be strictly sorted and unique",
        ));
    }
    if values == super::evaluator::expected_reason_codes(&assessment.relations) {
        Ok(())
    } else {
        Err(invalid(
            "assessment reason_codes do not match declared relations",
        ))
    }
}
