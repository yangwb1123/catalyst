use super::{
    ASSESSMENT_DIGEST_DOMAIN, ASSESSMENT_REQUEST_DIGEST_DOMAIN, CapabilityGrant,
    CapabilityGrantContractError, DeclaredAssessment, DeclaredAssessmentRequest,
    EFFECT_VOCABULARY_DIGEST_DOMAIN, EffectVocabulary, GRANT_DIGEST_DOMAIN, MAX_ASSESSMENT_BYTES,
    MAX_ASSESSMENT_REQUEST_BYTES, MAX_GRANT_BYTES, MAX_VOCABULARY_BYTES,
    REQUESTED_ACTION_DIGEST_DOMAIN, RequestedAction, canonical, invalid, validation,
};

/// Decodes exact compact canonical effect-vocabulary JSON.
///
/// # Errors
///
/// Returns an error for malformed, non-canonical, oversized, or invalid input.
pub fn decode_canonical_effect_vocabulary(
    bytes: &[u8],
) -> Result<EffectVocabulary, CapabilityGrantContractError> {
    decode(bytes, MAX_VOCABULARY_BYTES, "effect vocabulary", |value| {
        validation::validate_vocabulary(value)
    })
}

/// Encodes one validated effect vocabulary as exact compact canonical JSON.
///
/// # Errors
///
/// Returns an error when the vocabulary is invalid or exceeds resource limits.
pub fn canonical_effect_vocabulary_json(
    value: &EffectVocabulary,
) -> Result<String, CapabilityGrantContractError> {
    validation::validate_vocabulary(value)?;
    canonical::encode(value, MAX_VOCABULARY_BYTES, "effect vocabulary")
}

/// Decodes exact compact canonical `CapabilityGrant` JSON.
///
/// # Errors
///
/// Returns an error for malformed, non-canonical, oversized, or invalid input.
pub fn decode_canonical_grant(
    bytes: &[u8],
) -> Result<CapabilityGrant, CapabilityGrantContractError> {
    decode(bytes, MAX_GRANT_BYTES, "capability grant", |value| {
        validation::validate_grant(value)
    })
}

/// Encodes one validated `CapabilityGrant` as exact compact canonical JSON.
///
/// # Errors
///
/// Returns an error when the Grant is invalid or exceeds resource limits.
pub fn canonical_grant_json(
    value: &CapabilityGrant,
) -> Result<String, CapabilityGrantContractError> {
    validation::validate_grant(value)?;
    canonical::encode(value, MAX_GRANT_BYTES, "capability grant")
}

/// Decodes exact compact canonical declared-assessment request JSON.
///
/// # Errors
///
/// Returns an error for malformed, non-canonical, oversized, or invalid input.
pub fn decode_canonical_assessment_request(
    bytes: &[u8],
) -> Result<DeclaredAssessmentRequest, CapabilityGrantContractError> {
    decode(
        bytes,
        MAX_ASSESSMENT_REQUEST_BYTES,
        "declared assessment request",
        validation::validate_assessment_request,
    )
}

/// Encodes a validated declared-assessment request as canonical JSON.
///
/// # Errors
///
/// Returns an error when the request is invalid or exceeds resource limits.
pub fn canonical_assessment_request_json(
    value: &DeclaredAssessmentRequest,
) -> Result<String, CapabilityGrantContractError> {
    validation::validate_assessment_request(value)?;
    canonical::encode(
        value,
        MAX_ASSESSMENT_REQUEST_BYTES,
        "declared assessment request",
    )
}

/// Decodes exact compact canonical authority-neutral assessment JSON.
///
/// # Errors
///
/// Returns an error for malformed, non-canonical, oversized, or invalid input.
pub fn decode_canonical_assessment(
    bytes: &[u8],
) -> Result<DeclaredAssessment, CapabilityGrantContractError> {
    decode(
        bytes,
        MAX_ASSESSMENT_BYTES,
        "declared assessment",
        validation::validate_assessment_shape,
    )
}

/// Encodes one shape-valid authority-neutral assessment as canonical JSON.
///
/// # Errors
///
/// Returns an error when the assessment is invalid or exceeds resource limits.
pub fn canonical_assessment_json(
    value: &DeclaredAssessment,
) -> Result<String, CapabilityGrantContractError> {
    validation::validate_assessment_shape(value)?;
    canonical::encode(value, MAX_ASSESSMENT_BYTES, "declared assessment")
}

/// Computes the frozen effect-vocabulary self digest.
///
/// # Errors
///
/// Returns an error when the vocabulary cannot be canonically encoded.
pub fn effect_vocabulary_sha256(
    value: &EffectVocabulary,
) -> Result<String, CapabilityGrantContractError> {
    canonical::encode(value, MAX_VOCABULARY_BYTES, "effect vocabulary")?;
    let mut payload = value.clone();
    payload.vocabulary_sha256.clear();
    let encoded = canonical::encode(&payload, MAX_VOCABULARY_BYTES, "effect vocabulary")?;
    Ok(canonical::domain_sha256(
        EFFECT_VOCABULARY_DIGEST_DOMAIN,
        encoded.as_bytes(),
    ))
}

/// Computes the `CapabilityGrant` semantic self digest.
///
/// # Errors
///
/// Returns an error when the Grant cannot be canonically encoded.
pub fn grant_sha256(value: &CapabilityGrant) -> Result<String, CapabilityGrantContractError> {
    canonical::encode(value, MAX_GRANT_BYTES, "capability grant")?;
    let mut payload = value.clone();
    payload.grant_id.clear();
    payload.grant_sha256.clear();
    payload.authority_proof.proof_base64url.clear();
    let encoded = canonical::encode(&payload, MAX_GRANT_BYTES, "capability grant")?;
    Ok(canonical::domain_sha256(
        GRANT_DIGEST_DOMAIN,
        encoded.as_bytes(),
    ))
}

/// Computes the canonical requested-action digest.
///
/// # Errors
///
/// Returns an error when the requested action cannot be canonically encoded.
pub fn requested_action_sha256(
    value: &RequestedAction,
) -> Result<String, CapabilityGrantContractError> {
    let encoded = canonical::encode(value, MAX_GRANT_BYTES, "requested action")?;
    Ok(canonical::domain_sha256(
        REQUESTED_ACTION_DIGEST_DOMAIN,
        encoded.as_bytes(),
    ))
}

/// Computes the declared-assessment request self digest.
///
/// # Errors
///
/// Returns an error when the request cannot be canonically encoded.
pub fn assessment_request_sha256(
    value: &DeclaredAssessmentRequest,
) -> Result<String, CapabilityGrantContractError> {
    canonical::encode(
        value,
        MAX_ASSESSMENT_REQUEST_BYTES,
        "declared assessment request",
    )?;
    let mut payload = value.clone();
    payload.request_sha256.clear();
    let encoded = canonical::encode(
        &payload,
        MAX_ASSESSMENT_REQUEST_BYTES,
        "declared assessment request",
    )?;
    Ok(canonical::domain_sha256(
        ASSESSMENT_REQUEST_DIGEST_DOMAIN,
        encoded.as_bytes(),
    ))
}

/// Computes the authority-neutral declared-assessment self digest.
///
/// # Errors
///
/// Returns an error when the assessment cannot be canonically encoded.
pub fn assessment_sha256(
    value: &DeclaredAssessment,
) -> Result<String, CapabilityGrantContractError> {
    canonical::encode(value, MAX_ASSESSMENT_BYTES, "declared assessment")?;
    let mut payload = value.clone();
    payload.assessment_sha256.clear();
    let encoded = canonical::encode(&payload, MAX_ASSESSMENT_BYTES, "declared assessment")?;
    Ok(canonical::domain_sha256(
        ASSESSMENT_DIGEST_DOMAIN,
        encoded.as_bytes(),
    ))
}

fn decode<T>(
    bytes: &[u8],
    maximum: usize,
    label: &str,
    validate: impl FnOnce(&T) -> Result<(), CapabilityGrantContractError>,
) -> Result<T, CapabilityGrantContractError>
where
    T: serde::de::DeserializeOwned + serde::Serialize,
{
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(invalid(format!(
            "{label} violates the canonical byte limit"
        )));
    }
    let value: T = serde_json::from_slice(bytes)
        .map_err(|error| invalid(format!("{label} is invalid JSON: {error}")))?;
    validate(&value)?;
    let encoded = canonical::encode(&value, maximum, label)?;
    canonical::require_exact(bytes, &encoded, label)?;
    Ok(value)
}
