//! Strict consumer for the durable lease-bound Runner dispatch admission
//! preview. The response is metadata only and never contains fencing data.

#[path = "remote_runner_dispatch_admission/request.rs"]
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
const MAX_TOKEN_BYTES: usize = 256;
const MAX_OWNER_PART_BYTES: usize = 512;
const MAX_ARGUMENTS: usize = 64;
const MAX_ARGUMENT_BYTES: usize = 4_096;
const MAX_ARGUMENT_TOTAL_BYTES: usize = 65_536;
const MAX_OUTPUT_BYTES: u64 = 8 * 1024 * 1024;
const MAX_TIMEOUT_MS: u64 = 600_000;
const SCHEMA_VERSION: &str = "forge.runner-dispatch-admission/v1";
const EVALUATION_MODE: &str = "durable_lease_bound_dispatch_admission_preview";

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
struct Authority {
    device_identity_verified: bool,
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
    lease_proof_current: bool,
    lease_active: bool,
    command_binding_valid: bool,
    admission_ready: bool,
    rejection_reasons: Vec<String>,
    preview_only: bool,
    authority: Authority,
}

pub(super) fn read_request(input: &str) -> Result<Value, RemoteError> {
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes).map_err(|_| {
        RemoteError("remote Runner dispatch admission input contains duplicate JSON keys".into())
    })?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| {
        RemoteError("remote Runner dispatch admission input is invalid JSON".into())
    })?;
    validate_request(&value)?;
    Ok(value)
}

pub(super) fn read_tui_request(input: &str) -> Result<Value, RemoteError> {
    if input == "-" {
        return Err(RemoteError(
            "remote TUI Runner dispatch admission requires a file path; '-' belongs to the standalone CLI".into(),
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
            "invalid Runner dispatch admission response",
        )
    })?;
    writeln!(
        writer,
        "Runner dispatch admission [{}] owner={}/{} tenant={} conversation={} run={} attempt={} command={} target={} epoch={} lease_current={} lease_active={} ready={} evaluated_at={}",
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
        result.lease_proof_current,
        result.lease_active,
        result.admission_ready,
        result.evaluated_at_ms
    )?;
    writeln!(
        writer,
        "rejection_reasons={:?}; execution_authorized=false dispatch_performed=false audit_published=false (fencing token, argv, workspace, and output withheld)",
        result.rejection_reasons
    )
}

fn exact_fields<const N: usize>(object: &Map<String, Value>, fields: [&str; N]) -> bool {
    object.len() == N && fields.into_iter().all(|field| object.contains_key(field))
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

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_TOKEN_BYTES
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
    RemoteError("remote Runner dispatch admission input is invalid".into())
}
fn invalid_response() -> RemoteError {
    RemoteError("Forge API returned an invalid Runner dispatch admission".into())
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, RemoteError> {
    let mut bytes = Vec::with_capacity(MAX_INPUT_BYTES.min(64 * 1024));
    if input == "-" {
        io::stdin()
            .lock()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("could not read Runner dispatch admission stdin".into()))?;
    } else {
        let path = Path::new(input);
        let metadata = std::fs::metadata(path)
            .map_err(|_| RemoteError("could not read Runner dispatch admission input".into()))?;
        if !metadata.is_file() {
            return Err(RemoteError(
                "Runner dispatch admission input must be a regular file".into(),
            ));
        }
        File::open(path)
            .map_err(|_| RemoteError("could not read Runner dispatch admission input".into()))?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("could not read Runner dispatch admission input".into()))?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(RemoteError(format!(
            "Runner dispatch admission input exceeds {MAX_INPUT_BYTES} bytes"
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
            "owner": {"issuer":"https://id.example","subject":"user-1","tenant_id":"tenant-1"},
            "conversation_id":"conversation-1","run_id":"run-1","attempt_id":"attempt-1","attempt_state":"accepted","evaluated_at_ms":300,
            "command": {"v":1,"command_id":"command-1","lease_proof":{"attempt_id":"attempt-1","target_id":"runner-a","epoch":1,"fencing_token":"token-a"},"idempotency_key":"run-1:attempt-1:command-1","workspace_ref":"workspace-1","argv":["forge-task","--prompt-ref","prompt-1"],"timeout_ms":5000,"max_output_bytes":65536}
        })
    }

    #[test]
    fn request_is_strict_and_response_is_metadata_only() {
        validate_request(&request()).expect("request");
        let mut unknown = request();
        unknown["extra"] = Value::Bool(true);
        assert!(validate_request(&unknown).is_err());
        assert!(valid_digest(&"a".repeat(64)));
    }
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
    validate_response_state(&result)
}

struct ResponseRequest<'a> {
    request_object: &'a Map<String, Value>,
    owner: Owner,
    command_id: &'a str,
    target_id: &'a str,
    epoch: u64,
    request_attempt_state: &'a str,
    expected_command_sha: String,
}

fn response_request(request: &Value) -> Result<ResponseRequest<'_>, RemoteError> {
    let request_object = request.as_object().ok_or_else(invalid_request)?;
    let owner: Owner = request_field(request_object, "owner")?;
    let command = request_object
        .get("command")
        .and_then(Value::as_object)
        .ok_or_else(invalid_request)?;
    let command_id = command
        .get("command_id")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    let proof = command
        .get("lease_proof")
        .and_then(Value::as_object)
        .ok_or_else(invalid_request)?;
    let target_id = proof
        .get("target_id")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    let epoch = proof
        .get("epoch")
        .and_then(Value::as_u64)
        .ok_or_else(invalid_request)?;
    let request_attempt_state = request_object
        .get("attempt_state")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    let runner_command: RunnerCommand =
        serde_json::from_value(Value::Object(command.clone())).map_err(|_| invalid_request())?;
    let expected_command_sha = runner_command
        .command_sha256()
        .map_err(|_| invalid_request())?;
    Ok(ResponseRequest {
        request_object,
        owner,
        command_id,
        target_id,
        epoch,
        request_attempt_state,
        expected_command_sha,
    })
}

fn validate_response_binding(
    result: &AdmissionResult,
    context: &ResponseRequest<'_>,
    conversation_id: &str,
    run_id: &str,
) -> Result<(), RemoteError> {
    let command_sha = result.command_sha256.as_str();
    if result.schema_version != SCHEMA_VERSION
        || result.evaluation_mode != EVALUATION_MODE
        || result.owner.issuer != context.owner.issuer
        || result.owner.subject != context.owner.subject
        || result.owner.tenant_id != context.owner.tenant_id
        || result.conversation_id != conversation_id
        || result.run_id != run_id
        || result.attempt_id
            != context
                .request_object
                .get("attempt_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
        || result.command_id != context.command_id
        || result.attempt_state != context.request_attempt_state
        || result.target_id != context.target_id
        || result.lease_epoch != context.epoch
        || command_sha != context.expected_command_sha
        || !valid_digest(command_sha)
    {
        return Err(invalid_response());
    }
    Ok(())
}

fn validate_response_state(result: &AdmissionResult) -> Result<(), RemoteError> {
    let reasons = admission_reasons(result);
    if result.lease_issued_at_ms == 0
        || result.lease_expires_at_ms <= result.lease_issued_at_ms
        || result.lease_expires_at_ms > MAX_SAFE_INTEGER
        || result.evaluated_at_ms == 0
        || result.evaluated_at_ms > MAX_SAFE_INTEGER
        || result.attempt_state_admissible
            != matches!(
                result.attempt_state.as_str(),
                "accepted" | "starting" | "running"
            )
        || result.admission_ready
            != (result.command_binding_valid
                && result.lease_proof_current
                && result.lease_active
                && result.attempt_state_admissible)
        || result.rejection_reasons != reasons
        || result
            .rejection_reasons
            .iter()
            .any(|reason| !valid_identifier(reason))
        || !result.preview_only
        || result.authority.device_identity_verified
        || result.authority.reservation_created
        || result.authority.execution_authorized
        || result.authority.dispatch_performed
        || result.authority.audit_published
    {
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
