//! Strict client for the planning-only scheduler-selection preview.
//!
//! The response is a deterministic target declaration, not a lease or an
//! execution decision. This module validates the owner-bound identifiers and
//! keeps every authority bit closed before the CLI or TUI renders it.

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
const MAX_CANDIDATES: usize = 128;
const MAX_TOKEN_BYTES: usize = 128;
const MAX_OWNER_PART_BYTES: usize = 512;
const SCHEMA_VERSION: &str = "forge.scheduler-selection-preview/v1";
const EVALUATION_MODE: &str = "pure_scheduler_selection_preview";

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
pub(super) struct PreviewResult {
    schema_version: String,
    evaluation_mode: String,
    owner: Owner,
    conversation_id: String,
    run_id: String,
    attempt_id: String,
    evaluated_at_ms: u64,
    candidate_count: usize,
    eligible_candidate_count: usize,
    selection_available: bool,
    selection_reason: String,
    selected_device_id: Option<String>,
    selected_instance_id: Option<String>,
    preview_only: bool,
    authority: Authority,
}

pub(super) fn read_request(input: &str) -> Result<Value, RemoteError> {
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes).map_err(|_| {
        RemoteError("remote scheduler selection input contains duplicate JSON keys".into())
    })?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| RemoteError("remote scheduler selection input is invalid JSON".into()))?;
    validate_request(&value)?;
    Ok(value)
}

pub(super) fn read_tui_request(input: &str) -> Result<Value, RemoteError> {
    if input == "-" {
        return Err(RemoteError(
            "remote TUI scheduler selection preview requires a file path; '-' belongs to the standalone CLI"
                .into(),
        ));
    }
    read_request(input)
}

pub(super) fn validate_request(value: &Value) -> Result<(), RemoteError> {
    let object = value.as_object().ok_or_else(invalid_request)?;
    if !exact_fields(
        object,
        ["conversation_id", "run_id", "attempt_id", "requirements"],
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
    // Reuse the exact requirements grammar already used by the registry
    // placement candidate, while keeping the scheduler request's own fields
    // strict and owner/run bound.
    super::placement_registry::validate_request(&json!({
        "requirements": object.get("requirements").cloned().ok_or_else(invalid_request)?
    }))
    .map_err(|_| invalid_request())
}

pub(super) fn validate_response(value: &Value) -> Result<PreviewResult, RemoteError> {
    let result: PreviewResult = serde_json::from_value(value.clone()).map_err(|_| {
        RemoteError("Forge API returned an invalid scheduler selection preview".into())
    })?;
    if result.schema_version != SCHEMA_VERSION
        || result.evaluation_mode != EVALUATION_MODE
        || !valid_owner(&result.owner)
        || !valid_identifier(&result.conversation_id)
        || !valid_identifier(&result.run_id)
        || !valid_identifier(&result.attempt_id)
        || result.evaluated_at_ms == 0
        || result.evaluated_at_ms > MAX_SAFE_INTEGER
        || result.candidate_count > MAX_CANDIDATES
        || result.eligible_candidate_count > result.candidate_count
        || !result.preview_only
        || !all_false(&result.authority)
    {
        return Err(invalid_response());
    }
    match (
        result.selection_available,
        result.selected_device_id.as_deref(),
        result.selected_instance_id.as_deref(),
    ) {
        (true, Some(device), Some(instance))
            if result.eligible_candidate_count > 0
                && result.selection_reason == "first_sorted_eligible_candidate"
                && valid_identifier(device)
                && valid_identifier(instance) => {}
        (false, None, None) if result.selection_reason == "no_eligible_candidate" => {}
        _ => return Err(invalid_response()),
    }
    Ok(result)
}

/// Validates the response and binds its durable session identifiers to the
/// exact request that produced it. A valid preview for another Run must not
/// be rendered as the selected session's candidate.
pub(super) fn validate_response_for_request(
    value: &Value,
    request: &Value,
) -> Result<PreviewResult, RemoteError> {
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
                "Forge API returned a scheduler selection preview for a different request".into(),
            ));
        }
    }
    Ok(result)
}

pub(super) fn render_human(result: &PreviewResult, writer: &mut impl Write) -> io::Result<()> {
    let selected = match (&result.selected_device_id, &result.selected_instance_id) {
        (Some(device), Some(instance)) => format!("{device}/{instance}"),
        _ => "none".into(),
    };
    writeln!(
        writer,
        "scheduler selection preview [{}] owner={}/{} tenant={} conversation={} run={} attempt={} at {} candidates={} eligible={} selected={} reason={}",
        result.schema_version,
        result.owner.issuer,
        result.owner.subject,
        result.owner.tenant_id,
        result.conversation_id,
        result.run_id,
        result.attempt_id,
        result.evaluated_at_ms,
        result.candidate_count,
        result.eligible_candidate_count,
        selected,
        result.selection_reason
    )?;
    writeln!(
        writer,
        "preview_only=true authority: placement_selected=false reservation_created=false lease_issued=false execution_authorized=false dispatch_performed=false audit_published=false"
    )
}

fn exact_fields<const N: usize>(object: &Map<String, Value>, fields: [&str; N]) -> bool {
    object.len() == N && fields.into_iter().all(|field| object.contains_key(field))
}

fn all_false(value: &Authority) -> bool {
    !value.placement_selected
        && !value.reservation_created
        && !value.lease_issued
        && !value.execution_authorized
        && !value.dispatch_performed
        && !value.audit_published
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
    RemoteError("remote scheduler selection input is invalid".into())
}

fn invalid_response() -> RemoteError {
    RemoteError("Forge API returned an invalid scheduler selection preview".into())
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, RemoteError> {
    let mut bytes = Vec::with_capacity(MAX_INPUT_BYTES.min(64 * 1024));
    if input == "-" {
        io::stdin()
            .lock()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("could not read scheduler selection stdin".into()))?;
    } else {
        let path = Path::new(input);
        let metadata = std::fs::metadata(path)
            .map_err(|_| RemoteError("could not read scheduler selection input".into()))?;
        if !metadata.is_file() {
            return Err(RemoteError(
                "scheduler selection input must be a regular file".into(),
            ));
        }
        File::open(path)
            .map_err(|_| RemoteError("could not read scheduler selection input".into()))?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("could not read scheduler selection input".into()))?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(RemoteError(format!(
            "scheduler selection input exceeds {MAX_INPUT_BYTES} bytes"
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
            "requirements": {
                "os": "linux", "architecture": "amd64", "min_cpu_cores": 1,
                "min_memory_bytes": 1, "min_storage_bytes": 1, "runtime": "oci",
                "gpu": {"required": false, "min_memory_bytes": 0, "runtime": ""},
                "data_residency_zones": ["us-west"], "minimum_trust_zone": "standard",
                "sandbox_floor": "container", "concurrency_slots": 1
            }
        })
    }

    fn response(available: bool) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "evaluation_mode": EVALUATION_MODE,
            "owner": {"issuer": "https://id.example", "subject": "user-a", "tenant_id": "tenant-a"},
            "conversation_id": "conversation-1", "run_id": "run-1", "attempt_id": "attempt-1",
            "evaluated_at_ms": 1_800_000_000_000_i64,
            "candidate_count": 1, "eligible_candidate_count": if available {1} else {0},
            "selection_available": available,
            "selection_reason": if available {"first_sorted_eligible_candidate"} else {"no_eligible_candidate"},
            "selected_device_id": if available {json!("device-a")} else {Value::Null},
            "selected_instance_id": if available {json!("runner-a")} else {Value::Null},
            "preview_only": true,
            "authority": {"placement_selected": false, "reservation_created": false, "lease_issued": false, "execution_authorized": false, "dispatch_performed": false, "audit_published": false}
        })
    }

    #[test]
    fn validates_exact_request_and_response_modes() {
        validate_request(&request()).unwrap();
        validate_response(&response(false)).unwrap();
        validate_response(&response(true)).unwrap();

        let mut unknown = request();
        unknown["unexpected"] = json!(true);
        assert!(validate_request(&unknown).is_err());
        let mut authority = response(false);
        authority["authority"]["lease_issued"] = json!(true);
        assert!(validate_response(&authority).is_err());
    }

    #[test]
    fn rejects_selected_ids_without_available_result() {
        let mut value = response(false);
        value["selected_device_id"] = json!("device-a");
        assert!(validate_response(&value).is_err());
    }
}
