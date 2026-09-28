use super::{
    MAX_ARGUMENT_BYTES, MAX_ARGUMENT_TOTAL_BYTES, MAX_ARGUMENTS, MAX_OUTPUT_BYTES,
    MAX_SAFE_INTEGER, MAX_TIMEOUT_MS, MAX_TOKEN_BYTES, Map, RemoteError, Value, exact_fields,
    invalid_request, valid_identifier, validate_owner,
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
            "evaluated_at_ms",
        ],
    ) {
        return Err(invalid_request());
    }
    validate_identity(object)?;
    let command = object
        .get("command")
        .and_then(Value::as_object)
        .ok_or_else(invalid_request)?;
    validate_command_metadata(command)?;
    validate_command_resources(command)?;
    validate_proof_binding(command, object)?;
    Ok(())
}

fn validate_identity(object: &Map<String, Value>) -> Result<(), RemoteError> {
    let owner = object.get("owner").ok_or_else(invalid_request)?;
    validate_owner(owner)?;
    for field in ["conversation_id", "run_id", "attempt_id"] {
        let id = object
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(invalid_request)?;
        if !valid_identifier(id) {
            return Err(invalid_request());
        }
    }
    validate_attempt_state(object)?;
    Ok(())
}

fn validate_attempt_state(object: &Map<String, Value>) -> Result<(), RemoteError> {
    let state = object
        .get("attempt_state")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    if !matches!(
        state,
        "requested"
            | "accepted"
            | "starting"
            | "running"
            | "interrupted"
            | "completed"
            | "failed"
            | "uncertain"
    ) {
        return Err(invalid_request());
    }
    let evaluated = object
        .get("evaluated_at_ms")
        .and_then(Value::as_u64)
        .ok_or_else(invalid_request)?;
    if evaluated == 0 || evaluated > MAX_SAFE_INTEGER {
        return Err(invalid_request());
    }
    Ok(())
}

fn validate_command_metadata(command: &Map<String, Value>) -> Result<(), RemoteError> {
    if !exact_fields(
        command,
        [
            "v",
            "command_id",
            "lease_proof",
            "idempotency_key",
            "workspace_ref",
            "argv",
            "timeout_ms",
            "max_output_bytes",
        ],
    ) {
        return Err(invalid_request());
    }
    if command.get("v").and_then(Value::as_u64) != Some(1) {
        return Err(invalid_request());
    }
    let command_id = command
        .get("command_id")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    if !valid_identifier(command_id) {
        return Err(invalid_request());
    }
    for field in ["idempotency_key", "workspace_ref"] {
        let text = command
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(invalid_request)?;
        if text.is_empty()
            || text.len() > MAX_TOKEN_BYTES
            || text.trim() != text
            || text.chars().any(char::is_control)
        {
            return Err(invalid_request());
        }
    }
    Ok(())
}

fn validate_command_resources(command: &Map<String, Value>) -> Result<(), RemoteError> {
    let timeout = command
        .get("timeout_ms")
        .and_then(Value::as_u64)
        .ok_or_else(invalid_request)?;
    let output = command
        .get("max_output_bytes")
        .and_then(Value::as_u64)
        .ok_or_else(invalid_request)?;
    if timeout == 0 || timeout > MAX_TIMEOUT_MS || output == 0 || output > MAX_OUTPUT_BYTES {
        return Err(invalid_request());
    }
    let argv = command
        .get("argv")
        .and_then(Value::as_array)
        .ok_or_else(invalid_request)?;
    if argv.is_empty() || argv.len() > MAX_ARGUMENTS {
        return Err(invalid_request());
    }
    let mut total = 0usize;
    for (index, value) in argv.iter().enumerate() {
        let argument = value.as_str().ok_or_else(invalid_request)?;
        if argument.len() > MAX_ARGUMENT_BYTES
            || argument.chars().any(char::is_control)
            || (index == 0 && argument.is_empty())
        {
            return Err(invalid_request());
        }
        total = total
            .checked_add(argument.len())
            .ok_or_else(invalid_request)?;
    }
    if total > MAX_ARGUMENT_TOTAL_BYTES {
        return Err(invalid_request());
    }
    Ok(())
}

fn validate_proof_binding(
    command: &Map<String, Value>,
    object: &Map<String, Value>,
) -> Result<(), RemoteError> {
    let proof = command
        .get("lease_proof")
        .and_then(Value::as_object)
        .ok_or_else(invalid_request)?;
    if !exact_fields(proof, ["attempt_id", "target_id", "epoch", "fencing_token"]) {
        return Err(invalid_request());
    }
    let proof_attempt = proof
        .get("attempt_id")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    let proof_target = proof
        .get("target_id")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    if !valid_identifier(proof_attempt)
        || !valid_identifier(proof_target)
        || proof.get("epoch").and_then(Value::as_u64).is_none()
        || proof.get("epoch").and_then(Value::as_u64) == Some(0)
    {
        return Err(invalid_request());
    }
    let token = proof
        .get("fencing_token")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    if token.is_empty()
        || token.len() > MAX_TOKEN_BYTES
        || token.trim() != token
        || token.chars().any(char::is_control)
    {
        return Err(invalid_request());
    }
    if proof_attempt
        != object
            .get("attempt_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
    {
        return Err(invalid_request());
    }
    Ok(())
}
