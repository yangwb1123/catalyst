use crate::platform_core_contract::{
    EntityRef, EntityType, MAX_EXECUTION_ELAPSED_MS, MAX_OBSERVED_COUNT, MAX_OBSERVED_QUANTITY,
    RecordRef, ScopeRef, validate_artifact_ref, validate_entity_ref, validate_executor_descriptor,
    validate_idempotency_key, validate_record_ref, validate_scope_ref,
};

use super::{
    AttemptBudget, AttemptRequestError, AttemptRequestErrorCode, AttemptRequestInput,
    ControlVersionBinding,
};

pub const MAX_APPROVAL_REFS: usize = 16;
pub const MAX_REQUESTED_EFFECTS: usize = 32;
pub const MAX_REQUESTED_EFFECT_BYTES: usize = 64;
pub const MAX_CONTROL_AGGREGATE_VERSION: i64 = 1_000_000_000;
pub const MAX_ATTEMPT_DURATION_MS: i64 = MAX_EXECUTION_ELAPSED_MS;
pub const MAX_ATTEMPT_TIMEOUT_MS: i64 = MAX_ATTEMPT_DURATION_MS;
pub const MAX_ATTEMPT_COST_USD_MICROS: i64 = MAX_OBSERVED_QUANTITY;
pub const MAX_ATTEMPT_MODEL_CALLS: i64 = MAX_OBSERVED_COUNT;
pub const MAX_ATTEMPT_TOOL_CALLS: i64 = MAX_OBSERVED_COUNT;
pub const MAX_ATTEMPT_INPUT_TOKENS: i64 = MAX_OBSERVED_QUANTITY;
pub const MAX_ATTEMPT_OUTPUT_TOKENS: i64 = MAX_OBSERVED_QUANTITY;
pub const MAX_ATTEMPT_OUTPUT_BYTES: i64 = MAX_OBSERVED_QUANTITY;
pub const MAX_ATTEMPT_NETWORK_BYTES: i64 = MAX_OBSERVED_QUANTITY;
pub const WORKSPACE_CAPABILITY_RECORD_TYPE: &str = "forge.runtime.workspace_capability";
pub const CAPABILITY_GRANT_RECORD_TYPE: &str = "forge.control.capability_grant";
pub const APPROVAL_RECORD_TYPE: &str = "forge.control.approval_record";

pub(super) fn validate(value: &AttemptRequestInput) -> Result<(), AttemptRequestError> {
    validate_scope(&value.scope_ref)?;
    validate_refs(value)?;
    validate_executor_descriptor(&value.executor)?;
    validate_optional_artifact(value)?;
    validate_record_bindings(value)?;
    validate_effects(&value.requested_effects)?;
    validate_budget(value.budget, value.timeout_ms)?;
    validate_idempotency_key(&value.idempotency_key)?;
    validate_control_versions(value.control_versions)
}

fn validate_scope(value: &ScopeRef) -> Result<(), AttemptRequestError> {
    validate_scope_ref(value)?;
    let complete = value.project_id.is_some()
        && value.project_snapshot_id.is_some()
        && value.objective_id.is_some()
        && value.change_id.is_some()
        && value.work_graph_id.is_some()
        && value.work_item_id.is_some()
        && value.attempt_id.is_some();
    if !complete
        || value.session_id.is_some()
        || value.turn_id.is_some()
        || value.action_id.is_some()
    {
        return Err(reference("scope_ref must be an exact full Attempt scope"));
    }
    Ok(())
}

fn validate_refs(value: &AttemptRequestInput) -> Result<(), AttemptRequestError> {
    validate_exact_ref(
        &value.attempt_ref,
        &EntityType::Attempt,
        value.scope_ref.attempt_id.as_deref(),
        "attempt_ref",
    )?;
    validate_exact_ref(
        &value.work_item_ref,
        &EntityType::WorkItem,
        value.scope_ref.work_item_id.as_deref(),
        "work_item_ref",
    )?;
    validate_exact_ref(
        &value.project_ref,
        &EntityType::Project,
        value.scope_ref.project_id.as_deref(),
        "project_ref",
    )?;
    validate_exact_ref(
        &value.project_snapshot_ref,
        &EntityType::ProjectSnapshot,
        value.scope_ref.project_snapshot_id.as_deref(),
        "project_snapshot_ref",
    )
}

fn validate_exact_ref(
    value: &EntityRef,
    expected_type: &EntityType,
    expected_id: Option<&str>,
    label: &str,
) -> Result<(), AttemptRequestError> {
    if &value.entity_type != expected_type {
        return Err(reference(format!(
            "{label} must exactly match its scope_ref identity"
        )));
    }
    validate_entity_ref(value, label)?;
    if Some(value.entity_id.as_str()) != expected_id {
        return Err(reference(format!(
            "{label} must exactly match its scope_ref identity"
        )));
    }
    Ok(())
}

fn validate_optional_artifact(value: &AttemptRequestInput) -> Result<(), AttemptRequestError> {
    let Some(artifact) = &value.context_artifact_ref else {
        return Ok(());
    };
    validate_artifact_ref(artifact)?;
    if artifact.source_snapshot_ref != value.project_snapshot_ref {
        return Err(reference(
            "context_artifact_ref must bind the exact project_snapshot_ref",
        ));
    }
    if artifact.producer_attempt_id == value.attempt_ref.entity_id {
        return Err(reference(
            "context_artifact_ref cannot be produced by the requested Attempt",
        ));
    }
    Ok(())
}

fn validate_record_bindings(value: &AttemptRequestInput) -> Result<(), AttemptRequestError> {
    validate_optional_record(
        value.workspace_capability_ref.as_ref(),
        WORKSPACE_CAPABILITY_RECORD_TYPE,
        "workspace_capability_ref",
    )?;
    validate_optional_record(
        value.grant_ref.as_ref(),
        CAPABILITY_GRANT_RECORD_TYPE,
        "grant_ref",
    )?;
    if !value.requested_effects.is_empty() && value.grant_ref.is_none() {
        return Err(reference("requested_effects require a declared grant_ref"));
    }
    validate_approvals(&value.approval_refs)
}

fn validate_optional_record(
    value: Option<&RecordRef>,
    expected_type: &str,
    label: &str,
) -> Result<(), AttemptRequestError> {
    let Some(value) = value else {
        return Ok(());
    };
    validate_record_ref(value, label)?;
    if value.record_type != expected_type {
        return Err(reference(format!(
            "{label}.record_type must equal {expected_type}"
        )));
    }
    Ok(())
}

fn validate_approvals(values: &[RecordRef]) -> Result<(), AttemptRequestError> {
    if values.len() > MAX_APPROVAL_REFS {
        return Err(invalid("approval_refs exceed the bounded entry limit"));
    }
    for (index, value) in values.iter().enumerate() {
        validate_record_ref(value, &format!("approval_refs[{index}]"))?;
        if value.record_type != APPROVAL_RECORD_TYPE {
            return Err(reference("approval_ref has the wrong record_type"));
        }
        if values[..index]
            .iter()
            .any(|prior| prior.record_id == value.record_id)
        {
            return Err(invalid("approval_refs contain a duplicate record_id"));
        }
    }
    Ok(())
}

fn validate_effects(values: &[String]) -> Result<(), AttemptRequestError> {
    if values.len() > MAX_REQUESTED_EFFECTS {
        return Err(invalid("requested_effects exceed the bounded entry limit"));
    }
    for (index, value) in values.iter().enumerate() {
        if !valid_effect(value) {
            return Err(invalid("requested_effect is not a bounded lowercase token"));
        }
        if values[..index].contains(value) {
            return Err(invalid("requested_effects must be unique"));
        }
    }
    Ok(())
}

fn valid_effect(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= MAX_REQUESTED_EFFECT_BYTES
        && bytes[0].is_ascii_lowercase()
        && (bytes[bytes.len() - 1].is_ascii_lowercase() || bytes[bytes.len() - 1].is_ascii_digit())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn validate_budget(budget: AttemptBudget, timeout_ms: i64) -> Result<(), AttemptRequestError> {
    positive_bound(
        budget.max_duration_ms,
        MAX_ATTEMPT_DURATION_MS,
        "budget.max_duration_ms",
    )?;
    bounded(
        budget.max_cost_usd_micros,
        MAX_ATTEMPT_COST_USD_MICROS,
        "budget.max_cost_usd_micros",
    )?;
    bounded(
        budget.max_model_calls,
        MAX_ATTEMPT_MODEL_CALLS,
        "budget.max_model_calls",
    )?;
    bounded(
        budget.max_tool_calls,
        MAX_ATTEMPT_TOOL_CALLS,
        "budget.max_tool_calls",
    )?;
    bounded(
        budget.max_input_tokens,
        MAX_ATTEMPT_INPUT_TOKENS,
        "budget.max_input_tokens",
    )?;
    bounded(
        budget.max_output_tokens,
        MAX_ATTEMPT_OUTPUT_TOKENS,
        "budget.max_output_tokens",
    )?;
    bounded(
        budget.max_output_bytes,
        MAX_ATTEMPT_OUTPUT_BYTES,
        "budget.max_output_bytes",
    )?;
    bounded(
        budget.max_network_bytes,
        MAX_ATTEMPT_NETWORK_BYTES,
        "budget.max_network_bytes",
    )?;
    if !(1..=MAX_ATTEMPT_TIMEOUT_MS).contains(&timeout_ms) || timeout_ms > budget.max_duration_ms {
        return Err(invalid(
            "timeout_ms must be positive and at most budget.max_duration_ms",
        ));
    }
    Ok(())
}

fn positive_bound(value: i64, maximum: i64, label: &str) -> Result<(), AttemptRequestError> {
    if !(1..=maximum).contains(&value) {
        return Err(invalid(format!("{label} is outside its bounded range")));
    }
    Ok(())
}

fn bounded(value: i64, maximum: i64, label: &str) -> Result<(), AttemptRequestError> {
    if !(0..=maximum).contains(&value) {
        return Err(invalid(format!("{label} is outside its bounded range")));
    }
    Ok(())
}

fn validate_control_versions(value: ControlVersionBinding) -> Result<(), AttemptRequestError> {
    let versions = [
        ("objective_version", value.objective_version),
        ("change_version", value.change_version),
        ("work_graph_version", value.work_graph_version),
        ("work_item_version", value.work_item_version),
    ];
    for (label, version) in versions {
        if !(1..=MAX_CONTROL_AGGREGATE_VERSION).contains(&version) {
            return Err(invalid(format!("control_versions.{label} is invalid")));
        }
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> AttemptRequestError {
    AttemptRequestError::new(AttemptRequestErrorCode::InvalidValue, message)
}

fn reference(message: impl Into<String>) -> AttemptRequestError {
    AttemptRequestError::new(AttemptRequestErrorCode::ReferenceMismatch, message)
}
