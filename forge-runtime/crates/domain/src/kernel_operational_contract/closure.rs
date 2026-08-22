use sha2::{Digest, Sha256};

use super::{
    CANONICALIZATION, KernelOperationalContractError, KernelOperationalReferenceClosure,
    MAX_ARTIFACT_RECEIPTS, MAX_ARTIFACTS, MAX_CLOSURE_BYTES, MAX_EVENTS, MAX_EXECUTION_RECEIPTS,
    MAX_INVOCATIONS, SUCCESS_MARKER,
    constants::{CLOSURE_API, CLOSURE_DOMAIN, CLOSURE_KIND, CLOSURE_PREFIX},
    graph, invalid, primitives, records, wire,
};

pub(super) fn validate_counts(
    value: &KernelOperationalReferenceClosure,
) -> Result<(), KernelOperationalContractError> {
    if value.artifacts.len() > MAX_ARTIFACTS
        || value.artifact_receipts.len() > MAX_ARTIFACT_RECEIPTS
        || !(1..=MAX_INVOCATIONS).contains(&value.capability_invocations.len())
        || value.interaction_events.len() > MAX_EVENTS
        || !(1..=MAX_EXECUTION_RECEIPTS).contains(&value.execution_receipts.len())
    {
        Err(invalid(
            "closure record cardinality is outside the frozen bounds",
        ))
    } else {
        Ok(())
    }
}

fn validate_receipt_order(
    value: &KernelOperationalReferenceClosure,
) -> Result<(), KernelOperationalContractError> {
    if value
        .artifact_receipts
        .windows(2)
        .all(|pair| pair[0].artifact_receipt_id.as_bytes() < pair[1].artifact_receipt_id.as_bytes())
    {
        Ok(())
    } else {
        Err(invalid(
            "artifact_receipts must be strictly identity-sorted and unique",
        ))
    }
}

fn validate_body(
    value: &KernelOperationalReferenceClosure,
    allow_blank: bool,
) -> Result<(), KernelOperationalContractError> {
    if value.api_version != CLOSURE_API
        || value.kind != CLOSURE_KIND
        || value.canonicalization != CANONICALIZATION
        || value.result != SUCCESS_MARKER
    {
        return Err(invalid("closure constants or result marker differ"));
    }
    primitives::validate_identity_fields(
        &value.closure_id,
        &value.closure_sha256,
        CLOSURE_PREFIX,
        "closure",
        allow_blank,
    )?;
    primitives::validate_attestations(&value.attestations)?;
    validate_counts(value)?;
    primitives::validate_sorted_unique(
        &value.artifacts,
        "artifacts",
        MAX_ARTIFACTS,
        false,
        |artifact| primitives::validate_artifact(artifact, "artifacts"),
    )?;
    records::validate_records(
        &value.artifact_receipts,
        &value.capability_invocations,
        &value.interaction_events,
        &value.execution_receipts,
    )?;
    validate_receipt_order(value)?;
    graph::validate_reference_graph(value)
}

fn closure_digest(
    value: &KernelOperationalReferenceClosure,
) -> Result<String, KernelOperationalContractError> {
    let mut blank = value.clone();
    blank.closure_id.clear();
    blank.closure_sha256.clear();
    validate_body(&blank, true)?;
    let canonical = wire::canonical_typed_with_max(&blank, MAX_CLOSURE_BYTES)?;
    let mut hasher = Sha256::new();
    hasher.update(CLOSURE_DOMAIN);
    hasher.update(canonical.as_bytes());
    Ok(crate::governance_contract::codec::lower_hex(
        &hasher.finalize(),
    ))
}

/// Validates every record, digest, projection, and supplied DAG edge.
///
/// # Errors
///
/// Returns an error for any wire-independent semantic or self-seal violation.
pub fn validate_closure(
    value: &KernelOperationalReferenceClosure,
) -> Result<(), KernelOperationalContractError> {
    validate_body(value, false)?;
    if value.closure_sha256 != closure_digest(value)? {
        return Err(invalid(
            "closure_sha256 does not match the canonical preimage",
        ));
    }
    wire::canonical_typed_with_max(value, MAX_CLOSURE_BYTES)?;
    Ok(())
}

/// Seals one exact blank-identity nonsemantic reference closure copy.
///
/// # Errors
///
/// Returns an error for a nonblank identity or invalid closure, record, seal, or graph.
pub fn seal_closure(
    value: &KernelOperationalReferenceClosure,
) -> Result<KernelOperationalReferenceClosure, KernelOperationalContractError> {
    if !value.closure_id.is_empty() || !value.closure_sha256.is_empty() {
        return Err(invalid("sealing closure requires blank identity fields"));
    }
    let digest = closure_digest(value)?;
    let mut sealed = value.clone();
    sealed.closure_id = format!("{CLOSURE_PREFIX}{digest}");
    sealed.closure_sha256 = digest;
    validate_closure(&sealed)?;
    Ok(sealed)
}

/// Decodes one exact compact canonical closure with no trailing LF.
///
/// # Errors
///
/// Returns an error for malformed, noncanonical, oversized, or semantically invalid input.
pub fn decode_closure(
    bytes: &[u8],
) -> Result<KernelOperationalReferenceClosure, KernelOperationalContractError> {
    let value = wire::decode_typed(bytes, MAX_CLOSURE_BYTES)?;
    validate_closure(&value)?;
    Ok(value)
}
