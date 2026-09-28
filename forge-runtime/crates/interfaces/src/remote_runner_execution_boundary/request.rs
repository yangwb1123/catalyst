use super::{
    MAX_NONCE_BYTES, MAX_OWNER_PART_BYTES, MAX_PAYLOAD_BYTES, MAX_SAFE_INTEGER, Map, RemoteError,
    RunnerCommand, TRANSPORT_EVALUATION_MODE, TRANSPORT_SCHEMA_VERSION, Value, exact_fields,
    invalid_request, valid_attempt_state, valid_digest, valid_effect_state, valid_identifier,
};

pub(in super::super) fn validate_request(value: &Value) -> Result<(), RemoteError> {
    let object = value.as_object().ok_or_else(invalid_request)?;
    if !exact_fields(
        object,
        [
            "owner",
            "conversation_id",
            "run_id",
            "attempt_id",
            "attempt_state",
            "command",
            "transport",
            "expected_payload_sha256",
            "controls",
        ],
    ) {
        return Err(invalid_request());
    }
    validate_request_identity(object)?;
    let command_value = object.get("command").ok_or_else(invalid_request)?;
    let command: RunnerCommand =
        serde_json::from_value(command_value.clone()).map_err(|_| invalid_request())?;
    command.validate().map_err(|_| invalid_request())?;
    if command.lease_proof.attempt_id
        != object
            .get("attempt_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
    {
        return Err(invalid_request());
    }
    validate_transport(object.get("transport").ok_or_else(invalid_request)?)?;
    let expected = object
        .get("expected_payload_sha256")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    if !valid_digest(expected) {
        return Err(invalid_request());
    }
    validate_controls(object.get("controls").ok_or_else(invalid_request)?)
}

pub(super) fn validate_controls(value: &Value) -> Result<(), RemoteError> {
    let object = value.as_object().ok_or_else(invalid_request)?;
    if !exact_fields(object, ["effect_state", "cancellation_requested"]) {
        return Err(invalid_request());
    }
    let effect_state = object
        .get("effect_state")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    if !valid_effect_state(effect_state)
        || !object
            .get("cancellation_requested")
            .is_some_and(Value::is_boolean)
    {
        return Err(invalid_request());
    }
    Ok(())
}

pub(super) fn validate_transport(value: &Value) -> Result<(), RemoteError> {
    let object = value.as_object().ok_or_else(invalid_request)?;
    validate_transport_header(object)?;
    let path = object
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    let timestamp = object
        .get("timestamp")
        .and_then(Value::as_i64)
        .ok_or_else(invalid_request)?;
    let nonce = object
        .get("nonce")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    let payload_bytes = object
        .get("payload_bytes")
        .and_then(Value::as_u64)
        .ok_or_else(invalid_request)?;
    if !path.starts_with("/api/v1/runners/")
        || !path.ends_with("/dispatch")
        || path.len() > 2_048
        || timestamp <= 0
        || timestamp.cast_unsigned() > MAX_SAFE_INTEGER
        || nonce.is_empty()
        || nonce.len() > MAX_NONCE_BYTES
        || nonce.chars().any(char::is_control)
        || !valid_digest(
            object
                .get("payload_sha256")
                .and_then(Value::as_str)
                .ok_or_else(invalid_request)?,
        )
        || payload_bytes == 0
        || payload_bytes > MAX_PAYLOAD_BYTES as u64
        || object.get("replay_checked") != Some(&Value::Bool(true))
    {
        return Err(invalid_request());
    }
    validate_transport_authority(object)?;
    Ok(())
}

pub(super) fn validate_owner(value: &Value) -> Result<(), RemoteError> {
    let object = value.as_object().ok_or_else(invalid_request)?;
    if !exact_fields(object, ["issuer", "subject", "tenant_id"]) {
        return Err(invalid_request());
    }
    for field in ["issuer", "subject", "tenant_id"] {
        let part = object
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(invalid_request)?;
        if part.is_empty()
            || part.len() > MAX_OWNER_PART_BYTES
            || part.trim() != part
            || part.chars().any(char::is_control)
        {
            return Err(invalid_request());
        }
    }
    Ok(())
}

fn validate_request_identity(object: &Map<String, Value>) -> Result<(), RemoteError> {
    validate_owner(object.get("owner").ok_or_else(invalid_request)?)?;
    for field in ["conversation_id", "run_id", "attempt_id"] {
        let id = object
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(invalid_request)?;
        if !valid_identifier(id) {
            return Err(invalid_request());
        }
    }
    let state = object
        .get("attempt_state")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    if !valid_attempt_state(state) {
        return Err(invalid_request());
    }
    Ok(())
}

fn validate_transport_header(object: &Map<String, Value>) -> Result<(), RemoteError> {
    if !exact_fields(
        object,
        [
            "schema_version",
            "evaluation_mode",
            "method",
            "path",
            "timestamp",
            "nonce",
            "payload_sha256",
            "payload_bytes",
            "replay_checked",
            "preview_only",
            "authority",
        ],
    ) {
        return Err(invalid_request());
    }
    if object.get("schema_version").and_then(Value::as_str) != Some(TRANSPORT_SCHEMA_VERSION)
        || object.get("evaluation_mode").and_then(Value::as_str) != Some(TRANSPORT_EVALUATION_MODE)
        || object.get("method").and_then(Value::as_str) != Some("POST")
        || object.get("preview_only") != Some(&Value::Bool(true))
    {
        return Err(invalid_request());
    }
    Ok(())
}

fn validate_transport_authority(object: &Map<String, Value>) -> Result<(), RemoteError> {
    let authority = object
        .get("authority")
        .and_then(Value::as_object)
        .ok_or_else(invalid_request)?;
    if !exact_fields(
        authority,
        [
            "identity_verified",
            "heartbeat_accepted",
            "lease_issued",
            "reservation_created",
            "execution_authorized",
            "dispatch_performed",
            "audit_published",
        ],
    ) || authority.values().any(|v| v != &Value::Bool(false))
    {
        return Err(invalid_request());
    }
    Ok(())
}
