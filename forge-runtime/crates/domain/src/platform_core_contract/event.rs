use super::{
    EntityType, EventEnvelope, MAX_ENVELOPE_BYTES, MAX_UNIX_MILLISECONDS,
    PlatformCoreContractError, RejectionCode, SourceComponent, envelope, identity, invalid,
    references, reject, wire,
};

/// Validates a supplied event without appending or applying it.
///
/// # Errors
///
/// Returns an error for malformed identity, scope, payload, sequence, or bounds.
pub fn validate_event_envelope(value: &EventEnvelope) -> Result<(), PlatformCoreContractError> {
    let view = envelope_view(value);
    envelope::validate_common_values(&view)?;
    validate_event_values(value)?;
    envelope::validate_common_references(&view)?;
    validate_event_references(value)?;
    envelope::validate_common_relations(&view)?;
    validate_event_relations(value)?;
    wire::canonical_typed(value, MAX_ENVELOPE_BYTES).map(|_| ())
}

fn validate_event_values(value: &EventEnvelope) -> Result<(), PlatformCoreContractError> {
    identity::validate_typed_id(&value.event_id, "evt", "event_id")?;
    references::validate_entity_ref(&value.aggregate_ref, "aggregate_ref")?;
    if value.aggregate_version < 1 || value.sequence < 1 {
        return Err(invalid("aggregate_version and sequence must be positive"));
    }
    if !(1..=MAX_UNIX_MILLISECONDS).contains(&value.occurred_at_unix_ms) {
        return Err(invalid(format!(
            "occurred_at_unix_ms must be in 1..{MAX_UNIX_MILLISECONDS}"
        )));
    }
    if matches!(&value.source_component, SourceComponent::Unknown(_)) {
        return Err(reject(
            RejectionCode::ValueInvalid,
            "source_component is unsupported",
        ));
    }
    if let Some(source) = &value.source_snapshot_ref {
        references::validate_entity_ref(source, "source_snapshot_ref")?;
    }
    Ok(())
}

fn validate_event_references(value: &EventEnvelope) -> Result<(), PlatformCoreContractError> {
    if !references::scope_contains(&value.scope_ref, &value.aggregate_ref) {
        return Err(reject(
            RejectionCode::ReferenceMismatch,
            "aggregate_ref is not represented by scope_ref",
        ));
    }
    validate_event_snapshot(value)
}

fn validate_event_snapshot(value: &EventEnvelope) -> Result<(), PlatformCoreContractError> {
    match (
        value.scope_ref.project_snapshot_id.as_deref(),
        value.source_snapshot_ref.as_ref(),
    ) {
        (None, None) => Ok(()),
        (Some(scoped), Some(source))
            if source.entity_type == EntityType::ProjectSnapshot && source.entity_id == scoped =>
        {
            Ok(())
        }
        _ => Err(reject(
            RejectionCode::ReferenceMismatch,
            "source_snapshot_ref must exactly match scoped project_snapshot_id",
        )),
    }
}

fn validate_event_relations(value: &EventEnvelope) -> Result<(), PlatformCoreContractError> {
    if !identity::same_message_suffix(&value.message_id, &value.event_id) {
        return Err(reject(
            RejectionCode::RelationMismatch,
            "message_id and event_id suffixes must match",
        ));
    }
    if value
        .payload_artifact_ref
        .as_ref()
        .is_some_and(|artifact| artifact.created_at_unix_ms > value.occurred_at_unix_ms)
    {
        return Err(reject(
            RejectionCode::RelationMismatch,
            "event payload artifact cannot be created after occurrence time",
        ));
    }
    Ok(())
}

fn envelope_view(value: &EventEnvelope) -> envelope::EnvelopeView<'_> {
    envelope::EnvelopeView {
        canonicalization: &value.canonicalization,
        causation_id: &value.causation_id,
        correlation_id: &value.correlation_id,
        envelope_version: value.envelope_version,
        extensions: &value.extensions,
        message_id: &value.message_id,
        payload: &value.payload,
        payload_artifact_ref: &value.payload_artifact_ref,
        schema_name: &value.schema_name,
        schema_version: value.schema_version,
        scope_ref: &value.scope_ref,
        actor_ref: &value.actor_ref,
    }
}
