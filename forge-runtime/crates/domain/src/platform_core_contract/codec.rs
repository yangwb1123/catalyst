use sha2::{Digest, Sha256};

use super::{
    ARTIFACT_DIGEST_DOMAIN, ArtifactRef, COMMAND_DIGEST_DOMAIN, CommandEnvelope,
    EVENT_DIGEST_DOMAIN, EXECUTION_RECEIPT_DIGEST_DOMAIN, EventEnvelope, ExecutionReceipt,
    MAX_ARTIFACT_REF_BYTES, MAX_ENVELOPE_BYTES, MAX_RECEIPT_BYTES, PlatformCoreContractError,
    RejectionCode, VERIFICATION_RECEIPT_DIGEST_DOMAIN, VERIFICATION_REQUEST_DIGEST_DOMAIN,
    VerificationReceipt, VerificationRequest, artifact, command, event, execution_receipt, recode,
    verification, wire,
};

/// Decodes one exact compact canonical `ArtifactRef` v1.
///
/// # Errors
/// Returns an error for malformed, noncanonical, oversized, or invalid input.
pub fn decode_canonical_artifact_ref(
    bytes: &[u8],
) -> Result<ArtifactRef, PlatformCoreContractError> {
    let value = wire::decode_typed(bytes, MAX_ARTIFACT_REF_BYTES)
        .map_err(|error| recode(error, RejectionCode::DocumentInvalid))?;
    artifact::validate_artifact_ref(&value)?;
    Ok(value)
}

/// Encodes one validated `ArtifactRef` as exact compact canonical JSON.
///
/// # Errors
/// Returns an error when any declaration or bound is invalid.
pub fn canonical_artifact_ref_json(
    value: &ArtifactRef,
) -> Result<String, PlatformCoreContractError> {
    artifact::validate_artifact_ref(value)?;
    wire::canonical_typed(value, MAX_ARTIFACT_REF_BYTES)
}

/// Computes the domain-separated `ArtifactRef` conformance digest.
///
/// # Errors
/// Returns an error when the `ArtifactRef` cannot be canonically encoded.
pub fn artifact_ref_sha256(value: &ArtifactRef) -> Result<String, PlatformCoreContractError> {
    let canonical = canonical_artifact_ref_json(value)?;
    Ok(observation_digest(
        ARTIFACT_DIGEST_DOMAIN,
        canonical.as_bytes(),
    ))
}

/// Decodes one exact compact canonical `CommandEnvelope` v1.
///
/// # Errors
/// Returns an error for malformed, noncanonical, oversized, or invalid input.
pub fn decode_canonical_command_envelope(
    bytes: &[u8],
) -> Result<CommandEnvelope, PlatformCoreContractError> {
    let value = wire::decode_typed(bytes, MAX_ENVELOPE_BYTES)
        .map_err(|error| recode(error, RejectionCode::DocumentInvalid))?;
    command::validate_command_envelope(&value)?;
    Ok(value)
}

/// Encodes one validated `CommandEnvelope` as exact compact canonical JSON.
///
/// # Errors
/// Returns an error when any declaration, relation, or bound is invalid.
pub fn canonical_command_envelope_json(
    value: &CommandEnvelope,
) -> Result<String, PlatformCoreContractError> {
    command::validate_command_envelope(value)?;
    wire::canonical_typed(value, MAX_ENVELOPE_BYTES)
}

/// Computes the domain-separated `CommandEnvelope` conformance digest.
///
/// # Errors
/// Returns an error when the command cannot be canonically encoded.
pub fn command_envelope_sha256(
    value: &CommandEnvelope,
) -> Result<String, PlatformCoreContractError> {
    let canonical = canonical_command_envelope_json(value)?;
    Ok(observation_digest(
        COMMAND_DIGEST_DOMAIN,
        canonical.as_bytes(),
    ))
}

/// Decodes one exact compact canonical `EventEnvelope` v1.
///
/// # Errors
/// Returns an error for malformed, noncanonical, oversized, or invalid input.
pub fn decode_canonical_event_envelope(
    bytes: &[u8],
) -> Result<EventEnvelope, PlatformCoreContractError> {
    let value = wire::decode_typed(bytes, MAX_ENVELOPE_BYTES)
        .map_err(|error| recode(error, RejectionCode::DocumentInvalid))?;
    event::validate_event_envelope(&value)?;
    Ok(value)
}

/// Encodes one validated `EventEnvelope` as exact compact canonical JSON.
///
/// # Errors
/// Returns an error when any declaration, relation, or bound is invalid.
pub fn canonical_event_envelope_json(
    value: &EventEnvelope,
) -> Result<String, PlatformCoreContractError> {
    event::validate_event_envelope(value)?;
    wire::canonical_typed(value, MAX_ENVELOPE_BYTES)
}

/// Computes the domain-separated `EventEnvelope` conformance digest.
///
/// # Errors
/// Returns an error when the event cannot be canonically encoded.
pub fn event_envelope_sha256(value: &EventEnvelope) -> Result<String, PlatformCoreContractError> {
    let canonical = canonical_event_envelope_json(value)?;
    Ok(observation_digest(
        EVENT_DIGEST_DOMAIN,
        canonical.as_bytes(),
    ))
}

/// Decodes one exact compact canonical `ExecutionReceipt` v1.
///
/// # Errors
/// Returns a stable coded error for malformed, noncanonical, oversized, or invalid input.
pub fn decode_canonical_execution_receipt(
    bytes: &[u8],
) -> Result<ExecutionReceipt, PlatformCoreContractError> {
    let value = wire::decode_typed(bytes, MAX_RECEIPT_BYTES)
        .map_err(|error| recode(error, RejectionCode::DocumentInvalid))?;
    execution_receipt::validate_execution_receipt(&value)?;
    Ok(value)
}

/// Encodes one validated `ExecutionReceipt` as exact compact canonical JSON.
///
/// # Errors
/// Returns an error when any declaration, relation, state, or bound is invalid.
pub fn canonical_execution_receipt_json(
    value: &ExecutionReceipt,
) -> Result<String, PlatformCoreContractError> {
    execution_receipt::validate_execution_receipt(value)?;
    wire::canonical_typed(value, MAX_RECEIPT_BYTES)
}

/// Computes the domain-separated `ExecutionReceipt` conformance digest.
///
/// # Errors
/// Returns an error when the receipt cannot be canonically encoded.
pub fn execution_receipt_sha256(
    value: &ExecutionReceipt,
) -> Result<String, PlatformCoreContractError> {
    let canonical = canonical_execution_receipt_json(value)?;
    Ok(observation_digest(
        EXECUTION_RECEIPT_DIGEST_DOMAIN,
        canonical.as_bytes(),
    ))
}

/// Decodes one exact compact canonical `VerificationRequest` v1.
///
/// # Errors
/// Returns a stable coded error for malformed, noncanonical, oversized, or invalid input.
pub fn decode_canonical_verification_request(
    bytes: &[u8],
) -> Result<VerificationRequest, PlatformCoreContractError> {
    let value = wire::decode_typed(bytes, MAX_RECEIPT_BYTES)
        .map_err(|error| recode(error, RejectionCode::DocumentInvalid))?;
    verification::validate_verification_request(&value)?;
    Ok(value)
}

/// Encodes one validated `VerificationRequest` as exact compact canonical JSON.
///
/// # Errors
/// Returns an error when any declaration, reference, or bound is invalid.
pub fn canonical_verification_request_json(
    value: &VerificationRequest,
) -> Result<String, PlatformCoreContractError> {
    verification::validate_verification_request(value)?;
    wire::canonical_typed(value, MAX_RECEIPT_BYTES)
}

/// Computes the domain-separated `VerificationRequest` conformance digest.
///
/// # Errors
/// Returns an error when the request cannot be canonically encoded.
pub fn verification_request_sha256(
    value: &VerificationRequest,
) -> Result<String, PlatformCoreContractError> {
    let canonical = canonical_verification_request_json(value)?;
    Ok(observation_digest(
        VERIFICATION_REQUEST_DIGEST_DOMAIN,
        canonical.as_bytes(),
    ))
}

/// Decodes one exact compact canonical `VerificationReceipt` v1.
///
/// # Errors
/// Returns a stable coded error for malformed, noncanonical, oversized, or invalid input.
pub fn decode_canonical_verification_receipt(
    bytes: &[u8],
) -> Result<VerificationReceipt, PlatformCoreContractError> {
    let value = wire::decode_typed(bytes, MAX_RECEIPT_BYTES)
        .map_err(|error| recode(error, RejectionCode::DocumentInvalid))?;
    verification::validate_verification_receipt(&value)?;
    Ok(value)
}

/// Encodes one validated `VerificationReceipt` as exact compact canonical JSON.
///
/// # Errors
/// Returns an error when any declaration, relation, state, or bound is invalid.
pub fn canonical_verification_receipt_json(
    value: &VerificationReceipt,
) -> Result<String, PlatformCoreContractError> {
    verification::validate_verification_receipt(value)?;
    wire::canonical_typed(value, MAX_RECEIPT_BYTES)
}

/// Computes the domain-separated `VerificationReceipt` conformance digest.
///
/// # Errors
/// Returns an error when the receipt cannot be canonically encoded.
pub fn verification_receipt_sha256(
    value: &VerificationReceipt,
) -> Result<String, PlatformCoreContractError> {
    let canonical = canonical_verification_receipt_json(value)?;
    Ok(observation_digest(
        VERIFICATION_RECEIPT_DIGEST_DOMAIN,
        canonical.as_bytes(),
    ))
}

fn observation_digest(domain: &[u8], canonical: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(canonical);
    lower_hex(&hasher.finalize())
}

fn lower_hex(value: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(value.len() * 2);
    for byte in value {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}
