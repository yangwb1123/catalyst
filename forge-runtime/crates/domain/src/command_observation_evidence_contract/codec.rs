use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{
    COMMAND_DIGEST_DOMAIN, CommandObservation, CommandObservationEvidenceContractError,
    CommandObservationEvidenceRequest, MAX_REQUEST_BYTES, ObservedCommand, REQUEST_DIGEST_DOMAIN,
    SOURCE_DIGEST_DOMAIN, invalid,
};

/// Decodes one exact compact canonical projectable adapter request.
///
/// # Errors
///
/// Returns an error for malformed, duplicate, unknown, noncanonical, oversized,
/// or semantically invalid input.
pub fn decode_canonical_request(
    bytes: &[u8],
) -> Result<CommandObservationEvidenceRequest, CommandObservationEvidenceContractError> {
    if bytes.is_empty() || bytes.len() > MAX_REQUEST_BYTES {
        return Err(invalid("request byte length is outside the v1 limit"));
    }
    let request: CommandObservationEvidenceRequest = serde_json::from_slice(bytes)
        .map_err(|error| invalid(format!("request is invalid JSON: {error}")))?;
    super::validation::validate_request(&request)?;
    let canonical = canonical_request_json(&request)?;
    if canonical.as_bytes() != bytes {
        return Err(invalid(
            "input is not exact compact canonical JSON for command observation request",
        ));
    }
    Ok(request)
}

/// Encodes an exact command after strict source validation.
///
/// # Errors
///
/// Returns an error for invalid source semantics or canonical JSON limits.
pub fn canonical_command_json(
    command: &ObservedCommand,
) -> Result<String, CommandObservationEvidenceContractError> {
    super::validation::validate_command(command)?;
    canonical_json(command)
}

/// Encodes an observation, including valid non-projectable termination states.
///
/// # Errors
///
/// Returns an error for invalid observation semantics or canonical JSON limits.
pub fn canonical_observation_json(
    observation: &CommandObservation,
) -> Result<String, CommandObservationEvidenceContractError> {
    super::validation::validate_observation(observation)?;
    canonical_json(observation)
}

/// Encodes a complete projectable request.
///
/// # Errors
///
/// Returns an error for invalid request semantics or canonical JSON limits.
pub fn canonical_request_json(
    request: &CommandObservationEvidenceRequest,
) -> Result<String, CommandObservationEvidenceContractError> {
    super::validation::validate_request(request)?;
    canonical_json(request)
}

/// Computes the exact command identity after complete request validation.
///
/// # Errors
///
/// Returns an error when request validation or canonical encoding fails.
pub fn command_sha256(
    request: &CommandObservationEvidenceRequest,
) -> Result<String, CommandObservationEvidenceContractError> {
    super::validation::validate_request(request)?;
    Ok(digest_hex(
        COMMAND_DIGEST_DOMAIN,
        canonical_command_json(&request.observation.command)?.as_bytes(),
    ))
}

/// Computes the exact observation source snapshot identity.
///
/// # Errors
///
/// Returns an error when request validation or canonical encoding fails.
pub fn source_snapshot_sha256(
    request: &CommandObservationEvidenceRequest,
) -> Result<String, CommandObservationEvidenceContractError> {
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
    request: &CommandObservationEvidenceRequest,
) -> Result<String, CommandObservationEvidenceContractError> {
    Ok(digest_hex(
        REQUEST_DIGEST_DOMAIN,
        canonical_request_json(request)?.as_bytes(),
    ))
}

fn canonical_json(
    value: &(impl Serialize + ?Sized),
) -> Result<String, CommandObservationEvidenceContractError> {
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
