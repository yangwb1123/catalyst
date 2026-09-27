//! Strict client for the accepted EXECUTE scheduler-lease release.
//!
//! Release marks a durable fenced reservation inactive. The proof is sent
//! over the authenticated POST, while the response deliberately contains no
//! fencing token and grants no execution or dispatch authority.

use std::{
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use serde::Deserialize;
use serde_json::{Map, Value};

use super::RemoteError;

const MAX_INPUT_BYTES: usize = 512 * 1024;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_TOKEN_BYTES: usize = 256;
const MAX_OWNER_PART_BYTES: usize = 512;
const SCHEMA_VERSION: &str = "forge.execution-lease-registry/v1";
const EVALUATION_MODE: &str = "durable_scheduler_lease_release";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Authority {
    placement_selected: bool,
    reservation_created: bool,
    lease_issued: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
    audit_published: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Owner {
    issuer: String,
    subject: String,
    tenant_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseResult {
    schema_version: String,
    evaluation_mode: String,
    owner: Owner,
    conversation_id: String,
    run_id: String,
    attempt_id: String,
    device_id: String,
    instance_id: String,
    epoch: u64,
    released_at_ms: u64,
    replayed: bool,
    authority: Authority,
}

pub(super) fn read_request(input: &str) -> Result<Value, RemoteError> {
    if input == "-" {
        return Err(RemoteError(
            "scheduler lease release input must be a regular file for this command".into(),
        ));
    }
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes).map_err(|_| {
        RemoteError("remote scheduler lease release input contains duplicate JSON keys".into())
    })?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| RemoteError("remote scheduler lease release input is invalid JSON".into()))?;
    validate_request(&value)?;
    Ok(value)
}

pub(super) fn read_tui_request(input: &str) -> Result<Value, RemoteError> {
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
    Ok(())
}

pub(super) fn validate_response(value: &Value) -> Result<(), RemoteError> {
    let result: ReleaseResult = serde_json::from_value(value.clone())
        .map_err(|_| RemoteError("Forge API returned an invalid scheduler lease release".into()))?;
    if result.schema_version != SCHEMA_VERSION
        || result.evaluation_mode != EVALUATION_MODE
        || !valid_owner(&result.owner)
        || !valid_identifier(&result.conversation_id)
        || !valid_identifier(&result.run_id)
        || !valid_identifier(&result.attempt_id)
        || !valid_identifier(&result.device_id)
        || !valid_identifier(&result.instance_id)
        || result.epoch == 0
        || result.epoch > MAX_SAFE_INTEGER
        || result.released_at_ms == 0
        || result.released_at_ms > MAX_SAFE_INTEGER
        || result.authority.placement_selected
        || result.authority.reservation_created
        || result.authority.lease_issued
        || result.authority.execution_authorized
        || result.authority.dispatch_performed
        || result.authority.audit_published
    {
        return Err(invalid_response());
    }
    Ok(())
}

/// Validates the release receipt against the exact fenced proof supplied by
/// the caller. Release is cleanup authority, so a response for another
/// Conversation, Run, Attempt, target, or epoch must fail closed.
pub(super) fn validate_response_for_request(
    value: &Value,
    request: &Value,
) -> Result<(), RemoteError> {
    validate_request(request)?;
    validate_response(value)?;
    let request_object = request.as_object().ok_or_else(invalid_request)?;
    for field in ["conversation_id", "run_id", "attempt_id"] {
        if request_object.get(field) != value.get(field) {
            return Err(RemoteError(
                "Forge API returned a scheduler lease release for a different request".into(),
            ));
        }
    }
    let target_id = request_object
        .get("target_id")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    let epoch = request_object
        .get("epoch")
        .and_then(Value::as_u64)
        .ok_or_else(invalid_request)?;
    if value.get("instance_id").and_then(Value::as_str) != Some(target_id)
        || value.get("epoch").and_then(Value::as_u64) != Some(epoch)
    {
        return Err(RemoteError(
            "Forge API returned a scheduler lease release for a different proof".into(),
        ));
    }
    Ok(())
}

pub(super) fn render_human(value: &Value, writer: &mut impl Write) -> io::Result<()> {
    validate_response(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.0))?;
    let result: ReleaseResult = serde_json::from_value(value.clone())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid release response"))?;
    writeln!(
        writer,
        "scheduler lease release [{}] owner={}/{} tenant={} conversation={} run={} attempt={} target={}/{} epoch={} released_at={} replayed={}",
        result.schema_version,
        result.owner.issuer,
        result.owner.subject,
        result.owner.tenant_id,
        result.conversation_id,
        result.run_id,
        result.attempt_id,
        result.device_id,
        result.instance_id,
        result.epoch,
        result.released_at_ms,
        result.replayed,
    )?;
    writeln!(
        writer,
        "authority: placement_selected=false reservation_created=false lease_issued=false execution_authorized=false dispatch_performed=false audit_published=false (fencing token withheld from human output)"
    )
}

fn exact_fields<const N: usize>(object: &Map<String, Value>, fields: [&str; N]) -> bool {
    object.len() == N && fields.into_iter().all(|field| object.contains_key(field))
}

fn valid_owner(value: &Owner) -> bool {
    [&value.issuer, &value.subject, &value.tenant_id]
        .into_iter()
        .all(|part| {
            !part.is_empty()
                && part.len() <= MAX_OWNER_PART_BYTES
                && part.trim() == part
                && !part.chars().any(char::is_control)
        })
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
    RemoteError("remote scheduler lease release input is invalid".into())
}

fn invalid_response() -> RemoteError {
    RemoteError("Forge API returned an invalid scheduler lease release".into())
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, RemoteError> {
    let path = Path::new(input);
    let metadata = std::fs::metadata(path)
        .map_err(|_| RemoteError("could not read scheduler lease release input".into()))?;
    if !metadata.is_file() {
        return Err(RemoteError(
            "scheduler lease release input must be a regular file".into(),
        ));
    }
    let mut bytes = Vec::with_capacity(MAX_INPUT_BYTES.min(64 * 1024));
    File::open(path)
        .map_err(|_| RemoteError("could not read scheduler lease release input".into()))?
        .take((MAX_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| RemoteError("could not read scheduler lease release input".into()))?;
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(RemoteError(format!(
            "scheduler lease release input exceeds {MAX_INPUT_BYTES} bytes"
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
        })
    }

    fn response() -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "evaluation_mode": EVALUATION_MODE,
            "owner": {"issuer":"https://id.example","subject":"user-a","tenant_id":"tenant-a"},
            "conversation_id": "conversation-1", "run_id": "run-1", "attempt_id": "attempt-1",
            "device_id": "device-a", "instance_id": "runner-a", "epoch": 1,
            "released_at_ms": 1800000000100_i64, "replayed": false,
            "authority": {
                "placement_selected": false, "reservation_created": false, "lease_issued": false,
                "execution_authorized": false, "dispatch_performed": false, "audit_published": false
            }
        })
    }

    #[test]
    fn release_request_and_response_are_strict() {
        validate_request(&request()).expect("valid release request");
        let mut unknown = request();
        unknown["extra"] = Value::Bool(true);
        assert!(validate_request(&unknown).is_err());
        validate_response(&response()).expect("valid release response");
        let mut authority = response();
        authority["authority"]["lease_issued"] = Value::Bool(true);
        assert!(validate_response(&authority).is_err());
        let mut response_with_unknown = response();
        response_with_unknown["extra"] = json!(true);
        assert!(validate_response(&response_with_unknown).is_err());
    }
}
