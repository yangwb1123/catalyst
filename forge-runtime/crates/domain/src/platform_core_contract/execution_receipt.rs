use super::{
    ActorType, ArtifactRef, AttemptState, CANONICALIZATION, EntityRef, EntityType, EventRange,
    ExecutionReceipt, ExecutorDescriptor, MAX_EXECUTION_ELAPSED_MS, MAX_OBSERVED_COUNT,
    MAX_OBSERVED_QUANTITY, MAX_REASON_CODES, MAX_RECEIPT_ARTIFACTS, MAX_RECEIPT_BYTES,
    ObservedUsage, PlatformCoreContractError, RECEIPT_VERSION, RejectionCode, artifact, identity,
    recode, references, reject, wire,
};

const APPROVAL_RECORD_TYPE: &str = "forge.control.approval_record";
const GRANT_RECORD_TYPE: &str = "forge.control.capability_grant";

/// Validates supplied execution observations without resolving authority or advancing state.
///
/// # Errors
/// Returns a stable coded error for invalid declarations, references, state, or relations.
pub fn validate_execution_receipt(
    value: &ExecutionReceipt,
) -> Result<(), PlatformCoreContractError> {
    validate_header(value)?;
    validate_time_values(value)?;
    validate_executor(&value.executor)?;
    validate_declaration_values(value)?;
    validate_scope_bindings(value)?;
    validate_declaration_references(value)?;
    validate_terminal_state(value)?;
    validate_relations(value)?;
    wire::canonical_typed(value, MAX_RECEIPT_BYTES).map(|_| ())
}

fn validate_header(value: &ExecutionReceipt) -> Result<(), PlatformCoreContractError> {
    if value.canonicalization != CANONICALIZATION
        || value.execution_receipt_version != RECEIPT_VERSION
    {
        return Err(reject(
            RejectionCode::ValueInvalid,
            "execution receipt version or canonicalization is unsupported",
        ));
    }
    identity::validate_typed_id(&value.receipt_id, "rcp", "receipt_id")
}

fn validate_scope_bindings(value: &ExecutionReceipt) -> Result<(), PlatformCoreContractError> {
    references::validate_scope_references(&value.scope_ref)?;
    let Some(snapshot_id) = value.scope_ref.project_snapshot_id.as_deref() else {
        return Err(scope_error());
    };
    let Some(attempt_id) = value.scope_ref.attempt_id.as_deref() else {
        return Err(scope_error());
    };
    let Some(session_id) = value.scope_ref.session_id.as_deref() else {
        return Err(scope_error());
    };
    if value.scope_ref.turn_id.is_some() || value.scope_ref.action_id.is_some() {
        return Err(scope_error());
    }
    validate_exact_ref(
        &value.attempt_ref,
        &EntityType::Attempt,
        attempt_id,
        "attempt_ref",
    )?;
    validate_exact_ref(
        &value.session_ref,
        &EntityType::Session,
        session_id,
        "session_ref",
    )?;
    validate_exact_ref(
        &value.source_snapshot_ref,
        &EntityType::ProjectSnapshot,
        snapshot_id,
        "source_snapshot_ref",
    )
}

fn scope_error() -> PlatformCoreContractError {
    reject(
        RejectionCode::ReferenceMismatch,
        "execution receipt requires exact session-level snapshot scope",
    )
}

fn validate_exact_ref(
    value: &EntityRef,
    entity_type: &EntityType,
    identifier: &str,
    label: &str,
) -> Result<(), PlatformCoreContractError> {
    if &value.entity_type == entity_type && value.entity_id == identifier {
        Ok(())
    } else {
        Err(reject(
            RejectionCode::ReferenceMismatch,
            format!("{label} must exactly match scope_ref"),
        ))
    }
}

fn validate_time_values(value: &ExecutionReceipt) -> Result<(), PlatformCoreContractError> {
    validate_unix_ms(value.started_at_unix_ms, "started_at_unix_ms")?;
    validate_unix_ms(value.ended_at_unix_ms, "ended_at_unix_ms")?;
    validate_observed_usage(&value.observed_usage)
}

fn validate_terminal_state(value: &ExecutionReceipt) -> Result<(), PlatformCoreContractError> {
    if !matches!(
        &value.terminal_state,
        AttemptState::Interrupted
            | AttemptState::Completed
            | AttemptState::Failed
            | AttemptState::Uncertain
    ) {
        return Err(reject(
            RejectionCode::StateInvalid,
            "terminal_state is not an Attempt terminal state",
        ));
    }
    Ok(())
}

fn validate_executor(value: &ExecutorDescriptor) -> Result<(), PlatformCoreContractError> {
    references::validate_actor_ref(&value.actor_ref)?;
    if !matches!(
        &value.actor_ref.actor_type,
        ActorType::Agent | ActorType::Service | ActorType::System
    ) {
        return Err(reject(
            RejectionCode::ValueInvalid,
            "executor actor_type must be agent, service, or system",
        ));
    }
    wire::validate_schema_name(&value.adapter_id, "executor.adapter_id")?;
    validate_adapter_version(&value.adapter_version)
}

fn validate_adapter_version(value: &str) -> Result<(), PlatformCoreContractError> {
    if !(5..=32).contains(&value.len()) {
        return Err(adapter_version_error());
    }
    let parts: Vec<&str> = value.split('.').collect();
    if parts.len() != 3 || parts.iter().any(|part| !version_part(part)) {
        return Err(adapter_version_error());
    }
    Ok(())
}

fn version_part(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 9
        && (value.len() == 1 || !value.starts_with('0'))
        && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn adapter_version_error() -> PlatformCoreContractError {
    reject(
        RejectionCode::ValueInvalid,
        "adapter_version must be canonical bounded major.minor.patch",
    )
}

fn validate_observed_usage(value: &ObservedUsage) -> Result<(), PlatformCoreContractError> {
    let checks = [
        (
            "cost_usd_micros",
            value.cost_usd_micros,
            MAX_OBSERVED_QUANTITY,
        ),
        ("elapsed_ms", value.elapsed_ms, MAX_EXECUTION_ELAPSED_MS),
        ("input_tokens", value.input_tokens, MAX_OBSERVED_QUANTITY),
        ("model_calls", value.model_calls, MAX_OBSERVED_COUNT),
        ("network_bytes", value.network_bytes, MAX_OBSERVED_QUANTITY),
        ("output_bytes", value.output_bytes, MAX_OBSERVED_QUANTITY),
        ("output_tokens", value.output_tokens, MAX_OBSERVED_QUANTITY),
        ("tool_calls", value.tool_calls, MAX_OBSERVED_COUNT),
    ];
    for (label, number, maximum) in checks {
        if !(0..=maximum).contains(&number) {
            return Err(reject(
                RejectionCode::ValueInvalid,
                format!("observed_usage.{label} must be in 0..{maximum}"),
            ));
        }
    }
    Ok(())
}

fn validate_declaration_values(value: &ExecutionReceipt) -> Result<(), PlatformCoreContractError> {
    if let Some(reference) = &value.approval_ref {
        references::validate_record_ref(reference, "approval_ref")?;
    }
    if let Some(reference) = &value.grant_ref {
        references::validate_record_ref(reference, "grant_ref")?;
    }
    validate_reference_values(value)?;
    validate_artifact_set_values(&value.input_artifact_refs, "input_artifact_refs")?;
    validate_artifact_set_values(&value.output_artifact_refs, "output_artifact_refs")?;
    validate_event_range_values(value.event_range.as_ref())?;
    validate_reason_codes(&value.reason_codes, "reason_codes")
}

fn validate_reference_values(value: &ExecutionReceipt) -> Result<(), PlatformCoreContractError> {
    references::validate_scope_values(&value.scope_ref)?;
    references::validate_entity_ref(&value.attempt_ref, "attempt_ref")?;
    references::validate_entity_ref(&value.session_ref, "session_ref")?;
    references::validate_entity_ref(&value.source_snapshot_ref, "source_snapshot_ref")
}

fn validate_artifact_set_values(
    values: &[ArtifactRef],
    label: &str,
) -> Result<(), PlatformCoreContractError> {
    if values.len() > MAX_RECEIPT_ARTIFACTS {
        return Err(reject(
            RejectionCode::ValueInvalid,
            format!("{label} exceeds {MAX_RECEIPT_ARTIFACTS} items"),
        ));
    }
    let mut previous = "";
    for value in values {
        artifact::validate_artifact_values(value)?;
        if value.logical_id.as_str() <= previous {
            return Err(reject(
                RejectionCode::ValueInvalid,
                format!("{label} must be strictly sorted by logical_id"),
            ));
        }
        previous = &value.logical_id;
    }
    Ok(())
}

fn validate_declaration_references(
    value: &ExecutionReceipt,
) -> Result<(), PlatformCoreContractError> {
    validate_record_role(
        value.approval_ref.as_ref(),
        APPROVAL_RECORD_TYPE,
        "approval_ref",
    )?;
    validate_record_role(value.grant_ref.as_ref(), GRANT_RECORD_TYPE, "grant_ref")?;
    for artifact in value
        .input_artifact_refs
        .iter()
        .chain(&value.output_artifact_refs)
    {
        artifact::validate_artifact_references(artifact)?;
        if artifact.source_snapshot_ref.entity_id != value.source_snapshot_ref.entity_id {
            return Err(reject(
                RejectionCode::ReferenceMismatch,
                "execution artifact source snapshot must match receipt",
            ));
        }
    }
    if value
        .event_range
        .as_ref()
        .is_some_and(|event| event.aggregate_ref != value.attempt_ref)
    {
        return Err(reject(
            RejectionCode::ReferenceMismatch,
            "event_range aggregate must equal attempt_ref",
        ));
    }
    Ok(())
}

fn validate_record_role(
    value: Option<&super::RecordRef>,
    expected: &str,
    label: &str,
) -> Result<(), PlatformCoreContractError> {
    if value.is_some_and(|reference| reference.record_type != expected) {
        return Err(reject(
            RejectionCode::ReferenceMismatch,
            format!("{label} must declare record_type {expected}"),
        ));
    }
    Ok(())
}

fn validate_relations(value: &ExecutionReceipt) -> Result<(), PlatformCoreContractError> {
    validate_execution_interval(value)?;
    let elapsed = value.ended_at_unix_ms - value.started_at_unix_ms;
    if value.observed_usage.elapsed_ms != elapsed {
        return Err(reject(
            RejectionCode::RelationMismatch,
            "observed_usage.elapsed_ms must equal execution wall interval",
        ));
    }
    for artifact in &value.input_artifact_refs {
        artifact::validate_artifact_relations(artifact)?;
        validate_artifact_relation(artifact, value, false)?;
    }
    for artifact in &value.output_artifact_refs {
        artifact::validate_artifact_relations(artifact)?;
        validate_artifact_relation(artifact, value, true)?;
    }
    validate_event_range_relation(value.event_range.as_ref())?;
    validate_terminal_reason_relation(&value.terminal_state, &value.reason_codes)
}

fn validate_execution_interval(value: &ExecutionReceipt) -> Result<(), PlatformCoreContractError> {
    match value.ended_at_unix_ms.checked_sub(value.started_at_unix_ms) {
        Some(duration) if (0..=MAX_EXECUTION_ELAPSED_MS).contains(&duration) => Ok(()),
        _ => Err(reject(
            RejectionCode::RelationMismatch,
            "execution wall interval is invalid or exceeds maximum",
        )),
    }
}

fn validate_artifact_relation(
    value: &ArtifactRef,
    receipt: &ExecutionReceipt,
    output: bool,
) -> Result<(), PlatformCoreContractError> {
    if !output && value.created_at_unix_ms > receipt.started_at_unix_ms {
        return Err(reject(
            RejectionCode::RelationMismatch,
            "input artifact cannot postdate execution start",
        ));
    }
    if output
        && (value.producer_attempt_id != receipt.attempt_ref.entity_id
            || value.created_at_unix_ms < receipt.started_at_unix_ms
            || value.created_at_unix_ms > receipt.ended_at_unix_ms)
    {
        return Err(reject(
            RejectionCode::RelationMismatch,
            "output artifact must be produced by the attempt during execution",
        ));
    }
    Ok(())
}

fn validate_event_range_values(
    value: Option<&EventRange>,
) -> Result<(), PlatformCoreContractError> {
    let Some(value) = value else {
        return Ok(());
    };
    references::validate_entity_ref(&value.aggregate_ref, "event_range.aggregate_ref")?;
    identity::validate_typed_id(&value.first_event_id, "evt", "event_range.first_event_id")?;
    identity::validate_typed_id(&value.last_event_id, "evt", "event_range.last_event_id")?;
    if value.first_sequence < 1 || value.last_sequence < 1 {
        return Err(reject(
            RejectionCode::ValueInvalid,
            "event_range sequences must be positive",
        ));
    }
    Ok(())
}

fn validate_event_range_relation(
    value: Option<&EventRange>,
) -> Result<(), PlatformCoreContractError> {
    if value.is_some_and(|event| event.last_sequence < event.first_sequence) {
        return Err(reject(
            RejectionCode::RelationMismatch,
            "event_range sequence interval is reversed",
        ));
    }
    if value.is_some_and(|event| {
        (event.first_sequence == event.last_sequence)
            != (event.first_event_id == event.last_event_id)
    }) {
        return Err(reject(
            RejectionCode::RelationMismatch,
            "event_range identity and sequence cardinality disagree",
        ));
    }
    Ok(())
}

fn validate_terminal_reason_relation(
    state: &AttemptState,
    values: &[String],
) -> Result<(), PlatformCoreContractError> {
    if (state == &AttemptState::Completed) == values.is_empty() {
        Ok(())
    } else {
        Err(reject(
            RejectionCode::RelationMismatch,
            "completed requires no reasons; other terminals require reasons",
        ))
    }
}

pub(super) fn validate_reason_codes(
    values: &[String],
    label: &str,
) -> Result<(), PlatformCoreContractError> {
    if values.len() > MAX_REASON_CODES {
        return Err(reject(
            RejectionCode::ValueInvalid,
            format!("{label} exceeds {MAX_REASON_CODES} items"),
        ));
    }
    let mut previous = "";
    for value in values {
        if !wire::lower_token(value) || value.len() > 64 || value.as_str() <= previous {
            return Err(reject(
                RejectionCode::ValueInvalid,
                format!("{label} must be strictly sorted unique lowercase tokens"),
            ));
        }
        previous = value;
    }
    Ok(())
}

fn validate_unix_ms(value: i64, label: &str) -> Result<(), PlatformCoreContractError> {
    if (1..=super::MAX_UNIX_MILLISECONDS).contains(&value) {
        Ok(())
    } else {
        Err(recode(
            super::invalid(format!("{label} is outside the supported range")),
            RejectionCode::ValueInvalid,
        ))
    }
}
