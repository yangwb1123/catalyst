//! Strict Runtime consumer for the authenticated Runner transport admission
//! preview. The request carries a separately verified transport observation;
//! this module never opens a Runner connection or sends its payload.

#[path = "remote_runner_transport_admission/request.rs"]
mod request;
pub(super) use request::validate_request;

use std::{
    fs::File,
    io::{self, Read, Write},
    path::Path,
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
const SCHEMA_VERSION: &str = "forge.runner-transport-admission/v1";
const EVALUATION_MODE: &str = "fenced_runner_transport_admission_preview";
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
#[allow(
    clippy::struct_excessive_bools,
    reason = "Published wire predicates are separate booleans; changing their representation would change the protocol."
)]
struct AdmissionAuthority {
    device_identity_verified: bool,
    transport_authenticated: bool,
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
struct AdmissionResult {
    schema_version: String,
    evaluation_mode: String,
    owner: Owner,
    conversation_id: String,
    run_id: String,
    attempt_id: String,
    attempt_state: String,
    attempt_state_admissible: bool,
    command_id: String,
    command_sha256: String,
    target_id: String,
    lease_epoch: u64,
    lease_issued_at_ms: u64,
    lease_expires_at_ms: u64,
    evaluated_at_ms: u64,
    transport_method: String,
    transport_path: String,
    transport_timestamp: i64,
    transport_nonce: String,
    transport_payload_sha256: String,
    transport_payload_bytes: u64,
    transport_replay_checked: bool,
    lease_proof_current: bool,
    lease_active: bool,
    command_binding_valid: bool,
    transport_binding_valid: bool,
    admission_ready: bool,
    rejection_reasons: Vec<String>,
    preview_only: bool,
    authority: AdmissionAuthority,
}

pub(super) fn read_request(input: &str) -> Result<Value, RemoteError> {
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes).map_err(|_| {
        RemoteError("remote Runner transport admission input contains duplicate JSON keys".into())
    })?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| {
        RemoteError("remote Runner transport admission input is invalid JSON".into())
    })?;
    validate_request(&value)?;
    Ok(value)
}

pub(super) fn read_tui_request(input: &str) -> Result<Value, RemoteError> {
    if input == "-" {
        return Err(RemoteError(
            "remote TUI Runner transport admission requires a file path; '-' belongs to the standalone CLI".into(),
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
    let result: AdmissionResult = serde_json::from_value(value.clone()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid Runner transport admission response",
        )
    })?;
    writeln!(
        writer,
        "Runner transport admission [{}] owner={}/{} tenant={} conversation={} run={} attempt={} command={} target={} epoch={} transport={} {} ready={} evaluated_at={}",
        result.schema_version,
        result.owner.issuer,
        result.owner.subject,
        result.owner.tenant_id,
        result.conversation_id,
        result.run_id,
        result.attempt_id,
        result.command_id,
        result.target_id,
        result.lease_epoch,
        result.transport_method,
        result.transport_path,
        result.admission_ready,
        result.evaluated_at_ms
    )?;
    writeln!(
        writer,
        "rejection_reasons={:?}; execution_authorized=false dispatch_performed=false audit_published=false (transport payload, fencing token, argv, workspace, and output withheld)",
        result.rejection_reasons
    )
}

fn dispatchable_attempt_state(value: &str) -> bool {
    matches!(value, "accepted" | "starting" | "running")
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

fn exact_fields<const N: usize>(object: &Map<String, Value>, fields: [&str; N]) -> bool {
    object.len() == N && fields.into_iter().all(|field| object.contains_key(field))
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

fn invalid_request() -> RemoteError {
    RemoteError("remote Runner transport admission input is invalid".into())
}

fn invalid_response() -> RemoteError {
    RemoteError("Forge API returned an invalid Runner transport admission".into())
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, RemoteError> {
    let mut bytes = Vec::with_capacity(MAX_INPUT_BYTES.min(64 * 1024));
    if input == "-" {
        io::stdin()
            .lock()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("could not read Runner transport admission stdin".into()))?;
    } else {
        let path = Path::new(input);
        let metadata = std::fs::metadata(path)
            .map_err(|_| RemoteError("could not read Runner transport admission input".into()))?;
        if !metadata.is_file() {
            return Err(RemoteError(
                "Runner transport admission input must be a regular file".into(),
            ));
        }
        File::open(path)
            .map_err(|_| RemoteError("could not read Runner transport admission input".into()))?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("could not read Runner transport admission input".into()))?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(RemoteError(format!(
            "Runner transport admission input exceeds {MAX_INPUT_BYTES} bytes"
        )));
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
    let result: AdmissionResult =
        serde_json::from_value(value.clone()).map_err(|_| invalid_response())?;
    let context = response_request(request)?;
    validate_response_binding(&result, &context, conversation_id, run_id)?;
    validate_response_state(&result, &context)
}

struct ResponseRequest<'a> {
    owner: Owner,
    command: RunnerCommand,
    expected_command_sha: String,
    attempt_state: &'a str,
    attempt_id: &'a str,
    expected_payload: &'a str,
    transport: TransportObservation,
}

fn response_request(request: &Value) -> Result<ResponseRequest<'_>, RemoteError> {
    let request_object = request.as_object().ok_or_else(invalid_request)?;
    let owner: Owner = request_field(request_object, "owner")?;
    let command: RunnerCommand = request_field(request_object, "command")?;
    let expected_command_sha = command.command_sha256().map_err(|_| invalid_request())?;
    let attempt_state = request_object
        .get("attempt_state")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    let attempt_id = request_object
        .get("attempt_id")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    let expected_payload = request_object
        .get("expected_payload_sha256")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    let transport: TransportObservation = request_field(request_object, "transport")?;
    Ok(ResponseRequest {
        owner,
        command,
        expected_command_sha,
        attempt_state,
        attempt_id,
        expected_payload,
        transport,
    })
}

fn validate_response_binding(
    result: &AdmissionResult,
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
        || result.lease_epoch != context.command.lease_proof.epoch
        || result.lease_issued_at_ms == 0
        || result.lease_expires_at_ms <= result.lease_issued_at_ms
        || result.lease_expires_at_ms > MAX_SAFE_INTEGER
        || result.evaluated_at_ms == 0
        || result.evaluated_at_ms > MAX_SAFE_INTEGER
    {
        return Err(invalid_response());
    }
    Ok(())
}

fn validate_response_state(
    result: &AdmissionResult,
    context: &ResponseRequest<'_>,
) -> Result<(), RemoteError> {
    let target_id = context.command.lease_proof.target_id.as_str();
    let transport_binding = context.transport.method == "POST"
        && context.transport.path == format!("/api/v1/runners/{target_id}/dispatch")
        && context.transport.payload_sha256 == context.expected_payload;
    let transport_authority_clear = !context.transport.authority.identity_verified
        && !context.transport.authority.heartbeat_accepted
        && !context.transport.authority.lease_issued
        && !context.transport.authority.reservation_created
        && !context.transport.authority.execution_authorized
        && !context.transport.authority.dispatch_performed
        && !context.transport.authority.audit_published;
    if result.transport_method != context.transport.method
        || result.transport_path != context.transport.path
        || result.transport_timestamp != context.transport.timestamp
        || result.transport_nonce != context.transport.nonce
        || result.transport_payload_sha256 != context.transport.payload_sha256
        || result.transport_payload_bytes != context.transport.payload_bytes
        || result.transport_replay_checked != context.transport.replay_checked
        || context.transport.schema_version != TRANSPORT_SCHEMA_VERSION
        || context.transport.evaluation_mode != TRANSPORT_EVALUATION_MODE
        || !context.transport.preview_only
        || !transport_authority_clear
        || result.transport_timestamp <= 0
        || result.transport_payload_bytes == 0
        || result.transport_payload_bytes > MAX_PAYLOAD_BYTES as u64
        || result.transport_method != "POST"
        || result.transport_path != format!("/api/v1/runners/{}/dispatch", result.target_id)
        || result.transport_binding_valid != transport_binding
        || response_readiness_invalid(result)
    {
        return Err(invalid_response());
    }
    if result.admission_ready && !result.rejection_reasons.is_empty() {
        return Err(invalid_response());
    }
    Ok(())
}

fn admission_reasons(result: &AdmissionResult) -> Vec<String> {
    let mut reasons = Vec::new();
    if !result.command_binding_valid {
        reasons.push("command_binding_invalid".to_owned());
    }
    if !result.lease_proof_current {
        reasons.push("lease_proof_not_current".to_owned());
    }
    if !result.lease_active {
        reasons.push("lease_inactive_at_evaluated_time".to_owned());
    }
    if !result.attempt_state_admissible {
        reasons.push("attempt_state_not_dispatchable".to_owned());
    }
    if !result.transport_binding_valid {
        reasons.push("transport_binding_invalid".to_owned());
    }
    reasons.sort();
    reasons.dedup();
    reasons
}

fn request_field<T: serde::de::DeserializeOwned>(
    request: &Map<String, Value>,
    key: &str,
) -> Result<T, RemoteError> {
    serde_json::from_value(request.get(key).cloned().ok_or_else(invalid_request)?)
        .map_err(|_| invalid_request())
}

fn response_readiness_invalid(result: &AdmissionResult) -> bool {
    let reasons = admission_reasons(result);
    result.attempt_state_admissible != dispatchable_attempt_state(&result.attempt_state)
        || result.admission_ready
            != (result.command_binding_valid
                && result.lease_proof_current
                && result.lease_active
                && result.attempt_state_admissible
                && result.transport_binding_valid)
        || result.rejection_reasons != reasons
        || result
            .rejection_reasons
            .iter()
            .any(|reason| !valid_identifier(reason))
        || !result.preview_only
        || result.authority.device_identity_verified
        || result.authority.transport_authenticated
        || result.authority.reservation_created
        || result.authority.execution_authorized
        || result.authority.dispatch_performed
        || result.authority.audit_published
}
