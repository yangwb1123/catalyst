//! Strict Runtime consumer for the authenticated Runner execution-boundary
//! preview. This is a metadata-only join. It never opens a Runner transport,
//! sends a payload, or treats readiness as execution authority.

use std::{
    fs::File,
    io::{self, Read, Write},
};

use forge_runtime_domain::execution::runner_command::RunnerCommand;
use serde::Deserialize;
use serde_json::{Map, Value};

use super::RemoteError;

const MAX_INPUT_BYTES: usize = 512 * 1024;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_OWNER_PART_BYTES: usize = 512;
const MAX_TEXT_BYTES: usize = 256;
const MAX_NONCE_BYTES: usize = 128;
const MAX_PAYLOAD_BYTES: usize = 1 << 20;
const SCHEMA_VERSION: &str = "forge.runner-execution-boundary/v1";
const EVALUATION_MODE: &str = "p4_runner_authority_execution_boundary_preview";
const TRANSPORT_SCHEMA_VERSION: &str = "forge.runner-transport-admission/v1";
const TRANSPORT_EVALUATION_MODE: &str = "pure_runner_transport_admission";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Owner {
    issuer: String,
    subject: String,
    tenant_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransportAuthority {
    identity_verified: bool,
    heartbeat_accepted: bool,
    lease_issued: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
    audit_published: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransportObservation {
    schema_version: String,
    evaluation_mode: String,
    method: String,
    path: String,
    timestamp: i64,
    nonce: String,
    payload_sha256: String,
    payload_bytes: u64,
    replay_checked: bool,
    preview_only: bool,
    authority: TransportAuthority,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Controls {
    effect_state: String,
    cancellation_requested: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BoundaryAuthority {
    device_identity_verified: bool,
    command_persisted: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
    audit_published: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BoundaryResult {
    schema_version: String,
    evaluation_mode: String,
    mode: String,
    owner: Owner,
    conversation_id: String,
    run_id: String,
    attempt_id: String,
    attempt_state: String,
    command_id: String,
    command_sha256: String,
    target_id: String,
    lease_epoch: u64,
    activation_allowed: bool,
    runner_authority_accepted: bool,
    dispatch_admission_ready: bool,
    transport_admission_ready: bool,
    effect_state: String,
    effect_state_startable: bool,
    cancellation_clear: bool,
    execution_boundary_ready: bool,
    rejection_reasons: Vec<String>,
    preview_only: bool,
    authority: BoundaryAuthority,
}

pub(super) fn read_request(input: &str) -> Result<Value, RemoteError> {
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes).map_err(|_| {
        RemoteError("remote Runner execution boundary input contains duplicate JSON keys".into())
    })?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| {
        RemoteError("remote Runner execution boundary input is invalid JSON".into())
    })?;
    validate_request(&value)?;
    Ok(value)
}

pub(super) fn read_tui_request(input: &str) -> Result<Value, RemoteError> {
    if input == "-" {
        return Err(RemoteError(
            "remote TUI Runner execution boundary requires a file path; '-' belongs to the standalone CLI".into(),
        ));
    }
    read_request(input)
}

pub(super) fn conversation_and_run(value: &Value) -> Result<(String, String), RemoteError> {
    let object = value.as_object().ok_or_else(invalid_request)?;
    let conversation_id = object
        .get("conversation_id")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    let run_id = object
        .get("run_id")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    Ok((conversation_id.to_owned(), run_id.to_owned()))
}

pub(super) fn target_id(value: &Value) -> Result<&str, RemoteError> {
    value
        .get("command")
        .and_then(Value::as_object)
        .and_then(|command| command.get("lease_proof"))
        .and_then(Value::as_object)
        .and_then(|proof| proof.get("target_id"))
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)
}

pub(super) fn validate_request(value: &Value) -> Result<(), RemoteError> {
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

pub(super) fn validate_response(
    value: &Value,
    request: &Value,
    conversation_id: &str,
    run_id: &str,
) -> Result<(), RemoteError> {
    validate_request(request)?;
    let result: BoundaryResult =
        serde_json::from_value(value.clone()).map_err(|_| invalid_response())?;
    let request_object = request.as_object().ok_or_else(invalid_request)?;
    let owner: Owner = serde_json::from_value(
        request_object
            .get("owner")
            .cloned()
            .ok_or_else(invalid_request)?,
    )
    .map_err(|_| invalid_request())?;
    let command: RunnerCommand = serde_json::from_value(
        request_object
            .get("command")
            .cloned()
            .ok_or_else(invalid_request)?,
    )
    .map_err(|_| invalid_request())?;
    let expected_command_sha = command.command_sha256().map_err(|_| invalid_request())?;
    let attempt_id = request_object
        .get("attempt_id")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    let attempt_state = request_object
        .get("attempt_state")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    let controls: Controls = serde_json::from_value(
        request_object
            .get("controls")
            .cloned()
            .ok_or_else(invalid_request)?,
    )
    .map_err(|_| invalid_request())?;
    let transport: TransportObservation = serde_json::from_value(
        request_object
            .get("transport")
            .cloned()
            .ok_or_else(invalid_request)?,
    )
    .map_err(|_| invalid_request())?;
    let target_id = command.lease_proof.target_id.as_str();
    let expected_payload = request_object
        .get("expected_payload_sha256")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    let transport_binding = transport.method == "POST"
        && transport.path == format!("/api/v1/runners/{target_id}/dispatch")
        && transport.payload_sha256 == expected_payload;
    let reasons = result.rejection_reasons.as_slice();
    let ready = result.mode == "execute"
        && result.activation_allowed
        && result.runner_authority_accepted
        && result.dispatch_admission_ready
        && result.transport_admission_ready
        && result.effect_state_startable
        && result.cancellation_clear;
    if result.schema_version != SCHEMA_VERSION
        || result.evaluation_mode != EVALUATION_MODE
        || result.owner.issuer != owner.issuer
        || result.owner.subject != owner.subject
        || result.owner.tenant_id != owner.tenant_id
        || result.conversation_id != conversation_id
        || result.run_id != run_id
        || result.attempt_id != attempt_id
        || result.attempt_state != attempt_state
        || result.command_id != command.command_id
        || result.command_sha256 != expected_command_sha
        || !valid_digest(&result.command_sha256)
        || result.target_id != target_id
        || result.lease_epoch == 0
        || !valid_effect_state(&result.effect_state)
        || result.effect_state != controls.effect_state
        || result.effect_state_startable != effect_state_startable(&result.effect_state)
        || result.cancellation_clear == controls.cancellation_requested
        || result.transport_admission_ready != transport_binding
        || transport.schema_version != TRANSPORT_SCHEMA_VERSION
        || transport.evaluation_mode != TRANSPORT_EVALUATION_MODE
        || transport.timestamp <= 0
        || transport.nonce.is_empty()
        || transport.nonce.len() > MAX_NONCE_BYTES
        || transport.payload_bytes == 0
        || transport.payload_bytes > MAX_PAYLOAD_BYTES as u64
        || !transport.replay_checked
        || !transport.preview_only
        || !transport_authority_clear(&transport.authority)
        || !valid_mode(&result.mode)
        || result.execution_boundary_ready != ready
        || !sorted_unique_identifiers(reasons)
        || (ready && !reasons.is_empty())
        || (!ready && reasons.is_empty())
        || (!result.dispatch_admission_ready
            && !reasons.iter().any(|v| v == "dispatch_admission_not_ready"))
        || (!result.transport_admission_ready
            && !reasons.iter().any(|v| v == "transport_admission_not_ready"))
        || (!result.cancellation_clear && !reasons.iter().any(|v| v == "cancellation_requested"))
        || (!result.effect_state_startable
            && !reasons.iter().any(|v| {
                v == "effect_state_not_startable" || v == "uncertain_effect_requires_reconciliation"
            }))
        || !result.preview_only
        || !boundary_authority_clear(&result.authority)
    {
        return Err(invalid_response());
    }
    Ok(())
}

pub(super) fn render_human(value: &Value, writer: &mut impl Write) -> io::Result<()> {
    let result: BoundaryResult = serde_json::from_value(value.clone()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid Runner execution boundary response",
        )
    })?;
    writeln!(
        writer,
        "Runner execution boundary [{}] mode={} owner={}/{} tenant={} conversation={} run={} attempt={} command={} target={} epoch={} effect_state={} ready={}",
        result.schema_version,
        result.mode,
        result.owner.issuer,
        result.owner.subject,
        result.owner.tenant_id,
        result.conversation_id,
        result.run_id,
        result.attempt_id,
        result.command_id,
        result.target_id,
        result.lease_epoch,
        result.effect_state,
        result.execution_boundary_ready
    )?;
    writeln!(
        writer,
        "rejection_reasons={:?}; preview_only=true execution_authorized=false dispatch_performed=false audit_published=false (fencing token, argv, workspace, transport payload, and Runner output withheld)",
        result.rejection_reasons
    )
}

fn validate_controls(value: &Value) -> Result<(), RemoteError> {
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

fn validate_transport(value: &Value) -> Result<(), RemoteError> {
    let object = value.as_object().ok_or_else(invalid_request)?;
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
        || timestamp as u64 > MAX_SAFE_INTEGER
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

fn validate_owner(value: &Value) -> Result<(), RemoteError> {
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

fn valid_mode(value: &str) -> bool {
    matches!(
        value,
        "off" | "inventory" | "observe" | "execute" | "migrate" | "federate"
    )
}

fn valid_effect_state(value: &str) -> bool {
    matches!(
        value,
        "not_started" | "started" | "completed" | "failed" | "uncertain" | "reconciled"
    )
}

fn effect_state_startable(value: &str) -> bool {
    matches!(value, "not_started" | "reconciled")
}

fn valid_attempt_state(value: &str) -> bool {
    matches!(
        value,
        "requested"
            | "accepted"
            | "starting"
            | "running"
            | "interrupted"
            | "completed"
            | "failed"
            | "uncertain"
    )
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_TEXT_BYTES
        && value.chars().enumerate().all(|(index, c)| {
            c.is_ascii_alphanumeric()
                || (index > 0 && matches!(c, '.' | '_' | ':' | '+' | '/' | '-'))
        })
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

fn exact_fields<const N: usize>(object: &Map<String, Value>, fields: [&str; N]) -> bool {
    object.len() == N && fields.into_iter().all(|field| object.contains_key(field))
}

fn transport_authority_clear(authority: &TransportAuthority) -> bool {
    !authority.identity_verified
        && !authority.heartbeat_accepted
        && !authority.lease_issued
        && !authority.reservation_created
        && !authority.execution_authorized
        && !authority.dispatch_performed
        && !authority.audit_published
}

fn boundary_authority_clear(authority: &BoundaryAuthority) -> bool {
    !authority.device_identity_verified
        && !authority.command_persisted
        && !authority.reservation_created
        && !authority.execution_authorized
        && !authority.dispatch_performed
        && !authority.audit_published
}

fn sorted_unique_identifiers(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
        && values.iter().all(|value| valid_identifier(value))
}

fn invalid_request() -> RemoteError {
    RemoteError("remote Runner execution boundary input is invalid".into())
}

fn invalid_response() -> RemoteError {
    RemoteError("Forge API returned an invalid Runner execution boundary".into())
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, RemoteError> {
    let mut bytes = Vec::with_capacity(MAX_INPUT_BYTES.min(64 * 1024));
    if input == "-" {
        io::stdin()
            .lock()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("failed to read Runner execution boundary input".into()))?;
    } else {
        File::open(input)
            .map_err(|_| RemoteError("failed to open Runner execution boundary input".into()))?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("failed to read Runner execution boundary input".into()))?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(RemoteError(
            "Runner execution boundary input exceeds size limit".into(),
        ));
    }
    Ok(bytes)
}
