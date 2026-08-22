use sha2::{Digest, Sha256};

use super::{
    AtomRef, CANONICALIZATION, DecisionOption, DecisionTransaction, KernelDecisionContractError,
    MAX_TRANSACTION_BYTES, ProofObligation, TRANSACTION_API, TRANSACTION_DOMAIN, TRANSACTION_KIND,
    TRANSACTION_PREFIX, WritePrecondition, invalid,
    primitives::{
        attestations, bindings, capability, hash, identifier, identity, principal, strict_strings,
        string_set, task, text,
    },
    wire,
};

fn atom_ref(value: &AtomRef, label: &str) -> Result<(), KernelDecisionContractError> {
    hash(&value.atom_sha256, &format!("{label}.atom_sha256"))?;
    if value.atom_id != format!("{}{}", super::ATOM_PREFIX, value.atom_sha256) {
        return Err(invalid(format!("{label} atom_id does not bind digest")));
    }
    Ok(())
}

fn atom_refs(values: &[AtomRef], label: &str) -> Result<(), KernelDecisionContractError> {
    if values.len() > 64 {
        return Err(invalid(format!("{label} cardinality must be 0..=64")));
    }
    for value in values {
        atom_ref(value, label)?;
    }
    if !values
        .windows(2)
        .all(|pair| pair[0].atom_id.as_bytes() < pair[1].atom_id.as_bytes())
    {
        return Err(invalid(format!(
            "{label} must be strictly atom-id sorted and unique"
        )));
    }
    Ok(())
}

fn budget(value: &super::Budget) -> Result<(), KernelDecisionContractError> {
    let values = [
        (value.max_calls, 1_000_000_000_i64),
        (value.max_cost_usd_micros, 1_000_000_000_000_000),
        (value.max_input_tokens, 1_000_000_000),
        (value.max_network_bytes, 1_073_741_824),
        (value.max_output_bytes, 1_073_741_824),
        (value.max_output_tokens, 1_000_000_000),
        (value.timeout_ms, 86_400_000),
    ];
    if values
        .iter()
        .any(|(number, maximum)| !(0..=*maximum).contains(number))
        || value.max_calls == 0
        || value.timeout_ms == 0
    {
        return Err(invalid("budget field is outside its frozen range"));
    }
    Ok(())
}

fn compensation(value: &super::Compensation) -> Result<(), KernelDecisionContractError> {
    if !matches!(value.applicability.as_str(), "not_applicable" | "required") {
        return Err(invalid("compensation.applicability is unsupported"));
    }
    let capability_present = value.capability.is_some();
    let action_present = value.requested_action_sha256.is_some();
    if capability_present != action_present
        || (value.applicability == "required") != capability_present
    {
        return Err(invalid("compensation members do not match applicability"));
    }
    if let (Some(capability_value), Some(action)) =
        (&value.capability, &value.requested_action_sha256)
    {
        capability(capability_value)?;
        hash(action, "compensation.requested_action_sha256")?;
    }
    Ok(())
}

fn options(values: &[DecisionOption], selected: &str) -> Result<(), KernelDecisionContractError> {
    if values.is_empty() || values.len() > 16 {
        return Err(invalid("options cardinality must be 1..=16"));
    }
    for value in values {
        capability(&value.capability)?;
        identifier(&value.option_id, "option_id")?;
        hash(&value.requested_action_sha256, "requested_action_sha256")?;
    }
    let ids = values
        .iter()
        .map(|value| value.option_id.clone())
        .collect::<Vec<_>>();
    if !strict_strings(&ids) || !ids.iter().any(|value| value == selected) {
        return Err(invalid(
            "options must be sorted unique and selected_option_id must resolve",
        ));
    }
    Ok(())
}

fn proofs(values: &[ProofObligation]) -> Result<(), KernelDecisionContractError> {
    if values.is_empty() || values.len() > 32 {
        return Err(invalid("proof_obligations cardinality must be 1..=32"));
    }
    for value in values {
        identifier(&value.obligation_id, "obligation_id")?;
        hash(&value.predicate_sha256, "predicate_sha256")?;
        string_set(
            &value.required_evidence_kinds,
            "required_evidence_kinds",
            16,
            true,
        )?;
    }
    let ids = values
        .iter()
        .map(|value| value.obligation_id.clone())
        .collect::<Vec<_>>();
    if strict_strings(&ids) {
        Ok(())
    } else {
        Err(invalid(
            "proof_obligations must be strictly sorted and unique",
        ))
    }
}

fn receipt_refs(value: &DecisionTransaction) -> Result<(), KernelDecisionContractError> {
    if value.read_artifact_receipt_refs.len() > 32 {
        return Err(invalid(
            "read_artifact_receipt_refs cardinality must be 0..=32",
        ));
    }
    let mut ids = Vec::with_capacity(value.read_artifact_receipt_refs.len());
    for reference in &value.read_artifact_receipt_refs {
        identity(
            &reference.artifact_receipt_id,
            &reference.artifact_receipt_sha256,
            "artifact-receipt-",
            "artifact_receipt",
            false,
        )?;
        ids.push(reference.artifact_receipt_id.clone());
    }
    if strict_strings(&ids) {
        Ok(())
    } else {
        Err(invalid(
            "read_artifact_receipt_refs must be strictly sorted and unique",
        ))
    }
}

fn preconditions(values: &[WritePrecondition]) -> Result<(), KernelDecisionContractError> {
    if values.len() > 32 {
        return Err(invalid("write_preconditions cardinality must be 0..=32"));
    }
    for value in values {
        hash(&value.expected_sha256, "expected_sha256")?;
        identifier(&value.precondition_id, "precondition_id")?;
        text(&value.resource_ref, "resource_ref", 4096)?;
    }
    let ids = values
        .iter()
        .map(|value| value.precondition_id.clone())
        .collect::<Vec<_>>();
    if strict_strings(&ids) {
        Ok(())
    } else {
        Err(invalid(
            "write_preconditions must be strictly sorted and unique",
        ))
    }
}

fn roles(value: &DecisionTransaction) -> Result<(), KernelDecisionContractError> {
    atom_ref(&value.goal_atom_ref, "goal_atom_ref")?;
    atom_refs(&value.trigger_atom_refs, "trigger_atom_refs")?;
    atom_refs(&value.guard_atom_refs, "guard_atom_refs")?;
    let mut ids = vec![&value.goal_atom_ref.atom_id];
    ids.extend(value.trigger_atom_refs.iter().map(|item| &item.atom_id));
    ids.extend(value.guard_atom_refs.iter().map(|item| &item.atom_id));
    ids.sort_unstable();
    if ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(invalid("goal, trigger and guard roles must be disjoint"));
    }
    Ok(())
}

fn verifier(value: &DecisionTransaction) -> Result<(), KernelDecisionContractError> {
    capability(&value.verifier.capability)?;
    hash(
        &value.verifier.independence_basis_sha256,
        "independence_basis_sha256",
    )?;
    principal(&value.verifier.principal, "verifier.principal")?;
    if !(1..=86_400_000).contains(&value.verifier.timeout_ms) {
        return Err(invalid("verifier.timeout_ms is outside frozen range"));
    }
    if value.verifier.principal == value.actor
        || value.verifier.principal == value.accountable_owner
    {
        return Err(invalid(
            "verifier principal must differ from actor and accountable owner",
        ));
    }
    Ok(())
}

fn members(value: &DecisionTransaction) -> Result<(), KernelDecisionContractError> {
    attestations(&value.attestations)?;
    bindings(&value.bindings)?;
    task(&value.task_binding)?;
    principal(&value.actor, "actor")?;
    principal(&value.accountable_owner, "accountable_owner")?;
    budget(&value.budget)?;
    text(
        &value.completion_condition.condition_ref,
        "condition_ref",
        4096,
    )?;
    hash(
        &value.completion_condition.condition_sha256,
        "condition_sha256",
    )?;
    compensation(&value.compensation)
}

fn tail(value: &DecisionTransaction) -> Result<(), KernelDecisionContractError> {
    identifier(&value.idempotency_key, "idempotency_key")?;
    options(&value.options, &value.selected_option_id)?;
    hash(&value.selection_basis_sha256, "selection_basis_sha256")?;
    proofs(&value.proof_obligations)?;
    receipt_refs(value)?;
    verifier(value)?;
    preconditions(&value.write_preconditions)?;
    string_set(&value.write_slots, "write_slots", 32, false)
}

fn validate_body(
    value: &DecisionTransaction,
    allow_blank: bool,
) -> Result<(), KernelDecisionContractError> {
    if value.api_version != TRANSACTION_API
        || value.canonicalization != CANONICALIZATION
        || value.kind != TRANSACTION_KIND
        || value.transaction_mode != "structural_proposal_only"
    {
        return Err(invalid("DecisionTransaction constants differ"));
    }
    identity(
        &value.decision_transaction_id,
        &value.decision_transaction_sha256,
        TRANSACTION_PREFIX,
        "decision_transaction",
        allow_blank,
    )?;
    if value.created_at_unix_ms < 0 {
        return Err(invalid("created_at_unix_ms must be nonnegative"));
    }
    members(value)?;
    roles(value)?;
    tail(value)
}

fn digest(value: &DecisionTransaction) -> Result<String, KernelDecisionContractError> {
    let mut blank = value.clone();
    blank.decision_transaction_id.clear();
    blank.decision_transaction_sha256.clear();
    validate_body(&blank, true)?;
    let canonical = wire::canonical_with_max(&blank, MAX_TRANSACTION_BYTES)?;
    let mut hasher = Sha256::new();
    hasher.update(TRANSACTION_DOMAIN);
    hasher.update(canonical.as_bytes());
    Ok(crate::governance_contract::codec::lower_hex(
        &hasher.finalize(),
    ))
}

/// Validates one exact, sealed `DecisionTransaction` v1.
///
/// # Errors
///
/// Returns an error for any shape, role, declaration, bound, or digest violation.
pub fn validate_decision_transaction(
    value: &DecisionTransaction,
) -> Result<(), KernelDecisionContractError> {
    validate_body(value, false)?;
    if value.decision_transaction_sha256 != digest(value)? {
        return Err(invalid(
            "decision_transaction_sha256 does not match canonical preimage",
        ));
    }
    wire::canonical_with_max(value, MAX_TRANSACTION_BYTES)?;
    Ok(())
}

/// Seals one exact blank-identity `DecisionTransaction` v1 copy.
///
/// # Errors
///
/// Returns an error for nonblank identity or any invalid declaration or canonical preimage.
pub fn seal_decision_transaction(
    value: &DecisionTransaction,
) -> Result<DecisionTransaction, KernelDecisionContractError> {
    if !value.decision_transaction_id.is_empty() || !value.decision_transaction_sha256.is_empty() {
        return Err(invalid(
            "sealing DecisionTransaction requires blank identity",
        ));
    }
    let mut sealed = value.clone();
    let digest = digest(&sealed)?;
    sealed.decision_transaction_id = format!("{TRANSACTION_PREFIX}{digest}");
    sealed.decision_transaction_sha256 = digest;
    validate_decision_transaction(&sealed)?;
    Ok(sealed)
}

/// Decodes exact compact canonical `DecisionTransaction` v1 bytes.
///
/// # Errors
///
/// Returns an error for malformed, noncanonical, oversized, semantically invalid, or unsealed bytes.
pub fn decode_decision_transaction(
    bytes: &[u8],
) -> Result<DecisionTransaction, KernelDecisionContractError> {
    let value = wire::decode_typed(bytes, MAX_TRANSACTION_BYTES)?;
    validate_decision_transaction(&value)?;
    Ok(value)
}
