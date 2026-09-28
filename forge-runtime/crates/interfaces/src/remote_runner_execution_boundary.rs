//! Strict Runtime consumer for the authenticated Runner execution-boundary
//! preview. This is a metadata-only join. It never opens a Runner transport,
//! sends a payload, or treats readiness as execution authority.

#[path = "remote_runner_execution_boundary/request.rs"]
mod request;
pub(super) use request::validate_request;

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
#[allow(
    clippy::struct_excessive_bools,
    reason = "Published wire predicates are separate booleans; changing their representation would change the protocol."
)]
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
#[allow(
    clippy::struct_excessive_bools,
    reason = "Published wire predicates are separate booleans; changing their representation would change the protocol."
)]
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
#[allow(
    clippy::struct_excessive_bools,
    reason = "Published wire predicates are separate booleans; changing their representation would change the protocol."
)]
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

pub(super) fn validate_response(
    value: &Value,
    request: &Value,
    conversation_id: &str,
    run_id: &str,
) -> Result<(), RemoteError> {
    validate_request(request)?;
    let result: BoundaryResult =
        serde_json::from_value(value.clone()).map_err(|_| invalid_response())?;
    let context = response_request(request)?;
    validate_response_binding(&result, &context, conversation_id, run_id)?;
    validate_response_state(&result, &context)
}

struct ResponseRequest<'a> {
    owner: Owner,
    command: RunnerCommand,
    expected_command_sha: String,
    attempt_id: &'a str,
    attempt_state: &'a str,
    controls: Controls,
    transport: TransportObservation,
    expected_payload: &'a str,
}

fn response_request(request: &Value) -> Result<ResponseRequest<'_>, RemoteError> {
    let request_object = request.as_object().ok_or_else(invalid_request)?;
    let owner: Owner = request_field(request_object, "owner")?;
    let command: RunnerCommand = request_field(request_object, "command")?;
    let expected_command_sha = command.command_sha256().map_err(|_| invalid_request())?;
    let attempt_id = request_object
        .get("attempt_id")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    let attempt_state = request_object
        .get("attempt_state")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    let controls: Controls = request_field(request_object, "controls")?;
    let transport: TransportObservation = request_field(request_object, "transport")?;
    let expected_payload = request_object
        .get("expected_payload_sha256")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    Ok(ResponseRequest {
        owner,
        command,
        expected_command_sha,
        attempt_id,
        attempt_state,
        controls,
        transport,
        expected_payload,
    })
}

fn validate_response_binding(
    result: &BoundaryResult,
    context: &ResponseRequest<'_>,
    conversation_id: &str,
    run_id: &str,
) -> Result<(), RemoteError> {
    let target_id = context.command.lease_proof.target_id.as_str();
    if result.schema_version != SCHEMA_VERSION
        || result.evaluation_mode != EVALUATION_MODE
        || result.owner.issuer != context.owner.issuer
        || result.owner.subject != context.owner.subject
        || result.owner.tenant_id != context.owner.tenant_id
        || result.conversation_id != conversation_id
        || result.run_id != run_id
        || result.attempt_id != context.attempt_id
        || result.attempt_state != context.attempt_state
        || result.command_id != context.command.command_id
        || result.command_sha256 != context.expected_command_sha
        || !valid_digest(&result.command_sha256)
        || result.target_id != target_id
        || result.lease_epoch == 0
    {
        return Err(invalid_response());
    }
    Ok(())
}

fn validate_response_state(
    result: &BoundaryResult,
    context: &ResponseRequest<'_>,
) -> Result<(), RemoteError> {
    let target_id = context.command.lease_proof.target_id.as_str();
    let transport_binding = context.transport.method == "POST"
        && context.transport.path == format!("/api/v1/runners/{target_id}/dispatch")
        && context.transport.payload_sha256 == context.expected_payload;
    if !valid_effect_state(&result.effect_state)
        || result.effect_state != context.controls.effect_state
        || result.effect_state_startable != effect_state_startable(&result.effect_state)
        || result.cancellation_clear == context.controls.cancellation_requested
        || result.transport_admission_ready != transport_binding
        || context.transport.schema_version != TRANSPORT_SCHEMA_VERSION
        || context.transport.evaluation_mode != TRANSPORT_EVALUATION_MODE
        || context.transport.timestamp <= 0
        || context.transport.nonce.is_empty()
        || context.transport.nonce.len() > MAX_NONCE_BYTES
        || context.transport.payload_bytes == 0
        || context.transport.payload_bytes > MAX_PAYLOAD_BYTES as u64
        || !context.transport.replay_checked
        || !context.transport.preview_only
        || !transport_authority_clear(&context.transport.authority)
        || response_readiness_invalid(result)
    {
        return Err(invalid_response());
    }
    Ok(())
}

fn request_field<T: serde::de::DeserializeOwned>(
    request: &Map<String, Value>,
    key: &str,
) -> Result<T, RemoteError> {
    serde_json::from_value(request.get(key).cloned().ok_or_else(invalid_request)?)
        .map_err(|_| invalid_request())
}

fn response_readiness_invalid(result: &BoundaryResult) -> bool {
    let reasons = result.rejection_reasons.as_slice();
    let ready = result.mode == "execute"
        && result.activation_allowed
        && result.runner_authority_accepted
        && result.dispatch_admission_ready
        && result.transport_admission_ready
        && result.effect_state_startable
        && result.cancellation_clear;
    !valid_mode(&result.mode)
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
}
