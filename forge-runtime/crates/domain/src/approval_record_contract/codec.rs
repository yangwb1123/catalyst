use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

use crate::governance_contract::codec::{canonical_json, lower_hex};

use super::{
    APPROVAL_DIGEST_DOMAIN, ASSESSMENT_DIGEST_DOMAIN, ASSESSMENT_REQUEST_DIGEST_DOMAIN,
    ApprovalAssessmentRequest, ApprovalDeclaredAssessment, ApprovalDeclaredTarget, ApprovalRecord,
    ApprovalRecordContractError, DECLARED_TARGET_DIGEST_DOMAIN, MAX_ASSESSMENT_BYTES,
    MAX_ASSESSMENT_REQUEST_BYTES, MAX_DECLARED_TARGET_BYTES, MAX_RECORD_BYTES, invalid,
};

/// Decodes exact compact canonical `ApprovalRecord` JSON.
///
/// # Errors
///
/// Returns an error for malformed, non-canonical, oversized, or invalid input.
pub fn decode_canonical_approval_record(
    bytes: &[u8],
) -> Result<ApprovalRecord, ApprovalRecordContractError> {
    decode(bytes, MAX_RECORD_BYTES, "ApprovalRecord", |value| {
        super::record_validation::validate_record(value, false)
    })
}

/// Encodes one validated `ApprovalRecord` as exact compact canonical JSON.
///
/// # Errors
///
/// Returns an error when the record is invalid or oversized.
pub fn canonical_approval_record_json(
    value: &ApprovalRecord,
) -> Result<String, ApprovalRecordContractError> {
    super::record_validation::validate_record(value, false)?;
    bounded(value, MAX_RECORD_BYTES, "ApprovalRecord")
}

/// Decodes exact compact canonical declared-target JSON.
///
/// # Errors
///
/// Returns an error for malformed, non-canonical, oversized, or invalid input.
pub fn decode_canonical_declared_target(
    bytes: &[u8],
) -> Result<ApprovalDeclaredTarget, ApprovalRecordContractError> {
    decode(
        bytes,
        MAX_DECLARED_TARGET_BYTES,
        "declared target",
        super::record_validation::validate_target,
    )
}

/// Encodes one validated declared target as exact compact canonical JSON.
///
/// # Errors
///
/// Returns an error when the target is invalid or oversized.
pub fn canonical_declared_target_json(
    value: &ApprovalDeclaredTarget,
) -> Result<String, ApprovalRecordContractError> {
    super::record_validation::validate_target(value)?;
    bounded(value, MAX_DECLARED_TARGET_BYTES, "declared target")
}

/// Decodes exact compact canonical assessment-request JSON.
///
/// # Errors
///
/// Returns an error for malformed, non-canonical, oversized, or invalid input.
pub fn decode_canonical_assessment_request(
    bytes: &[u8],
) -> Result<ApprovalAssessmentRequest, ApprovalRecordContractError> {
    decode(
        bytes,
        MAX_ASSESSMENT_REQUEST_BYTES,
        "approval assessment request",
        |value| super::assessment_validation::validate_request(value, false),
    )
}

/// Encodes one validated assessment request as compact canonical JSON.
///
/// # Errors
///
/// Returns an error when the request is invalid or oversized.
pub fn canonical_assessment_request_json(
    value: &ApprovalAssessmentRequest,
) -> Result<String, ApprovalRecordContractError> {
    super::assessment_validation::validate_request(value, false)?;
    bounded(
        value,
        MAX_ASSESSMENT_REQUEST_BYTES,
        "approval assessment request",
    )
}

/// Decodes exact compact canonical authority-neutral assessment JSON.
///
/// # Errors
///
/// Returns an error for malformed, non-canonical, oversized, or invalid input.
pub fn decode_canonical_assessment(
    bytes: &[u8],
) -> Result<ApprovalDeclaredAssessment, ApprovalRecordContractError> {
    decode(
        bytes,
        MAX_ASSESSMENT_BYTES,
        "approval declared assessment",
        super::assessment_validation::validate_assessment_shape,
    )
}

/// Encodes one shape-valid authority-neutral assessment as canonical JSON.
///
/// # Errors
///
/// Returns an error when the assessment is invalid or oversized.
pub fn canonical_assessment_json(
    value: &ApprovalDeclaredAssessment,
) -> Result<String, ApprovalRecordContractError> {
    super::assessment_validation::validate_assessment_shape(value)?;
    bounded(value, MAX_ASSESSMENT_BYTES, "approval declared assessment")
}

/// Computes the frozen `ApprovalRecord` self digest.
///
/// # Errors
///
/// Returns an error when the record is invalid or cannot be encoded.
pub fn approval_sha256(value: &ApprovalRecord) -> Result<String, ApprovalRecordContractError> {
    super::record_validation::validate_record(value, true)?;
    approval_sha256_unchecked(value)
}

pub(super) fn approval_sha256_unchecked(
    value: &ApprovalRecord,
) -> Result<String, ApprovalRecordContractError> {
    let mut payload = value.clone();
    payload.approval_id.clear();
    payload.approval_sha256.clear();
    payload.authority_proof.proof_base64url.clear();
    payload.separation_of_duty.proof_base64url.clear();
    domain_digest(
        APPROVAL_DIGEST_DOMAIN,
        &payload,
        MAX_RECORD_BYTES,
        "ApprovalRecord",
    )
}

/// Computes the complete declared-target digest.
///
/// # Errors
///
/// Returns an error when the target is invalid or cannot be encoded.
pub fn declared_target_sha256(
    value: &ApprovalDeclaredTarget,
) -> Result<String, ApprovalRecordContractError> {
    super::record_validation::validate_target(value)?;
    domain_digest(
        DECLARED_TARGET_DIGEST_DOMAIN,
        value,
        MAX_DECLARED_TARGET_BYTES,
        "declared target",
    )
}

/// Computes the assessment-request self digest.
///
/// # Errors
///
/// Returns an error when the request is invalid or cannot be encoded.
pub fn assessment_request_sha256(
    value: &ApprovalAssessmentRequest,
) -> Result<String, ApprovalRecordContractError> {
    super::assessment_validation::validate_request(value, true)?;
    assessment_request_sha256_unchecked(value)
}

pub(super) fn assessment_request_sha256_unchecked(
    value: &ApprovalAssessmentRequest,
) -> Result<String, ApprovalRecordContractError> {
    let mut payload = value.clone();
    payload.request_sha256.clear();
    domain_digest(
        ASSESSMENT_REQUEST_DIGEST_DOMAIN,
        &payload,
        MAX_ASSESSMENT_REQUEST_BYTES,
        "approval assessment request",
    )
}

/// Computes the authority-neutral assessment self digest.
///
/// # Errors
///
/// Returns an error when the assessment cannot be encoded.
pub fn assessment_sha256(
    value: &ApprovalDeclaredAssessment,
) -> Result<String, ApprovalRecordContractError> {
    bounded(value, MAX_ASSESSMENT_BYTES, "approval declared assessment")?;
    assessment_sha256_unchecked(value)
}

pub(super) fn assessment_sha256_unchecked(
    value: &ApprovalDeclaredAssessment,
) -> Result<String, ApprovalRecordContractError> {
    let mut payload = value.clone();
    payload.assessment_sha256.clear();
    domain_digest(
        ASSESSMENT_DIGEST_DOMAIN,
        &payload,
        MAX_ASSESSMENT_BYTES,
        "approval declared assessment",
    )
}

pub(super) fn canonical_unbounded(
    value: &(impl Serialize + ?Sized),
) -> Result<String, ApprovalRecordContractError> {
    canonical_json(value).map_err(|error| invalid(error.message))
}

fn bounded(
    value: &(impl Serialize + ?Sized),
    maximum: usize,
    label: &str,
) -> Result<String, ApprovalRecordContractError> {
    let encoded = canonical_unbounded(value)?;
    if encoded.is_empty() || encoded.len() > maximum {
        Err(invalid(format!("{label} exceeds its canonical byte limit")))
    } else {
        Ok(encoded)
    }
}

fn domain_digest(
    domain: &[u8],
    value: &(impl Serialize + ?Sized),
    maximum: usize,
    label: &str,
) -> Result<String, ApprovalRecordContractError> {
    let encoded = bounded(value, maximum, label)?;
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(encoded.as_bytes());
    Ok(lower_hex(&digest.finalize()))
}

fn decode<T>(
    bytes: &[u8],
    maximum: usize,
    label: &str,
    validate: impl FnOnce(&T) -> Result<(), ApprovalRecordContractError>,
) -> Result<T, ApprovalRecordContractError>
where
    T: DeserializeOwned + Serialize,
{
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(invalid(format!("{label} violates its byte limit")));
    }
    let value: T = serde_json::from_slice(bytes)
        .map_err(|error| invalid(format!("{label} is invalid JSON: {error}")))?;
    validate(&value)?;
    let encoded = bounded(&value, maximum, label)?;
    if bytes == encoded.as_bytes() {
        Ok(value)
    } else {
        Err(invalid(format!(
            "input is not exact compact canonical JSON for {label}"
        )))
    }
}
