use serde::{Serialize, de::DeserializeOwned};

use super::{
    ASSESSMENT_DIGEST_DOMAIN, ASSESSMENT_REQUEST_DIGEST_DOMAIN, DECLARED_TARGET_DIGEST_DOMAIN,
    KnowledgeUpdateAssessmentRequest, KnowledgeUpdateDeclaredAssessment,
    KnowledgeUpdateDeclaredTarget, KnowledgeUpdateProposal, KnowledgeUpdateProposalContractError,
    MAX_ASSESSMENT_BYTES, MAX_ASSESSMENT_REQUEST_BYTES, MAX_DECLARED_TARGET_BYTES,
    MAX_PROPOSAL_BYTES, PROPOSAL_DIGEST_DOMAIN, RECORD_SET_DIGEST_DOMAIN, canonical, invalid,
    validation,
};

/// Decodes exact compact canonical `KnowledgeUpdateProposal` JSON.
///
/// # Errors
///
/// Returns an error for malformed, non-canonical, oversized, or invalid input.
pub fn decode_canonical_proposal(
    bytes: &[u8],
) -> Result<KnowledgeUpdateProposal, KnowledgeUpdateProposalContractError> {
    decode(
        bytes,
        MAX_PROPOSAL_BYTES,
        "knowledge update proposal",
        validation::validate_proposal,
    )
}

/// Encodes one validated proposal as exact compact canonical JSON.
///
/// # Errors
///
/// Returns an error when the proposal is invalid or exceeds resource bounds.
pub fn canonical_proposal_json(
    value: &KnowledgeUpdateProposal,
) -> Result<String, KnowledgeUpdateProposalContractError> {
    validation::validate_proposal(value)?;
    canonical::encode(value, MAX_PROPOSAL_BYTES, "knowledge update proposal")
}

/// Decodes one exact records-free declared target.
///
/// # Errors
///
/// Returns an error for malformed, non-canonical, oversized, or invalid input.
pub fn decode_canonical_declared_target(
    bytes: &[u8],
) -> Result<KnowledgeUpdateDeclaredTarget, KnowledgeUpdateProposalContractError> {
    decode(
        bytes,
        MAX_DECLARED_TARGET_BYTES,
        "knowledge update declared target",
        validation::validate_target,
    )
}

/// Encodes one validated records-free declared target.
///
/// # Errors
///
/// Returns an error when the target is invalid or exceeds resource bounds.
pub fn canonical_declared_target_json(
    value: &KnowledgeUpdateDeclaredTarget,
) -> Result<String, KnowledgeUpdateProposalContractError> {
    validation::validate_target(value)?;
    canonical::encode(
        value,
        MAX_DECLARED_TARGET_BYTES,
        "knowledge update declared target",
    )
}

/// Decodes one exact declared-assessment request.
///
/// # Errors
///
/// Returns an error for malformed, non-canonical, oversized, or invalid input.
pub fn decode_canonical_assessment_request(
    bytes: &[u8],
) -> Result<KnowledgeUpdateAssessmentRequest, KnowledgeUpdateProposalContractError> {
    decode(
        bytes,
        MAX_ASSESSMENT_REQUEST_BYTES,
        "knowledge update assessment request",
        validation::validate_assessment_request,
    )
}

/// Encodes one validated declared-assessment request.
///
/// # Errors
///
/// Returns an error when the request is invalid or exceeds resource bounds.
pub fn canonical_assessment_request_json(
    value: &KnowledgeUpdateAssessmentRequest,
) -> Result<String, KnowledgeUpdateProposalContractError> {
    validation::validate_assessment_request(value)?;
    canonical::encode(
        value,
        MAX_ASSESSMENT_REQUEST_BYTES,
        "knowledge update assessment request",
    )
}

/// Decodes one exact authority-neutral declared assessment.
///
/// # Errors
///
/// Returns an error for malformed, non-canonical, oversized, or invalid input.
pub fn decode_canonical_assessment(
    bytes: &[u8],
) -> Result<KnowledgeUpdateDeclaredAssessment, KnowledgeUpdateProposalContractError> {
    decode(
        bytes,
        MAX_ASSESSMENT_BYTES,
        "knowledge update declared assessment",
        validation::validate_assessment_shape,
    )
}

/// Encodes one shape-valid authority-neutral declared assessment.
///
/// # Errors
///
/// Returns an error when the assessment is invalid or exceeds resource bounds.
pub fn canonical_assessment_json(
    value: &KnowledgeUpdateDeclaredAssessment,
) -> Result<String, KnowledgeUpdateProposalContractError> {
    validation::validate_assessment_shape(value)?;
    canonical::encode(
        value,
        MAX_ASSESSMENT_BYTES,
        "knowledge update declared assessment",
    )
}

/// Computes the exact ADR-0045 canonical record-set digest.
///
/// # Errors
///
/// Returns an error when the embedded record set cannot be canonically encoded.
pub fn record_set_sha256(
    value: &KnowledgeUpdateProposal,
) -> Result<String, KnowledgeUpdateProposalContractError> {
    crate::governance_contract::validate_record_set(&value.records)
        .map_err(|error| invalid(format!("proposal records: {}", error.message)))?;
    let encoded = crate::governance_contract::codec::canonical_record_set_json(&value.records)
        .map_err(|error| invalid(format!("proposal records: {}", error.message)))?;
    Ok(canonical::domain_sha256(
        RECORD_SET_DIGEST_DOMAIN,
        encoded.as_bytes(),
    ))
}

/// Computes proposal content identity after blanking only its ID and self digest.
///
/// # Errors
///
/// Returns an error when the proposal or digest preimage cannot be canonically encoded.
pub fn proposal_sha256(
    value: &KnowledgeUpdateProposal,
) -> Result<String, KnowledgeUpdateProposalContractError> {
    validation::validate_proposal(value)?;
    proposal_sha256_unchecked(value)
}

pub(super) fn proposal_sha256_unchecked(
    value: &KnowledgeUpdateProposal,
) -> Result<String, KnowledgeUpdateProposalContractError> {
    canonical::encode(value, MAX_PROPOSAL_BYTES, "knowledge update proposal")?;
    let mut payload = value.clone();
    payload.proposal_id.clear();
    payload.proposal_sha256.clear();
    let encoded = canonical::encode(&payload, MAX_PROPOSAL_BYTES, "proposal digest preimage")?;
    Ok(canonical::domain_sha256(
        PROPOSAL_DIGEST_DOMAIN,
        encoded.as_bytes(),
    ))
}

/// Computes content identity for the records-free declared target projection.
///
/// # Errors
///
/// Returns an error when the target cannot be canonically encoded within its bound.
pub fn declared_target_sha256(
    value: &KnowledgeUpdateDeclaredTarget,
) -> Result<String, KnowledgeUpdateProposalContractError> {
    validation::validate_target(value)?;
    let encoded = canonical::encode(value, MAX_DECLARED_TARGET_BYTES, "declared target")?;
    Ok(canonical::domain_sha256(
        DECLARED_TARGET_DIGEST_DOMAIN,
        encoded.as_bytes(),
    ))
}

/// Computes request identity after blanking only `request_sha256`.
///
/// # Errors
///
/// Returns an error when the request or digest preimage cannot be canonically encoded.
pub fn assessment_request_sha256(
    value: &KnowledgeUpdateAssessmentRequest,
) -> Result<String, KnowledgeUpdateProposalContractError> {
    validation::validate_assessment_request(value)?;
    assessment_request_sha256_unchecked(value)
}

pub(super) fn assessment_request_sha256_unchecked(
    value: &KnowledgeUpdateAssessmentRequest,
) -> Result<String, KnowledgeUpdateProposalContractError> {
    canonical::encode(
        value,
        MAX_ASSESSMENT_REQUEST_BYTES,
        "knowledge update assessment request",
    )?;
    let mut payload = value.clone();
    payload.request_sha256.clear();
    let encoded = canonical::encode(
        &payload,
        MAX_ASSESSMENT_REQUEST_BYTES,
        "assessment request digest preimage",
    )?;
    Ok(canonical::domain_sha256(
        ASSESSMENT_REQUEST_DIGEST_DOMAIN,
        encoded.as_bytes(),
    ))
}

/// Computes assessment identity after blanking only `assessment_sha256`.
///
/// # Errors
///
/// Returns an error when the assessment or digest preimage cannot be canonically encoded.
pub fn assessment_sha256(
    value: &KnowledgeUpdateDeclaredAssessment,
) -> Result<String, KnowledgeUpdateProposalContractError> {
    canonical::encode(
        value,
        MAX_ASSESSMENT_BYTES,
        "knowledge update declared assessment",
    )?;
    let mut payload = value.clone();
    payload.assessment_sha256.clear();
    let encoded = canonical::encode(&payload, MAX_ASSESSMENT_BYTES, "assessment digest preimage")?;
    Ok(canonical::domain_sha256(
        ASSESSMENT_DIGEST_DOMAIN,
        encoded.as_bytes(),
    ))
}

fn decode<T>(
    bytes: &[u8],
    maximum: usize,
    label: &str,
    validate: impl FnOnce(&T) -> Result<(), KnowledgeUpdateProposalContractError>,
) -> Result<T, KnowledgeUpdateProposalContractError>
where
    T: DeserializeOwned + Serialize,
{
    if bytes.len() > maximum {
        return Err(invalid(format!("{label} exceeds {maximum} bytes")));
    }
    let value = serde_json::from_slice(bytes)
        .map_err(|error| invalid(format!("{label} is invalid JSON: {error}")))?;
    validate(&value)?;
    let encoded = canonical::encode(&value, maximum, label)?;
    if encoded.as_bytes() != bytes {
        return Err(invalid(format!(
            "{label} is not exact compact canonical JSON"
        )));
    }
    Ok(value)
}
