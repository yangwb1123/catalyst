use serde_json::{Map, Value};

use super::{
    ActorRef, ArtifactRef, CANONICALIZATION, ENVELOPE_VERSION, MAX_EXTENSION_FIELDS,
    MAX_EXTENSIONS_BYTES, MAX_PAYLOAD_BYTES, PlatformCoreContractError, RejectionCode, ScopeRef,
    artifact, identity, invalid, references, reject, wire,
};

pub(super) struct EnvelopeView<'a> {
    pub canonicalization: &'a str,
    pub causation_id: &'a Option<String>,
    pub correlation_id: &'a str,
    pub envelope_version: i64,
    pub extensions: &'a Map<String, Value>,
    pub message_id: &'a str,
    pub payload: &'a Option<Map<String, Value>>,
    pub payload_artifact_ref: &'a Option<ArtifactRef>,
    pub schema_name: &'a str,
    pub schema_version: i64,
    pub scope_ref: &'a ScopeRef,
    pub actor_ref: &'a ActorRef,
}

pub(super) fn validate_common_values(
    view: &EnvelopeView<'_>,
) -> Result<(), PlatformCoreContractError> {
    if view.canonicalization != CANONICALIZATION || view.envelope_version != ENVELOPE_VERSION {
        return Err(invalid(
            "envelope canonicalization or envelope_version is unsupported",
        ));
    }
    if view.schema_version < 1 {
        return Err(invalid("schema_version must be positive"));
    }
    validate_common_ids(view)?;
    wire::validate_schema_name(view.schema_name, "schema_name")?;
    references::validate_actor_ref(view.actor_ref)?;
    references::validate_scope_values(view.scope_ref)?;
    validate_body_values(view)
}

fn validate_common_ids(view: &EnvelopeView<'_>) -> Result<(), PlatformCoreContractError> {
    identity::validate_typed_id(view.message_id, "msg", "message_id")?;
    identity::validate_typed_id(view.correlation_id, "cor", "correlation_id")?;
    if let Some(causation_id) = view.causation_id {
        identity::validate_typed_id(causation_id, "msg", "causation_id")?;
    }
    Ok(())
}

fn validate_body_values(view: &EnvelopeView<'_>) -> Result<(), PlatformCoreContractError> {
    if let Some(payload) = view.payload {
        wire::canonical_object(payload, MAX_PAYLOAD_BYTES)
            .map_err(|error| invalid(format!("payload: {error}")))?;
    }
    if let Some(artifact_ref) = view.payload_artifact_ref {
        artifact::validate_artifact_values(artifact_ref)?;
    }
    validate_extensions(view.extensions)
}

pub(super) fn validate_common_references(
    view: &EnvelopeView<'_>,
) -> Result<(), PlatformCoreContractError> {
    references::validate_scope_references(view.scope_ref)?;
    let Some(artifact_ref) = view.payload_artifact_ref else {
        return Ok(());
    };
    artifact::validate_artifact_references(artifact_ref)?;
    if view.scope_ref.project_snapshot_id.as_deref()
        != Some(artifact_ref.source_snapshot_ref.entity_id.as_str())
    {
        return Err(reference_error(
            "payload_artifact_ref source snapshot must match scope",
        ));
    }
    if view.scope_ref.attempt_id.as_deref() != Some(artifact_ref.producer_attempt_id.as_str()) {
        return Err(reference_error(
            "payload_artifact_ref producer attempt must match scope",
        ));
    }
    Ok(())
}

fn reference_error(message: &'static str) -> PlatformCoreContractError {
    reject(RejectionCode::ReferenceMismatch, message)
}

pub(super) fn validate_common_relations(
    view: &EnvelopeView<'_>,
) -> Result<(), PlatformCoreContractError> {
    if view.causation_id.as_deref() == Some(view.message_id) {
        return Err(reject(
            RejectionCode::RelationMismatch,
            "causation_id must not equal message_id",
        ));
    }
    if view.payload.is_some() == view.payload_artifact_ref.is_some() {
        return Err(reject(
            RejectionCode::RelationMismatch,
            "exactly one of payload or payload_artifact_ref is required",
        ));
    }
    if let Some(artifact_ref) = view.payload_artifact_ref {
        artifact::validate_artifact_relations(artifact_ref)?;
    }
    Ok(())
}

fn validate_extensions(value: &Map<String, Value>) -> Result<(), PlatformCoreContractError> {
    if value.len() > MAX_EXTENSION_FIELDS {
        return Err(invalid(format!(
            "extensions exceeds {MAX_EXTENSION_FIELDS} fields"
        )));
    }
    for key in value.keys() {
        validate_extension_key(key)?;
    }
    wire::canonical_object(value, MAX_EXTENSIONS_BYTES)
        .map_err(|error| invalid(format!("extensions: {error}")))?;
    Ok(())
}

fn validate_extension_key(value: &str) -> Result<(), PlatformCoreContractError> {
    let parts: Vec<&str> = value.split('.').collect();
    if parts.len() != 2
        || !(2..=32).contains(&parts[0].len())
        || parts[1].is_empty()
        || parts[1].len() > 64
        || parts.iter().any(|part| !wire::lower_token(part))
    {
        return Err(invalid(format!(
            "extension key {value:?} must have two bounded namespace segments"
        )));
    }
    Ok(())
}

pub(super) fn validate_idempotency_key(value: &str) -> Result<(), PlatformCoreContractError> {
    if !(16..=128).contains(&value.len())
        || !value.bytes().all(|byte| (b'!'..=b'~').contains(&byte))
    {
        return Err(invalid(
            "idempotency_key must contain 16..128 visible ASCII bytes",
        ));
    }
    Ok(())
}
