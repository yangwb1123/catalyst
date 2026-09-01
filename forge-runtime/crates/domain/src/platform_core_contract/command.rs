use super::{
    CommandEnvelope, MAX_ENVELOPE_BYTES, MAX_UNIX_MILLISECONDS, PlatformCoreContractError,
    RejectionCode, envelope, identity, invalid, references, reject, wire,
};

/// Validates a supplied command without authorizing or dispatching it.
///
/// # Errors
///
/// Returns an error for malformed identity, scope, payload, timing, or bounds.
pub fn validate_command_envelope(value: &CommandEnvelope) -> Result<(), PlatformCoreContractError> {
    let view = envelope_view(value);
    envelope::validate_common_values(&view)?;
    validate_command_values(value)?;
    envelope::validate_common_references(&view)?;
    validate_command_references(value)?;
    envelope::validate_common_relations(&view)?;
    validate_command_relations(value)?;
    wire::canonical_typed(value, MAX_ENVELOPE_BYTES).map(|_| ())
}

fn validate_command_values(value: &CommandEnvelope) -> Result<(), PlatformCoreContractError> {
    identity::validate_typed_id(&value.command_id, "cmd", "command_id")?;
    references::validate_entity_ref(&value.target_ref, "target_ref")?;
    validate_unix_ms(value.issued_at_unix_ms, "issued_at_unix_ms")?;
    if let Some(deadline) = value.deadline_unix_ms {
        validate_unix_ms(deadline, "deadline_unix_ms")?;
    }
    if value.expected_version.is_some_and(|version| version < 0) {
        return Err(invalid("expected_version must be null or nonnegative"));
    }
    envelope::validate_idempotency_key(&value.idempotency_key)?;
    if let Some(reference) = &value.authorization_ref {
        references::validate_record_ref(reference, "authorization_ref")?;
    }
    Ok(())
}

fn validate_command_references(value: &CommandEnvelope) -> Result<(), PlatformCoreContractError> {
    if references::scope_contains(&value.scope_ref, &value.target_ref) {
        Ok(())
    } else {
        Err(reject(
            RejectionCode::ReferenceMismatch,
            "target_ref is not represented by scope_ref",
        ))
    }
}

fn validate_command_relations(value: &CommandEnvelope) -> Result<(), PlatformCoreContractError> {
    if !identity::same_message_suffix(&value.message_id, &value.command_id) {
        return Err(reject(
            RejectionCode::RelationMismatch,
            "message_id and command_id suffixes must match",
        ));
    }
    if value
        .deadline_unix_ms
        .is_some_and(|deadline| deadline < value.issued_at_unix_ms)
    {
        return Err(reject(
            RejectionCode::RelationMismatch,
            "deadline_unix_ms must not precede issued_at_unix_ms",
        ));
    }
    if value
        .payload_artifact_ref
        .as_ref()
        .is_some_and(|artifact| artifact.created_at_unix_ms > value.issued_at_unix_ms)
    {
        return Err(reject(
            RejectionCode::RelationMismatch,
            "command payload artifact cannot be created after issue time",
        ));
    }
    Ok(())
}

fn validate_unix_ms(value: i64, label: &str) -> Result<(), PlatformCoreContractError> {
    if (1..=MAX_UNIX_MILLISECONDS).contains(&value) {
        Ok(())
    } else {
        Err(invalid(format!(
            "{label} must be in 1..{MAX_UNIX_MILLISECONDS}"
        )))
    }
}

fn envelope_view(value: &CommandEnvelope) -> envelope::EnvelopeView<'_> {
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
