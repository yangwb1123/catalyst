use std::collections::BTreeSet;

use serde_json::{Map, Value};

use crate::kernel_operational_contract::{
    CapabilityIdentity, OperationalBindings, Principal, TaskBinding,
};

use super::{DecisionAttestations, KernelDecisionContractError, invalid};

pub(super) fn text(
    value: &str,
    label: &str,
    maximum: usize,
) -> Result<(), KernelDecisionContractError> {
    if value.is_empty() || value.len() > maximum {
        return Err(invalid(format!(
            "{label} must be nonempty UTF-8 text <= {maximum} bytes"
        )));
    }
    if value.chars().any(forbidden_scalar) {
        return Err(invalid(format!(
            "{label} contains a forbidden Unicode scalar"
        )));
    }
    Ok(())
}

fn forbidden_scalar(value: char) -> bool {
    value.is_control()
        || matches!(
            value,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
}

pub(super) fn identifier(value: &str, label: &str) -> Result<(), KernelDecisionContractError> {
    if value.is_empty() || value.len() > 160 {
        return Err(invalid(format!("{label} is not a frozen identifier")));
    }
    let mut bytes = value.bytes();
    if !bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        || !bytes.all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'_' | b':' | b'/' | b'-')
        })
    {
        return Err(invalid(format!("{label} is not a frozen identifier")));
    }
    Ok(())
}

pub(super) fn hash(value: &str, label: &str) -> Result<(), KernelDecisionContractError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid(format!("{label} must be a lowercase bare SHA-256")));
    }
    Ok(())
}

pub(super) fn identity(
    id: &str,
    digest: &str,
    prefix: &str,
    label: &str,
    allow_blank: bool,
) -> Result<(), KernelDecisionContractError> {
    if allow_blank && id.is_empty() && digest.is_empty() {
        return Ok(());
    }
    hash(digest, &format!("{label}_sha256"))?;
    if id != format!("{prefix}{digest}") {
        return Err(invalid(format!("{label} identity does not bind digest")));
    }
    Ok(())
}

pub(super) fn attestations(
    value: &DecisionAttestations,
) -> Result<(), KernelDecisionContractError> {
    let values = [
        value.approval_authentication_attestation,
        value.authority_attestation,
        value.authorization_attestation,
        value.binding_authentication_attestation,
        value.cas_attestation,
        value.completion_attestation,
        value.content_provenance_attestation,
        value.effect_attestation,
        value.event_append_attestation,
        value.execution_attestation,
        value.grant_authentication_attestation,
        value.hard_guard_attestation,
        value.instruction_attestation,
        value.outcome_attestation,
        value.permission_attestation,
        value.persistence_attestation,
        value.principal_authentication_attestation,
        value.source_resolution_attestation,
        value.transition_attestation,
        value.truth_attestation,
        value.usage_measurement_attestation,
        value.verifier_independence_attestation,
    ];
    if values.iter().any(|item| *item) {
        Err(invalid(
            "all twenty-two decision attestations must be false",
        ))
    } else {
        Ok(())
    }
}

pub(super) fn principal(value: &Principal, label: &str) -> Result<(), KernelDecisionContractError> {
    text(
        &value.authority_domain,
        &format!("{label}.authority_domain"),
        160,
    )?;
    text(&value.principal_id, &format!("{label}.principal_id"), 160)
}

pub(super) fn task(value: &TaskBinding) -> Result<(), KernelDecisionContractError> {
    for item in [
        &value.change_id,
        &value.environment_id,
        &value.node_id,
        &value.project_id,
        &value.role,
        &value.run_id,
        &value.task_id,
    ] {
        text(item, "task_binding field", 160)?;
    }
    for item in [&value.attempt_id, &value.target_id].into_iter().flatten() {
        text(item, "task_binding optional field", 160)?;
    }
    Ok(())
}

pub(super) fn bindings(value: &OperationalBindings) -> Result<(), KernelDecisionContractError> {
    for digest in [
        &value.context_sha256,
        &value.environment_sha256,
        &value.policy_sha256,
        &value.source_tree_sha256,
    ] {
        hash(digest, "bindings digest")?;
    }
    for item in [
        &value.environment_profile_id,
        &value.source_profile_id,
        &value.source_revision,
    ] {
        text(item, "bindings field", 160)?;
    }
    Ok(())
}

pub(super) fn capability(value: &CapabilityIdentity) -> Result<(), KernelDecisionContractError> {
    hash(
        &value.capability_contract_sha256,
        "capability_contract_sha256",
    )?;
    text(&value.capability_id, "capability_id", 160)?;
    text(&value.capability_version, "capability_version", 160)
}

pub(super) fn string_set(
    values: &[String],
    label: &str,
    maximum: usize,
    nonempty: bool,
) -> Result<(), KernelDecisionContractError> {
    if values.len() > maximum || nonempty && values.is_empty() {
        return Err(invalid(format!(
            "{label} cardinality is outside frozen bounds"
        )));
    }
    for value in values {
        identifier(value, label)?;
    }
    if !strict_strings(values) {
        return Err(invalid(format!(
            "{label} must be strictly sorted and unique"
        )));
    }
    Ok(())
}

pub(super) fn strict_strings(values: &[String]) -> bool {
    values
        .windows(2)
        .all(|pair| pair[0].as_bytes() < pair[1].as_bytes())
}

pub(super) fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
    label: &str,
) -> Result<&'a Map<String, Value>, KernelDecisionContractError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(format!("{label} must be an object")))?;
    let expected = fields.iter().copied().collect::<BTreeSet<_>>();
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(invalid(format!("{label} fields differ")));
    }
    Ok(object)
}

pub(super) fn object_text<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, KernelDecisionContractError> {
    object[field]
        .as_str()
        .ok_or_else(|| invalid(format!("{field} must be text")))
}
