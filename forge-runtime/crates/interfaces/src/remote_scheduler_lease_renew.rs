//! Strict client for the accepted EXECUTE scheduler-lease renewal.
//!
//! Renewal submits the current lease proof once and validates the returned
//! fenced receipt. The human TUI reuses the existing lease renderer, which
//! withholds fencing material from terminal output.

use std::{fs::File, io::Read, path::Path};

use serde_json::{Map, Value};

use super::{RemoteError, scheduler_lease};

const MAX_INPUT_BYTES: usize = 512 * 1024;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_TOKEN_BYTES: usize = 256;
const MIN_TTL_MS: u64 = 1_000;
const MAX_TTL_MS: u64 = 600_000;

pub(super) fn read_request(input: &str) -> Result<Value, RemoteError> {
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes).map_err(|_| {
        RemoteError("remote scheduler lease renewal input contains duplicate JSON keys".into())
    })?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| RemoteError("remote scheduler lease renewal input is invalid JSON".into()))?;
    validate_request(&value)?;
    Ok(value)
}

pub(super) fn read_tui_request(input: &str) -> Result<Value, RemoteError> {
    if input == "-" {
        return Err(RemoteError(
            "remote TUI scheduler lease renewal requires a file path; '-' belongs to the standalone CLI"
                .into(),
        ));
    }
    read_request(input)
}

pub(super) fn validate_request(value: &Value) -> Result<(), RemoteError> {
    let object = value.as_object().ok_or_else(invalid_request)?;
    if !exact_fields(
        object,
        [
            "conversation_id",
            "run_id",
            "attempt_id",
            "target_id",
            "epoch",
            "fencing_token",
            "ttl_ms",
        ],
    ) {
        return Err(invalid_request());
    }
    for field in ["conversation_id", "run_id", "attempt_id", "target_id"] {
        let value = object
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(invalid_request)?;
        if !valid_identifier(value) {
            return Err(invalid_request());
        }
    }
    let epoch = object
        .get("epoch")
        .and_then(Value::as_u64)
        .ok_or_else(invalid_request)?;
    if epoch == 0 || epoch > MAX_SAFE_INTEGER {
        return Err(invalid_request());
    }
    let token = object
        .get("fencing_token")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    if !valid_token(token) {
        return Err(invalid_request());
    }
    let ttl = object
        .get("ttl_ms")
        .and_then(Value::as_u64)
        .ok_or_else(invalid_request)?;
    if !(MIN_TTL_MS..=MAX_TTL_MS).contains(&ttl) {
        return Err(invalid_request());
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn validate_response(value: &Value) -> Result<(), RemoteError> {
    scheduler_lease::validate_response(value).map(|_| ())
}

/// Validates a renewal receipt against the proof being renewed. Renewal
/// advances the fencing epoch, so a response for another target or epoch is
/// never returned to the caller as the current lease.
pub(super) fn validate_response_for_request(
    value: &Value,
    request: &Value,
) -> Result<(), RemoteError> {
    validate_request(request)?;
    scheduler_lease::validate_response(value)?;
    let request_object = request.as_object().ok_or_else(invalid_request)?;
    for field in ["conversation_id", "run_id", "attempt_id"] {
        if request_object.get(field) != value.get(field) {
            return Err(RemoteError(
                "Forge API returned a scheduler lease renewal for a different request".into(),
            ));
        }
    }
    let target_id = request_object
        .get("target_id")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    if value.get("instance_id").and_then(Value::as_str) != Some(target_id)
        || value
            .get("grant")
            .and_then(|grant| grant.get("target_id"))
            .and_then(Value::as_str)
            != Some(target_id)
    {
        return Err(RemoteError(
            "Forge API returned a scheduler lease renewal for a different target".into(),
        ));
    }
    let request_epoch = request_object
        .get("epoch")
        .and_then(Value::as_u64)
        .ok_or_else(invalid_request)?;
    let expected_epoch = request_epoch.checked_add(1).ok_or_else(invalid_request)?;
    if value
        .get("grant")
        .and_then(|grant| grant.get("epoch"))
        .and_then(Value::as_u64)
        != Some(expected_epoch)
    {
        return Err(RemoteError(
            "Forge API returned a scheduler lease renewal with an unexpected epoch".into(),
        ));
    }
    Ok(())
}

pub(super) fn render_human(value: &Value, writer: &mut impl std::io::Write) -> std::io::Result<()> {
    let result = scheduler_lease::validate_response(value)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error.0))?;
    scheduler_lease::render_human(&result, writer)
}

fn exact_fields<const N: usize>(object: &Map<String, Value>, fields: [&str; N]) -> bool {
    object.len() == N && fields.into_iter().all(|field| object.contains_key(field))
}

fn valid_identifier(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_TOKEN_BYTES {
        return false;
    }
    value.chars().enumerate().all(|(index, character)| {
        character.is_ascii_alphanumeric()
            || (index > 0 && matches!(character, '.' | '_' | ':' | '+' | '/' | '-'))
    })
}

fn valid_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_TOKEN_BYTES
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn invalid_request() -> RemoteError {
    RemoteError("remote scheduler lease renewal input is invalid".into())
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, RemoteError> {
    let path = Path::new(input);
    if input == "-" {
        return Err(RemoteError(
            "scheduler lease renewal input must be a regular file for this command".into(),
        ));
    }
    let metadata = std::fs::metadata(path)
        .map_err(|_| RemoteError("could not read scheduler lease renewal input".into()))?;
    if !metadata.is_file() {
        return Err(RemoteError(
            "scheduler lease renewal input must be a regular file".into(),
        ));
    }
    let mut bytes = Vec::with_capacity(MAX_INPUT_BYTES.min(64 * 1024));
    File::open(path)
        .map_err(|_| RemoteError("could not read scheduler lease renewal input".into()))?
        .take((MAX_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| RemoteError("could not read scheduler lease renewal input".into()))?;
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(RemoteError(format!(
            "scheduler lease renewal input exceeds {MAX_INPUT_BYTES} bytes"
        )));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request() -> Value {
        json!({
            "conversation_id": "conversation-1",
            "run_id": "run-1",
            "attempt_id": "attempt-1",
            "target_id": "runner-a",
            "epoch": 1,
            "fencing_token": "token-a",
            "ttl_ms": 30000,
        })
    }

    #[test]
    fn renewal_request_is_strict_and_bounded() {
        validate_request(&request()).expect("valid renewal request");
        let mut unknown = request();
        unknown["extra"] = Value::Bool(true);
        assert!(validate_request(&unknown).is_err());
        let mut bad_ttl = request();
        bad_ttl["ttl_ms"] = json!(999);
        assert!(validate_request(&bad_ttl).is_err());
        let mut bad_token = request();
        bad_token["fencing_token"] = json!("token\n");
        assert!(validate_request(&bad_token).is_err());
    }
}
