use super::{
    ASSESSMENT_API_VERSION, ASSESSMENT_MODE, ASSESSMENT_REQUEST_API_VERSION, ASSESSMENT_RESULT,
    CANONICALIZATION, CapabilityGrant, CapabilityGrantContractError, DeclaredAssessment,
    DeclaredAssessmentRequest, EffectVocabulary, MAX_ASSESSMENT_BYTES,
    MAX_ASSESSMENT_REQUEST_BYTES, MAX_GRANT_BYTES, MAX_VOCABULARY_BYTES, canonical, codec,
    grant_validation, invalid, primitives, scope_validation, vocabulary_validation,
};

pub(super) fn validate_vocabulary(
    value: &EffectVocabulary,
) -> Result<(), CapabilityGrantContractError> {
    canonical::encode(value, MAX_VOCABULARY_BYTES, "effect vocabulary")?;
    vocabulary_validation::validate(value)
}

pub(super) fn validate_grant(value: &CapabilityGrant) -> Result<(), CapabilityGrantContractError> {
    canonical::encode(value, MAX_GRANT_BYTES, "capability grant")?;
    grant_validation::validate(value)
}

pub(super) fn validate_assessment_request(
    value: &DeclaredAssessmentRequest,
) -> Result<(), CapabilityGrantContractError> {
    if value.api_version != ASSESSMENT_REQUEST_API_VERSION
        || value.canonicalization != CANONICALIZATION
        || value.evaluated_at_unix_ms < 0
    {
        return Err(invalid(
            "declared assessment request envelope does not match v1",
        ));
    }
    canonical::encode(
        value,
        MAX_ASSESSMENT_REQUEST_BYTES,
        "declared assessment request",
    )?;
    validate_grant(&value.grant)?;
    grant_validation::validate_bindings(&value.expected.bindings)?;
    grant_validation::validate_capability(&value.expected.capability)?;
    grant_validation::validate_principal(&value.expected.subject, "expected subject")?;
    grant_validation::validate_task_binding(&value.expected.task_binding)?;
    scope_validation::validate_action(&value.requested_action)?;
    primitives::sha256(&value.request_sha256, "assessment request_sha256")?;
    if codec::assessment_request_sha256(value)? != value.request_sha256 {
        return Err(invalid(
            "declared assessment request self digest does not match",
        ));
    }
    Ok(())
}

pub(super) fn validate_assessment_shape(
    value: &DeclaredAssessment,
) -> Result<(), CapabilityGrantContractError> {
    if value.api_version != ASSESSMENT_API_VERSION
        || value.assessment_mode != ASSESSMENT_MODE
        || value.canonicalization != CANONICALIZATION
        || value.result != ASSESSMENT_RESULT
        || value.effect_attestation
        || value.permission_attestation
    {
        return Err(invalid(
            "declared assessment authority-neutral envelope drifted",
        ));
    }
    canonical::encode(value, MAX_ASSESSMENT_BYTES, "declared assessment")?;
    for (label, digest) in [
        ("assessment_sha256", value.assessment_sha256.as_str()),
        ("grant_sha256", value.grant_sha256.as_str()),
        ("request_sha256", value.request_sha256.as_str()),
        (
            "requested_action_sha256",
            value.requested_action_sha256.as_str(),
        ),
    ] {
        primitives::sha256(digest, label)?;
    }
    validate_grant_id(&value.grant_id, &value.grant_sha256)?;
    validate_relation_shape(value)?;
    validate_reason_codes(&value.reason_codes)?;
    if value.reason_codes != super::evaluator::expected_reason_codes(&value.relations) {
        return Err(invalid("reason_codes do not match declared relations"));
    }
    if codec::assessment_sha256(value)? != value.assessment_sha256 {
        return Err(invalid("declared assessment self digest does not match"));
    }
    Ok(())
}

fn validate_relation_shape(value: &DeclaredAssessment) -> Result<(), CapabilityGrantContractError> {
    if value.relations.effect == super::EffectRelation::EffectMismatch
        && value.relations.scope != super::ScopeRelation::OutsideDeclaredScope
    {
        Err(invalid("effect_mismatch requires outside_declared_scope"))
    } else {
        Ok(())
    }
}

fn validate_grant_id(value: &str, grant_sha256: &str) -> Result<(), CapabilityGrantContractError> {
    if value == format!("capability-grant-{grant_sha256}") {
        Ok(())
    } else {
        Err(invalid(
            "assessment grant_id is not derived from grant_sha256",
        ))
    }
}

fn validate_reason_codes(values: &[super::ReasonCode]) -> Result<(), CapabilityGrantContractError> {
    if values.len() > 9
        || !values
            .windows(2)
            .all(|pair| pair[0].as_str().as_bytes() < pair[1].as_str().as_bytes())
    {
        return Err(invalid("assessment reason_codes are not strictly ordered"));
    }
    Ok(())
}
