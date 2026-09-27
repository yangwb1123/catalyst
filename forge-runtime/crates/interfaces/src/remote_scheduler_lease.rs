//! Strict client for the accepted EXECUTE scheduler lease claim.
//!
//! This is the first effectful placement boundary: the API may return a
//! fenced lease receipt. The client validates every binding and authority bit
//! before rendering it and never retries the POST automatically.

use std::{
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::RemoteError;

const MAX_INPUT_BYTES: usize = 512 * 1024;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_TOKEN_BYTES: usize = 256;
const MAX_OWNER_PART_BYTES: usize = 512;
const SCHEMA_VERSION: &str = "forge.execution-lease-registry/v1";
const EVALUATION_MODE: &str = "durable_scheduler_lease_claim";
const MIN_TTL_MS: u64 = 1_000;
const MAX_TTL_MS: u64 = 600_000;

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
struct Grant {
    v: u16,
    attempt_id: String,
    target_id: String,
    epoch: u64,
    fencing_token: String,
    issued_at_ms: u64,
    expires_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LeaseResult {
    schema_version: String,
    evaluation_mode: String,
    owner: Owner,
    conversation_id: String,
    run_id: String,
    attempt_id: String,
    device_id: String,
    instance_id: String,
    inventory_revision: u64,
    generation: u64,
    heartbeat_sequence: u64,
    grant: Grant,
    replayed: bool,
    authority: Authority,
}

pub(super) fn read_request(input: &str) -> Result<Value, RemoteError> {
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes).map_err(|_| {
        RemoteError("remote scheduler lease input contains duplicate JSON keys".into())
    })?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| RemoteError("remote scheduler lease input is invalid JSON".into()))?;
    validate_request(&value)?;
    Ok(value)
}

pub(super) fn read_tui_request(input: &str) -> Result<Value, RemoteError> {
    if input == "-" {
        return Err(RemoteError(
            "remote TUI scheduler lease requires a file path; '-' belongs to the standalone CLI"
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
            "requirements",
            "ttl_ms",
        ],
    ) {
        return Err(invalid_request());
    }
    for field in ["conversation_id", "run_id", "attempt_id"] {
        let id = object
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(invalid_request)?;
        if !valid_identifier(id) {
            return Err(invalid_request());
        }
    }
    let ttl = object
        .get("ttl_ms")
        .and_then(Value::as_u64)
        .ok_or_else(invalid_request)?;
    if !(MIN_TTL_MS..=MAX_TTL_MS).contains(&ttl) {
        return Err(invalid_request());
    }
    super::placement_registry::validate_request(&json!({
        "requirements": object.get("requirements").cloned().ok_or_else(invalid_request)?
    }))
    .map_err(|_| invalid_request())
}

pub(super) fn validate_response(value: &Value) -> Result<LeaseResult, RemoteError> {
    let result: LeaseResult = serde_json::from_value(value.clone())
        .map_err(|_| RemoteError("Forge API returned an invalid scheduler lease".into()))?;
    if result.schema_version != SCHEMA_VERSION
        || result.evaluation_mode != EVALUATION_MODE
        || !valid_owner(&result.owner)
        || !valid_identifier(&result.conversation_id)
        || !valid_identifier(&result.run_id)
        || !valid_identifier(&result.attempt_id)
        || !valid_identifier(&result.device_id)
        || !valid_identifier(&result.instance_id)
        || result.inventory_revision == 0
        || result.inventory_revision > MAX_SAFE_INTEGER
        || result.generation == 0
        || result.generation > MAX_SAFE_INTEGER
        || result.heartbeat_sequence == 0
        || result.heartbeat_sequence > MAX_SAFE_INTEGER
        || result.grant.v != 1
        || result.grant.attempt_id != result.attempt_id
        || result.grant.target_id != result.instance_id
        || result.grant.epoch == 0
        || !valid_identifier(&result.grant.fencing_token)
        || result.grant.fencing_token.len() > MAX_TOKEN_BYTES
        || result.grant.issued_at_ms == 0
        || result.grant.expires_at_ms <= result.grant.issued_at_ms
        || result.grant.expires_at_ms - result.grant.issued_at_ms < MIN_TTL_MS
        || result.grant.expires_at_ms - result.grant.issued_at_ms > MAX_TTL_MS
        || !result.authority.placement_selected
        || !result.authority.reservation_created
        || !result.authority.lease_issued
        || result.authority.execution_authorized
        || result.authority.dispatch_performed
        || result.authority.audit_published
    {
        return Err(invalid_response());
    }
    Ok(result)
}

/// Validates a claim receipt and binds its Conversation/Run/Attempt to the
/// exact owner-scoped claim request. The selected target remains server
/// chosen, but the receipt must belong to the requested execution identity.
pub(super) fn validate_response_for_request(
    value: &Value,
    request: &Value,
) -> Result<LeaseResult, RemoteError> {
    validate_request(request)?;
    let result = validate_response(value)?;
    let request_object = request.as_object().ok_or_else(invalid_request)?;
    for (field, actual) in [
        ("conversation_id", result.conversation_id.as_str()),
        ("run_id", result.run_id.as_str()),
        ("attempt_id", result.attempt_id.as_str()),
    ] {
        if request_object.get(field).and_then(Value::as_str) != Some(actual) {
            return Err(RemoteError(
                "Forge API returned a scheduler lease for a different request".into(),
            ));
        }
    }
    Ok(result)
}

pub(super) fn render_human(result: &LeaseResult, writer: &mut impl Write) -> io::Result<()> {
    writeln!(
        writer,
        "scheduler lease [{}] owner={}/{} tenant={} conversation={} run={} attempt={} target={}/{} revision={} generation={} heartbeat={} epoch={} issued_at={} expires_at={} replayed={}",
        result.schema_version,
        result.owner.issuer,
        result.owner.subject,
        result.owner.tenant_id,
        result.conversation_id,
        result.run_id,
        result.attempt_id,
        result.device_id,
        result.instance_id,
        result.inventory_revision,
        result.generation,
        result.heartbeat_sequence,
        result.grant.epoch,
        result.grant.issued_at_ms,
        result.grant.expires_at_ms,
        result.replayed
    )?;
    writeln!(
        writer,
        "authority: placement_selected=true reservation_created=true lease_issued=true execution_authorized=false dispatch_performed=false audit_published=false (fencing token withheld from human output)"
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

fn invalid_request() -> RemoteError {
    RemoteError("remote scheduler lease input is invalid".into())
}

fn invalid_response() -> RemoteError {
    RemoteError("Forge API returned an invalid scheduler lease".into())
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, RemoteError> {
    let mut bytes = Vec::with_capacity(MAX_INPUT_BYTES.min(64 * 1024));
    if input == "-" {
        io::stdin()
            .lock()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("could not read scheduler lease stdin".into()))?;
    } else {
        let path = Path::new(input);
        let metadata = std::fs::metadata(path)
            .map_err(|_| RemoteError("could not read scheduler lease input".into()))?;
        if !metadata.is_file() {
            return Err(RemoteError(
                "scheduler lease input must be a regular file".into(),
            ));
        }
        File::open(path)
            .map_err(|_| RemoteError("could not read scheduler lease input".into()))?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("could not read scheduler lease input".into()))?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(RemoteError(format!(
            "scheduler lease input exceeds {MAX_INPUT_BYTES} bytes"
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
            "conversation_id": "conversation-1", "run_id": "run-1", "attempt_id": "attempt-1",
            "requirements": {
                "os": "linux", "architecture": "amd64", "min_cpu_cores": 1,
                "min_memory_bytes": 1, "min_storage_bytes": 1, "runtime": "oci",
                "gpu": {"required": false, "min_memory_bytes": 0, "runtime": ""},
                "data_residency_zones": ["us-west"], "minimum_trust_zone": "standard",
                "sandbox_floor": "container", "concurrency_slots": 1
            },
            "ttl_ms": 30_000
        })
    }

    fn response() -> Value {
        json!({
            "schema_version": SCHEMA_VERSION, "evaluation_mode": EVALUATION_MODE,
            "owner": {"issuer": "https://id.example", "subject": "user-a", "tenant_id": "tenant-a"},
            "conversation_id": "conversation-1", "run_id": "run-1", "attempt_id": "attempt-1",
            "device_id": "device-a", "instance_id": "runner-a", "inventory_revision": 1,
            "generation": 1, "heartbeat_sequence": 1,
            "grant": {"v": 1, "attempt_id": "attempt-1", "target_id": "runner-a", "epoch": 1,
                "fencing_token": "token-a", "issued_at_ms": 1_800_000_000_000_u64,
                "expires_at_ms": 1_800_000_030_000_u64},
            "replayed": false,
            "authority": {"placement_selected": true, "reservation_created": true, "lease_issued": true,
                "execution_authorized": false, "dispatch_performed": false, "audit_published": false}
        })
    }

    #[test]
    fn validates_strict_request_and_receipt() {
        validate_request(&request()).unwrap();
        validate_response(&response()).unwrap();
        let mut unknown = request();
        unknown["unexpected"] = json!(true);
        assert!(validate_request(&unknown).is_err());
        let mut authority = response();
        authority["authority"]["execution_authorized"] = json!(true);
        assert!(validate_response(&authority).is_err());
    }
}
