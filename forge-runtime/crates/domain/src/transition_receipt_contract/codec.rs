use std::io::{self, Write};

use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

use crate::governance_contract::codec::{canonical_json, lower_hex};

use super::{
    ASSESSMENT_DIGEST_DOMAIN, ASSESSMENT_REQUEST_DIGEST_DOMAIN, DECLARED_TARGET_DIGEST_DOMAIN,
    MAX_ASSESSMENT_BYTES, MAX_ASSESSMENT_REQUEST_BYTES, MAX_DECLARED_TARGET_BYTES,
    MAX_RECEIPT_BYTES, MAX_VOCABULARY_BYTES, RECEIPT_DIGEST_DOMAIN, TransitionAssessmentRequest,
    TransitionDeclaredAssessment, TransitionDeclaredTarget, TransitionReceipt,
    TransitionReceiptContractError, TransitionStateVocabulary, VOCABULARY_DIGEST_DOMAIN, invalid,
};

/// Decodes the exact canonical Transition vocabulary.
///
/// # Errors
/// Returns an error for malformed, noncanonical, drifted, or oversized bytes.
pub fn decode_canonical_vocabulary(
    bytes: &[u8],
) -> Result<TransitionStateVocabulary, TransitionReceiptContractError> {
    decode(
        bytes,
        MAX_VOCABULARY_BYTES,
        "Transition vocabulary",
        |value| super::validation::validate_vocabulary(value, false),
    )
}

/// Encodes the exact frozen Transition vocabulary.
///
/// # Errors
/// Returns an error for drift or resource-limit violations.
pub fn canonical_vocabulary_json(
    value: &TransitionStateVocabulary,
) -> Result<String, TransitionReceiptContractError> {
    super::validation::validate_vocabulary(value, false)?;
    bounded(value, MAX_VOCABULARY_BYTES, "Transition vocabulary")
}

/// Decodes one exact canonical `TransitionReceipt`.
///
/// # Errors
/// Returns an error for malformed, noncanonical, or oversized bytes.
pub fn decode_canonical_receipt(
    bytes: &[u8],
) -> Result<TransitionReceipt, TransitionReceiptContractError> {
    decode(bytes, MAX_RECEIPT_BYTES, "TransitionReceipt", |value| {
        super::validation::validate_receipt(value, false)
    })
}

/// Encodes one validated `TransitionReceipt`.
///
/// # Errors
/// Returns an error for an invalid receipt or resource-limit violation.
pub fn canonical_receipt_json(
    value: &TransitionReceipt,
) -> Result<String, TransitionReceiptContractError> {
    super::validation::validate_receipt(value, false)?;
    bounded(value, MAX_RECEIPT_BYTES, "TransitionReceipt")
}

/// Decodes one exact canonical declared target.
///
/// # Errors
/// Returns an error for malformed, noncanonical, or oversized bytes.
pub fn decode_canonical_declared_target(
    bytes: &[u8],
) -> Result<TransitionDeclaredTarget, TransitionReceiptContractError> {
    decode(
        bytes,
        MAX_DECLARED_TARGET_BYTES,
        "Transition declared target",
        super::validation::validate_target,
    )
}

/// Encodes one validated declared target.
///
/// # Errors
/// Returns an error for an invalid target or resource-limit violation.
pub fn canonical_declared_target_json(
    value: &TransitionDeclaredTarget,
) -> Result<String, TransitionReceiptContractError> {
    super::validation::validate_target(value)?;
    bounded(
        value,
        MAX_DECLARED_TARGET_BYTES,
        "Transition declared target",
    )
}

/// Decodes one exact canonical assessment request.
///
/// # Errors
/// Returns an error for malformed, noncanonical, or oversized bytes.
pub fn decode_canonical_assessment_request(
    bytes: &[u8],
) -> Result<TransitionAssessmentRequest, TransitionReceiptContractError> {
    decode(
        bytes,
        MAX_ASSESSMENT_REQUEST_BYTES,
        "Transition assessment request",
        |value| super::validation::validate_request(value, false),
    )
}

/// Encodes one validated assessment request.
///
/// # Errors
/// Returns an error for an invalid request or resource-limit violation.
pub fn canonical_assessment_request_json(
    value: &TransitionAssessmentRequest,
) -> Result<String, TransitionReceiptContractError> {
    super::validation::validate_request(value, false)?;
    bounded(
        value,
        MAX_ASSESSMENT_REQUEST_BYTES,
        "Transition assessment request",
    )
}

/// Decodes one exact canonical authority-neutral assessment.
///
/// # Errors
/// Returns an error for malformed, noncanonical, or oversized bytes.
pub fn decode_canonical_assessment(
    bytes: &[u8],
) -> Result<TransitionDeclaredAssessment, TransitionReceiptContractError> {
    decode(
        bytes,
        MAX_ASSESSMENT_BYTES,
        "Transition declared assessment",
        super::assessment_validation::validate_assessment_shape,
    )
}

/// Encodes one validated authority-neutral assessment.
///
/// # Errors
/// Returns an error for an invalid assessment or resource-limit violation.
pub fn canonical_assessment_json(
    value: &TransitionDeclaredAssessment,
) -> Result<String, TransitionReceiptContractError> {
    super::assessment_validation::validate_assessment_shape(value)?;
    bounded(
        value,
        MAX_ASSESSMENT_BYTES,
        "Transition declared assessment",
    )
}

/// Computes the frozen vocabulary content digest.
///
/// # Errors
/// Returns an error when the vocabulary is invalid or cannot be bounded.
pub fn vocabulary_sha256(
    value: &TransitionStateVocabulary,
) -> Result<String, TransitionReceiptContractError> {
    super::validation::validate_vocabulary(value, true)?;
    vocabulary_sha256_unchecked(value)
}

pub(super) fn vocabulary_sha256_unchecked(
    value: &TransitionStateVocabulary,
) -> Result<String, TransitionReceiptContractError> {
    let mut payload = value.clone();
    payload.vocabulary_sha256.clear();
    domain_digest(
        VOCABULARY_DIGEST_DOMAIN,
        &payload,
        MAX_VOCABULARY_BYTES,
        "Transition vocabulary",
    )
}

/// Computes the `TransitionReceipt` content digest.
///
/// # Errors
/// Returns an error when the receipt is invalid or cannot be bounded.
pub fn receipt_sha256(value: &TransitionReceipt) -> Result<String, TransitionReceiptContractError> {
    super::validation::validate_receipt(value, true)?;
    receipt_sha256_unchecked(value)
}

pub(super) fn receipt_sha256_unchecked(
    value: &TransitionReceipt,
) -> Result<String, TransitionReceiptContractError> {
    let mut payload = value.clone();
    payload.receipt_id.clear();
    payload.receipt_sha256.clear();
    domain_digest(
        RECEIPT_DIGEST_DOMAIN,
        &payload,
        MAX_RECEIPT_BYTES,
        "TransitionReceipt",
    )
}

/// Computes the complete declared-target digest.
///
/// # Errors
/// Returns an error when the target is invalid or cannot be bounded.
pub fn declared_target_sha256(
    value: &TransitionDeclaredTarget,
) -> Result<String, TransitionReceiptContractError> {
    super::validation::validate_target(value)?;
    domain_digest(
        DECLARED_TARGET_DIGEST_DOMAIN,
        value,
        MAX_DECLARED_TARGET_BYTES,
        "Transition declared target",
    )
}

/// Computes the assessment-request self digest.
///
/// # Errors
/// Returns an error when the request is invalid or cannot be bounded.
pub fn assessment_request_sha256(
    value: &TransitionAssessmentRequest,
) -> Result<String, TransitionReceiptContractError> {
    super::validation::validate_request(value, true)?;
    assessment_request_sha256_unchecked(value)
}

pub(super) fn assessment_request_sha256_unchecked(
    value: &TransitionAssessmentRequest,
) -> Result<String, TransitionReceiptContractError> {
    let mut payload = value.clone();
    payload.request_sha256.clear();
    domain_digest(
        ASSESSMENT_REQUEST_DIGEST_DOMAIN,
        &payload,
        MAX_ASSESSMENT_REQUEST_BYTES,
        "Transition assessment request",
    )
}

/// Computes the authority-neutral assessment self digest.
///
/// # Errors
/// Returns an error when the assessment cannot be bounded or encoded.
pub fn assessment_sha256(
    value: &TransitionDeclaredAssessment,
) -> Result<String, TransitionReceiptContractError> {
    bounded(
        value,
        MAX_ASSESSMENT_BYTES,
        "Transition declared assessment",
    )?;
    assessment_sha256_unchecked(value)
}

pub(super) fn assessment_sha256_unchecked(
    value: &TransitionDeclaredAssessment,
) -> Result<String, TransitionReceiptContractError> {
    let mut payload = value.clone();
    payload.assessment_sha256.clear();
    domain_digest(
        ASSESSMENT_DIGEST_DOMAIN,
        &payload,
        MAX_ASSESSMENT_BYTES,
        "Transition declared assessment",
    )
}

pub(super) fn canonical_unbounded(
    value: &(impl Serialize + ?Sized),
) -> Result<String, TransitionReceiptContractError> {
    canonical_json(value).map_err(|error| invalid(error.message))
}

pub(super) fn bounded(
    value: &(impl Serialize + ?Sized),
    maximum: usize,
    label: &str,
) -> Result<String, TransitionReceiptContractError> {
    measure_serialized(value, maximum, label)?;
    let encoded = canonical_unbounded(value)?;
    if encoded.is_empty() || encoded.len() > maximum {
        Err(invalid(format!("{label} exceeds its canonical byte limit")))
    } else {
        Ok(encoded)
    }
}

fn measure_serialized(
    value: &(impl Serialize + ?Sized),
    maximum: usize,
    label: &str,
) -> Result<(), TransitionReceiptContractError> {
    let mut counter = BoundedCounter {
        maximum,
        written: 0,
    };
    serde_json::to_writer(&mut counter, value)
        .map_err(|error| invalid(format!("{label} cannot be bounded: {error}")))?;
    if counter.written == 0 {
        Err(invalid(format!("{label} has an empty canonical encoding")))
    } else {
        Ok(())
    }
}

struct BoundedCounter {
    maximum: usize,
    written: usize,
}

impl Write for BoundedCounter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let next = self
            .written
            .checked_add(bytes.len())
            .ok_or_else(limit_error)?;
        if next > self.maximum {
            return Err(limit_error());
        }
        self.written = next;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn limit_error() -> io::Error {
    io::Error::other("canonical JSON exceeds its configured byte ceiling")
}

fn domain_digest(
    domain: &[u8],
    value: &(impl Serialize + ?Sized),
    maximum: usize,
    label: &str,
) -> Result<String, TransitionReceiptContractError> {
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
    validate: impl FnOnce(&T) -> Result<(), TransitionReceiptContractError>,
) -> Result<T, TransitionReceiptContractError>
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
