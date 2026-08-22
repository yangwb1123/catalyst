use sha2::{Digest, Sha256};

use super::{
    ArtifactReceipt, CapabilityInvocation, ExecutionReceipt, InteractionEvent,
    KernelOperationalContractError, MAX_ARTIFACT_RECEIPT_BYTES, MAX_EVENT_BYTES,
    MAX_EXECUTION_RECEIPT_BYTES, MAX_INVOCATION_BYTES,
    constants::{
        ARTIFACT_RECEIPT_DOMAIN, ARTIFACT_RECEIPT_PREFIX, EVENT_DOMAIN, EVENT_PREFIX,
        EXECUTION_RECEIPT_DOMAIN, EXECUTION_RECEIPT_PREFIX, INVOCATION_DOMAIN, INVOCATION_PREFIX,
    },
    invalid,
    validation::{
        validate_artifact_receipt_body, validate_event_body, validate_execution_receipt_body,
        validate_invocation_body,
    },
    wire,
};

fn digest(domain: &[u8], canonical: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(canonical.as_bytes());
    crate::governance_contract::codec::lower_hex(&hasher.finalize())
}

fn artifact_receipt_digest(
    value: &ArtifactReceipt,
) -> Result<String, KernelOperationalContractError> {
    let mut blank = value.clone();
    blank.artifact_receipt_id.clear();
    blank.artifact_receipt_sha256.clear();
    validate_artifact_receipt_body(&blank, true)?;
    let canonical = wire::canonical_typed_with_max(&blank, MAX_ARTIFACT_RECEIPT_BYTES)?;
    Ok(digest(ARTIFACT_RECEIPT_DOMAIN, &canonical))
}

fn validate_artifact_receipt(
    value: &ArtifactReceipt,
) -> Result<(), KernelOperationalContractError> {
    validate_artifact_receipt_body(value, false)?;
    if value.artifact_receipt_sha256 != artifact_receipt_digest(value)? {
        return Err(invalid(
            "artifact_receipt_sha256 does not match the canonical preimage",
        ));
    }
    wire::canonical_typed_with_max(value, MAX_ARTIFACT_RECEIPT_BYTES)?;
    Ok(())
}

/// Seals one exact blank-identity `ArtifactReceipt` copy.
///
/// # Errors
///
/// Returns an error for a nonblank identity or invalid field, bound, order, or seal.
pub fn seal_artifact_receipt(
    value: &ArtifactReceipt,
) -> Result<ArtifactReceipt, KernelOperationalContractError> {
    if !value.artifact_receipt_id.is_empty() || !value.artifact_receipt_sha256.is_empty() {
        return Err(invalid(
            "sealing ArtifactReceipt requires blank identity fields",
        ));
    }
    let digest = artifact_receipt_digest(value)?;
    let mut sealed = value.clone();
    sealed.artifact_receipt_id = format!("{ARTIFACT_RECEIPT_PREFIX}{digest}");
    sealed.artifact_receipt_sha256 = digest;
    validate_artifact_receipt(&sealed)?;
    Ok(sealed)
}

/// Decodes one exact canonical sealed `ArtifactReceipt`.
///
/// # Errors
///
/// Returns an error for malformed, noncanonical, oversized, or invalid input.
pub fn decode_artifact_receipt(
    bytes: &[u8],
) -> Result<ArtifactReceipt, KernelOperationalContractError> {
    let value = wire::decode_typed(bytes, MAX_ARTIFACT_RECEIPT_BYTES)?;
    validate_artifact_receipt(&value)?;
    Ok(value)
}

fn invocation_digest(
    value: &CapabilityInvocation,
) -> Result<String, KernelOperationalContractError> {
    let mut blank = value.clone();
    blank.invocation_id.clear();
    blank.invocation_sha256.clear();
    validate_invocation_body(&blank, true)?;
    let canonical = wire::canonical_typed_with_max(&blank, MAX_INVOCATION_BYTES)?;
    Ok(digest(INVOCATION_DOMAIN, &canonical))
}

fn validate_invocation(value: &CapabilityInvocation) -> Result<(), KernelOperationalContractError> {
    validate_invocation_body(value, false)?;
    if value.invocation_sha256 != invocation_digest(value)? {
        return Err(invalid(
            "invocation_sha256 does not match the canonical preimage",
        ));
    }
    wire::canonical_typed_with_max(value, MAX_INVOCATION_BYTES)?;
    Ok(())
}

/// Seals one exact blank-identity `CapabilityInvocation` copy.
///
/// # Errors
///
/// Returns an error for a nonblank identity or invalid field, bound, order, or seal.
pub fn seal_capability_invocation(
    value: &CapabilityInvocation,
) -> Result<CapabilityInvocation, KernelOperationalContractError> {
    if !value.invocation_id.is_empty() || !value.invocation_sha256.is_empty() {
        return Err(invalid("sealing invocation requires blank identity fields"));
    }
    let digest = invocation_digest(value)?;
    let mut sealed = value.clone();
    sealed.invocation_id = format!("{INVOCATION_PREFIX}{digest}");
    sealed.invocation_sha256 = digest;
    validate_invocation(&sealed)?;
    Ok(sealed)
}

/// Decodes one exact canonical sealed `CapabilityInvocation`.
///
/// # Errors
///
/// Returns an error for malformed, noncanonical, oversized, or invalid input.
pub fn decode_capability_invocation(
    bytes: &[u8],
) -> Result<CapabilityInvocation, KernelOperationalContractError> {
    let value = wire::decode_typed(bytes, MAX_INVOCATION_BYTES)?;
    validate_invocation(&value)?;
    Ok(value)
}

fn event_digest(value: &InteractionEvent) -> Result<String, KernelOperationalContractError> {
    let mut blank = value.clone();
    blank.event_id.clear();
    blank.event_sha256.clear();
    validate_event_body(&blank, true)?;
    let canonical = wire::canonical_typed_with_max(&blank, MAX_EVENT_BYTES)?;
    Ok(digest(EVENT_DOMAIN, &canonical))
}

fn validate_event(value: &InteractionEvent) -> Result<(), KernelOperationalContractError> {
    validate_event_body(value, false)?;
    if value.event_sha256 != event_digest(value)? {
        return Err(invalid(
            "event_sha256 does not match the canonical preimage",
        ));
    }
    wire::canonical_typed_with_max(value, MAX_EVENT_BYTES)?;
    Ok(())
}

/// Seals one exact blank-identity `InteractionEvent` copy.
///
/// # Errors
///
/// Returns an error for a nonblank identity or invalid field, bound, order, or seal.
pub fn seal_interaction_event(
    value: &InteractionEvent,
) -> Result<InteractionEvent, KernelOperationalContractError> {
    if !value.event_id.is_empty() || !value.event_sha256.is_empty() {
        return Err(invalid("sealing event requires blank identity fields"));
    }
    let digest = event_digest(value)?;
    let mut sealed = value.clone();
    sealed.event_id = format!("{EVENT_PREFIX}{digest}");
    sealed.event_sha256 = digest;
    validate_event(&sealed)?;
    Ok(sealed)
}

/// Decodes one exact canonical sealed `InteractionEvent`.
///
/// # Errors
///
/// Returns an error for malformed, noncanonical, oversized, or invalid input.
pub fn decode_interaction_event(
    bytes: &[u8],
) -> Result<InteractionEvent, KernelOperationalContractError> {
    let value = wire::decode_typed(bytes, MAX_EVENT_BYTES)?;
    validate_event(&value)?;
    Ok(value)
}

fn execution_receipt_digest(
    value: &ExecutionReceipt,
) -> Result<String, KernelOperationalContractError> {
    let mut blank = value.clone();
    blank.execution_receipt_id.clear();
    blank.execution_receipt_sha256.clear();
    validate_execution_receipt_body(&blank, true)?;
    let canonical = wire::canonical_typed_with_max(&blank, MAX_EXECUTION_RECEIPT_BYTES)?;
    Ok(digest(EXECUTION_RECEIPT_DOMAIN, &canonical))
}

fn validate_execution_receipt(
    value: &ExecutionReceipt,
) -> Result<(), KernelOperationalContractError> {
    validate_execution_receipt_body(value, false)?;
    if value.execution_receipt_sha256 != execution_receipt_digest(value)? {
        return Err(invalid(
            "execution_receipt_sha256 does not match the canonical preimage",
        ));
    }
    wire::canonical_typed_with_max(value, MAX_EXECUTION_RECEIPT_BYTES)?;
    Ok(())
}

/// Seals one exact blank-identity `ExecutionReceipt` copy.
///
/// # Errors
///
/// Returns an error for a nonblank identity or invalid field, bound, order, or seal.
pub fn seal_execution_receipt(
    value: &ExecutionReceipt,
) -> Result<ExecutionReceipt, KernelOperationalContractError> {
    if !value.execution_receipt_id.is_empty() || !value.execution_receipt_sha256.is_empty() {
        return Err(invalid(
            "sealing ExecutionReceipt requires blank identity fields",
        ));
    }
    let digest = execution_receipt_digest(value)?;
    let mut sealed = value.clone();
    sealed.execution_receipt_id = format!("{EXECUTION_RECEIPT_PREFIX}{digest}");
    sealed.execution_receipt_sha256 = digest;
    validate_execution_receipt(&sealed)?;
    Ok(sealed)
}

/// Decodes one exact canonical sealed `ExecutionReceipt`.
///
/// # Errors
///
/// Returns an error for malformed, noncanonical, oversized, or invalid input.
pub fn decode_execution_receipt(
    bytes: &[u8],
) -> Result<ExecutionReceipt, KernelOperationalContractError> {
    let value = wire::decode_typed(bytes, MAX_EXECUTION_RECEIPT_BYTES)?;
    validate_execution_receipt(&value)?;
    Ok(value)
}

pub(super) fn validate_records(
    artifact_receipts: &[ArtifactReceipt],
    invocations: &[CapabilityInvocation],
    events: &[InteractionEvent],
    receipts: &[ExecutionReceipt],
) -> Result<(), KernelOperationalContractError> {
    for value in artifact_receipts {
        validate_artifact_receipt(value)?;
    }
    for value in invocations {
        validate_invocation(value)?;
    }
    for value in events {
        validate_event(value)?;
    }
    for value in receipts {
        validate_execution_receipt(value)?;
    }
    Ok(())
}
