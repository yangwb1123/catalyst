use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{
    EvolveRepoLocator, EvolveRepoLocatorEvidenceContractError, EvolveRepoLocatorEvidenceRequest,
    EvolveRepoLocatorObservation, LOCATOR_DIGEST_DOMAIN, MAX_REQUEST_BYTES, REQUEST_DIGEST_DOMAIN,
    SOURCE_DIGEST_DOMAIN, invalid,
};

/// Decodes one exact compact canonical Evolve locator adapter request.
///
/// # Errors
///
/// Returns an error for malformed, duplicate, unknown, noncanonical, oversized,
/// or semantically invalid input.
pub fn decode_canonical_request(
    bytes: &[u8],
) -> Result<EvolveRepoLocatorEvidenceRequest, EvolveRepoLocatorEvidenceContractError> {
    if bytes.is_empty() || bytes.len() > MAX_REQUEST_BYTES {
        return Err(invalid("request byte length is outside the v1 limit"));
    }
    let request: EvolveRepoLocatorEvidenceRequest = serde_json::from_slice(bytes)
        .map_err(|error| invalid(format!("request is invalid JSON: {error}")))?;
    super::validation::validate_request(&request)?;
    let canonical = canonical_request_json(&request)?;
    if canonical.as_bytes() != bytes {
        return Err(invalid(
            "input is not exact compact canonical JSON for Evolve locator request",
        ));
    }
    Ok(request)
}

/// Encodes one exact Evolve repository locator after strict validation.
///
/// # Errors
///
/// Returns an error for invalid locator semantics or canonical JSON limits.
pub fn canonical_locator_json(
    locator: &EvolveRepoLocator,
) -> Result<String, EvolveRepoLocatorEvidenceContractError> {
    super::validation::validate_locator(locator)?;
    canonical_json(locator)
}

/// Encodes one exact Evolve repository locator observation.
///
/// # Errors
///
/// Returns an error for invalid observation semantics or canonical JSON limits.
pub fn canonical_observation_json(
    observation: &EvolveRepoLocatorObservation,
) -> Result<String, EvolveRepoLocatorEvidenceContractError> {
    super::validation::validate_observation(observation)?;
    canonical_json(observation)
}

/// Encodes one complete exact projectable adapter request.
///
/// # Errors
///
/// Returns an error for invalid request semantics or canonical JSON limits.
pub fn canonical_request_json(
    request: &EvolveRepoLocatorEvidenceRequest,
) -> Result<String, EvolveRepoLocatorEvidenceContractError> {
    super::validation::validate_request(request)?;
    canonical_json(request)
}

/// Computes the exact locator identity after complete request validation.
///
/// # Errors
///
/// Returns an error when request validation or canonical encoding fails.
pub fn locator_sha256(
    request: &EvolveRepoLocatorEvidenceRequest,
) -> Result<String, EvolveRepoLocatorEvidenceContractError> {
    super::validation::validate_request(request)?;
    Ok(digest_hex(
        LOCATOR_DIGEST_DOMAIN,
        canonical_locator_json(&request.observation.locator)?.as_bytes(),
    ))
}

/// Computes the exact observation source snapshot identity.
///
/// # Errors
///
/// Returns an error when request validation or canonical encoding fails.
pub fn source_snapshot_sha256(
    request: &EvolveRepoLocatorEvidenceRequest,
) -> Result<String, EvolveRepoLocatorEvidenceContractError> {
    super::validation::validate_request(request)?;
    Ok(digest_hex(
        SOURCE_DIGEST_DOMAIN,
        canonical_observation_json(&request.observation)?.as_bytes(),
    ))
}

/// Computes the complete request identity.
///
/// # Errors
///
/// Returns an error when request validation or canonical encoding fails.
pub fn request_sha256(
    request: &EvolveRepoLocatorEvidenceRequest,
) -> Result<String, EvolveRepoLocatorEvidenceContractError> {
    Ok(digest_hex(
        REQUEST_DIGEST_DOMAIN,
        canonical_request_json(request)?.as_bytes(),
    ))
}

fn canonical_json(
    value: &(impl Serialize + ?Sized),
) -> Result<String, EvolveRepoLocatorEvidenceContractError> {
    let canonical = crate::governance_contract::codec::canonical_json(value)
        .map_err(|error| invalid(format!("canonical JSON: {error}")))?;
    if canonical.len() > MAX_REQUEST_BYTES {
        return Err(invalid("adapter JSON exceeds the v1 byte limit"));
    }
    Ok(canonical)
}

fn digest_hex(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    crate::governance_contract::codec::lower_hex(&hasher.finalize())
}
