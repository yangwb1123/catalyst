use sha2::{Digest, Sha256};

use super::{
    CANONICALIZATION, CLOSURE_API, CLOSURE_DOMAIN, CLOSURE_KIND, CLOSURE_PREFIX,
    KernelDecisionContractError, KernelDecisionReferenceClosure, MAX_ATOM_SET_BYTES,
    MAX_CLOSURE_BYTES, SUCCESS_MARKER,
    atom::validate_atoms,
    graph::validate_reference_graph,
    invalid,
    primitives::{attestations, identity},
    transaction::validate_decision_transaction,
    wire,
};

fn validate_body(
    value: &KernelDecisionReferenceClosure,
    allow_blank: bool,
) -> Result<(), KernelDecisionContractError> {
    if value.api_version != CLOSURE_API
        || value.canonicalization != CANONICALIZATION
        || value.kind != CLOSURE_KIND
        || value.result != SUCCESS_MARKER
    {
        return Err(invalid(
            "KernelDecisionReferenceClosure constants or marker differ",
        ));
    }
    identity(
        &value.closure_id,
        &value.closure_sha256,
        CLOSURE_PREFIX,
        "closure",
        allow_blank,
    )?;
    attestations(&value.attestations)?;
    validate_atoms(&value.cognitive_atoms)?;
    wire::canonical_with_max(&value.cognitive_atoms, MAX_ATOM_SET_BYTES)?;
    validate_decision_transaction(&value.decision_transaction)?;
    crate::kernel_operational_contract::validate_closure(&value.operational_closure)
        .map_err(|error| invalid(format!("operational_closure: {}", error.message)))?;
    validate_reference_graph(
        &value.cognitive_atoms,
        &value.decision_transaction,
        &value.operational_closure,
    )
}

fn digest(value: &KernelDecisionReferenceClosure) -> Result<String, KernelDecisionContractError> {
    let mut blank = value.clone();
    blank.closure_id.clear();
    blank.closure_sha256.clear();
    validate_body(&blank, true)?;
    let canonical = wire::canonical_with_max(&blank, MAX_CLOSURE_BYTES)?;
    let mut hasher = Sha256::new();
    hasher.update(CLOSURE_DOMAIN);
    hasher.update(canonical.as_bytes());
    Ok(crate::governance_contract::codec::lower_hex(
        &hasher.finalize(),
    ))
}

/// Validates one exact sealed Kernel decision reference closure and complete one-way DAG.
///
/// # Errors
///
/// Returns an error for any atom, transaction, reused operational closure, graph, or digest violation.
pub fn validate_closure(
    value: &KernelDecisionReferenceClosure,
) -> Result<(), KernelDecisionContractError> {
    validate_body(value, false)?;
    if value.closure_sha256 != digest(value)? {
        return Err(invalid("closure_sha256 does not match canonical preimage"));
    }
    wire::canonical_with_max(value, MAX_CLOSURE_BYTES)?;
    Ok(())
}

/// Seals one exact blank-identity Kernel decision reference closure copy.
///
/// # Errors
///
/// Returns an error for nonblank identity or any invalid nested record, graph edge, or preimage.
pub fn seal_closure(
    value: &KernelDecisionReferenceClosure,
) -> Result<KernelDecisionReferenceClosure, KernelDecisionContractError> {
    if !value.closure_id.is_empty() || !value.closure_sha256.is_empty() {
        return Err(invalid("sealing closure requires blank identity"));
    }
    let mut sealed = value.clone();
    let digest = digest(&sealed)?;
    sealed.closure_id = format!("{CLOSURE_PREFIX}{digest}");
    sealed.closure_sha256 = digest;
    validate_closure(&sealed)?;
    Ok(sealed)
}

/// Decodes exact compact canonical Kernel decision reference closure bytes.
///
/// # Errors
///
/// Returns an error for malformed, noncanonical, oversized, invalid, unresolved, or unsealed bytes.
pub fn decode_closure(
    bytes: &[u8],
) -> Result<KernelDecisionReferenceClosure, KernelDecisionContractError> {
    let value: KernelDecisionReferenceClosure = wire::decode_typed(bytes, MAX_CLOSURE_BYTES)?;
    let operational =
        crate::kernel_operational_contract::canonical_json(&value.operational_closure)
            .map_err(|error| invalid(format!("operational_closure encode: {}", error.message)))?;
    crate::kernel_operational_contract::decode_closure(operational.as_bytes())
        .map_err(|error| invalid(format!("operational_closure: {}", error.message)))?;
    validate_closure(&value)?;
    Ok(value)
}
