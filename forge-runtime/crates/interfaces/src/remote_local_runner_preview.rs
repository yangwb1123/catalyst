//! Strict remote consumer for the test-only local Runner execution preview.
//!
//! The Forge Core candidate behind this module is deliberately private and is
//! not mounted by the production session constructor.  This module therefore
//! accepts only bounded, caller-supplied values and keeps the same properties
//! at the Rust boundary: the request is an existing intent plus a lease
//! observation, the response is metadata-only, and every authority bit must
//! remain false.

use std::{
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use forge_runtime_domain::execution::{
    lease::LeaseGrant,
    runner_execution_intent::{
        RunnerExecutionIntentAuthority, RunnerExecutionIntentObservation,
        RunnerExecutionIntentRequest, RunnerExecutionOwner, observe_runner_execution_intent,
    },
    session_runner_receipt::SessionRunnerReceiptObservation,
};
use serde::Deserialize;
use serde_json::Value;

use super::{RemoteError, validation};

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const SCHEMA_VERSION: &str = "forge.runner-local-execution-preview/v1";
const EVALUATION_MODE: &str = "injected_local_runner_preview_only";

/// The exact request envelope accepted by the Go local preview candidate.
/// Every nested domain value has `deny_unknown_fields`, so decoding this
/// value also rejects fields which could otherwise be silently ignored.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LocalRunnerPreviewRequest {
    intent: RunnerExecutionIntentRequest,
    grant: LeaseGrant,
    observed_at_ms: u64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "Published wire predicates are separate booleans; changing their representation would change the protocol."
)]
struct LocalRunnerPreviewAuthority {
    device_identity_verified: bool,
    command_persisted: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
    audit_published: bool,
}

/// `RunnerExecutionIntentObservation` owns static schema strings and is
/// intentionally Serialize-only in the domain crate. Decode the transported
/// value into this owned wire form, then compare it to the domain observation
/// derived from the request below.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunnerExecutionIntentWire {
    schema_version: String,
    evaluation_mode: String,
    owner: RunnerExecutionOwner,
    conversation_id: String,
    prompt_id: String,
    run_id: String,
    attempt_id: String,
    command_id: String,
    target_id: String,
    command_sha256: String,
    idempotency_key: String,
    prompt_run_binding_valid: bool,
    runner_command_binding_valid: bool,
    preview_only: bool,
    selected_target_id: Option<String>,
    authority: RunnerExecutionIntentAuthority,
}

/// Owned wire form for the metadata-only response.  The nested observations
/// are the canonical domain projections, so the response cannot grow a
/// second, weaker interpretation of a Runner receipt.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LocalRunnerPreviewObservation {
    schema_version: String,
    evaluation_mode: String,
    runner_execution_intent: RunnerExecutionIntentWire,
    session_runner_receipt: SessionRunnerReceiptObservation,
    command_id: String,
    attempt_id: String,
    target_id: String,
    command_sha256: String,
    disposition_kind: String,
    observed_at_ms: u64,
    output_bytes: u64,
    exit_code: i64,
    executor_invoked: bool,
    preview_only: bool,
    authority: LocalRunnerPreviewAuthority,
}

/// Reads one bounded local Runner preview request from a file or stdin.
///
/// The bytes are caller supplied and no Hub, Runner, clock, or credential is
/// touched here.  Duplicate keys are rejected before JSON decoding because a
/// last-key-wins parser would make the signed/requested identity ambiguous.
pub(super) fn read_request(input: &str) -> Result<Value, RemoteError> {
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes).map_err(|_| {
        RemoteError("remote local Runner preview input contains duplicate JSON keys".into())
    })?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| RemoteError("remote local Runner preview input is invalid JSON".into()))?;
    validate_request(&value)?;
    Ok(value)
}

/// The TUI always reads a named file.  Keeping stdin exclusive to the
/// standalone CLI prevents an interactive command from consuming its own
/// terminal stream.
pub(super) fn read_tui_request(input: &str) -> Result<Value, RemoteError> {
    if input == "-" {
        return Err(RemoteError(
            "remote TUI local Runner preview requires a file path; '-' belongs to the standalone CLI"
                .into(),
        ));
    }
    read_request(input)
}

/// Returns the URL-bound Conversation and Run-intent identities from a valid
/// request.  The intent ID is the accepted Prompt intent ID, matching the Go
/// route's `{intent_id}` segment.
pub(super) fn conversation_and_intent(value: &Value) -> Result<(&str, &str), RemoteError> {
    validate_request(value)?;
    let object = value.as_object().ok_or_else(|| {
        RemoteError("remote local Runner preview request must be an object".into())
    })?;
    let intent = object
        .get("intent")
        .and_then(Value::as_object)
        .ok_or_else(|| RemoteError("remote local Runner preview intent is invalid".into()))?;
    let conversation_id = intent
        .get("conversation_id")
        .and_then(Value::as_str)
        .ok_or_else(|| RemoteError("remote local Runner preview Conversation is invalid".into()))?;
    let prompt = intent
        .get("prompt_receipt")
        .and_then(Value::as_object)
        .ok_or_else(|| RemoteError("remote local Runner preview Prompt is invalid".into()))?;
    let intent_id = prompt
        .get("intent_id")
        .and_then(Value::as_str)
        .ok_or_else(|| RemoteError("remote local Runner preview intent is invalid".into()))?;
    validation::validate_conversation_id(conversation_id)?;
    validation::validate_entity_id(intent_id, "Run-intent")?;
    Ok((conversation_id, intent_id))
}

/// Returns the target Runner instance declared by a valid local preview
/// request. The value is still only a caller supplied observation; the TUI
/// uses it to keep an explicit client-instance projection from previewing a
/// target absent from its current resource image.
pub(super) fn target_id(value: &Value) -> Result<&str, RemoteError> {
    validate_request(value)?;
    let target_id = value
        .get("intent")
        .and_then(|intent| intent.get("execution_intent"))
        .and_then(|execution_intent| execution_intent.get("target_id"))
        .and_then(Value::as_str)
        .ok_or_else(|| RemoteError("remote local Runner preview target is invalid".into()))?;
    validation::validate_entity_id(target_id, "Runner target")?;
    Ok(target_id)
}

/// Validates the complete request envelope against the pure domain rules.
/// In particular, the lease proof is checked against the supplied grant and
/// the explicit observation time, but the grant is never adopted or renewed.
pub(super) fn validate_request(value: &Value) -> Result<(), RemoteError> {
    let request = decode_request(value)?;
    if request.observed_at_ms == 0 || request.observed_at_ms > MAX_SAFE_INTEGER {
        return Err(invalid_request());
    }
    if request.grant.issued_at_ms > MAX_SAFE_INTEGER
        || request.grant.expires_at_ms > MAX_SAFE_INTEGER
    {
        return Err(invalid_request());
    }
    observe_runner_execution_intent(request.intent.clone()).map_err(|_| invalid_request())?;
    request.grant.validate().map_err(|_| invalid_request())?;
    request
        .grant
        .validate_proof(&request.intent.command.lease_proof, request.observed_at_ms)
        .map_err(|_| invalid_request())?;
    Ok(())
}

/// Revalidates a response against the exact request and URL path.  The
/// response is required to be the same owner/Prompt/Run/command identity,
/// while output content, argv, lease tokens, and diagnostics are absent from
/// the wire projection.
pub(super) fn validate_response(
    value: &Value,
    request: &Value,
    conversation_id: &str,
    intent_id: &str,
) -> Result<(), RemoteError> {
    let request_value = decode_request(request)?;
    validate_request(request)?;
    validation::validate_conversation_id(conversation_id)?;
    validation::validate_entity_id(intent_id, "Run-intent")?;
    let (request_conversation_id, request_intent_id) = conversation_and_intent(request)?;
    if request_conversation_id != conversation_id || request_intent_id != intent_id {
        return Err(RemoteError(
            "local Runner preview request does not match the URL path".into(),
        ));
    }

    let response: LocalRunnerPreviewObservation = serde_json::from_value(value.clone())
        .map_err(|_| RemoteError("Forge API returned an invalid local Runner preview".into()))?;
    let expected_intent = observe_runner_execution_intent(request_value.intent.clone())
        .map_err(|_| RemoteError("local Runner preview request is invalid".into()))?;
    if response.schema_version != SCHEMA_VERSION
        || response.evaluation_mode != EVALUATION_MODE
        || !response.runner_execution_intent.matches(&expected_intent)
        || !response.executor_invoked
        || !response.preview_only
        || response.authority != LocalRunnerPreviewAuthority::default()
        || response.observed_at_ms != request_value.observed_at_ms
        || response.observed_at_ms == 0
        || response.observed_at_ms > MAX_SAFE_INTEGER
        || response.output_bytes > MAX_SAFE_INTEGER
        || response.exit_code.unsigned_abs() > MAX_SAFE_INTEGER
    {
        return Err(RemoteError(
            "Forge API returned a local Runner preview with invalid authority or binding".into(),
        ));
    }

    validate_session_binding(&response, &request_value)?;
    Ok(())
}

/// Renders only bounded metadata.  No argv, lease token, output text, or
/// executor error is present in this output path.
pub(super) fn render_human(value: &Value, writer: &mut impl Write) -> io::Result<()> {
    let response: LocalRunnerPreviewObservation =
        serde_json::from_value(value.clone()).expect("validated local Runner preview observation");
    let intent = &response.runner_execution_intent;
    let session = &response.session_runner_receipt;
    writeln!(
        writer,
        "local Runner execution-readiness preview [{}]",
        response.schema_version
    )?;
    writeln!(
        writer,
        "owner={} conversation={} prompt={} run={}",
        intent.owner.subject, intent.conversation_id, intent.prompt_id, intent.run_id
    )?;
    writeln!(
        writer,
        "command={} attempt={} target={} disposition={} observed_at_ms={} output_bytes={} exit_code={}",
        response.command_id,
        response.attempt_id,
        response.target_id,
        response.disposition_kind,
        response.observed_at_ms,
        response.output_bytes,
        response.exit_code
    )?;
    writeln!(
        writer,
        "binding: prompt_run_binding_valid={} runner_command_binding_valid={} receipt_binding_valid={} preview_only={} executor_invoked={}",
        intent.prompt_run_binding_valid,
        intent.runner_command_binding_valid,
        session.receipt_binding_valid,
        response.preview_only,
        response.executor_invoked
    )?;
    writeln!(
        writer,
        "authority: device_identity_verified=false command_persisted=false reservation_created=false execution_authorized=false dispatch_performed=false audit_published=false"
    )
}

fn decode_request(value: &Value) -> Result<LocalRunnerPreviewRequest, RemoteError> {
    serde_json::from_value(value.clone()).map_err(|_| invalid_request())
}

impl RunnerExecutionIntentWire {
    fn matches(&self, expected: &RunnerExecutionIntentObservation) -> bool {
        self.schema_version == expected.schema_version
            && self.evaluation_mode == expected.evaluation_mode
            && self.owner == expected.owner
            && self.conversation_id == expected.conversation_id
            && self.prompt_id == expected.prompt_id
            && self.run_id == expected.run_id
            && self.attempt_id == expected.attempt_id
            && self.command_id == expected.command_id
            && self.target_id == expected.target_id
            && self.command_sha256 == expected.command_sha256
            && self.idempotency_key == expected.idempotency_key
            && self.prompt_run_binding_valid == expected.prompt_run_binding_valid
            && self.runner_command_binding_valid == expected.runner_command_binding_valid
            && self.preview_only == expected.preview_only
            && self.selected_target_id == expected.selected_target_id
            && self.authority == expected.authority
    }
}

fn invalid_request() -> RemoteError {
    RemoteError("remote local Runner preview request is invalid".into())
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, RemoteError> {
    let mut bytes = Vec::with_capacity(MAX_INPUT_BYTES.min(64 * 1024));
    if input == "-" {
        io::stdin()
            .lock()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("could not read local Runner preview stdin".into()))?;
    } else {
        let path = Path::new(input);
        let metadata = std::fs::metadata(path)
            .map_err(|_| RemoteError("could not read local Runner preview input".into()))?;
        if !metadata.is_file() {
            return Err(RemoteError(
                "local Runner preview input must be a regular file".into(),
            ));
        }
        File::open(path)
            .map_err(|_| RemoteError("could not read local Runner preview input".into()))?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("could not read local Runner preview input".into()))?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(RemoteError(format!(
            "local Runner preview input exceeds {MAX_INPUT_BYTES} bytes"
        )));
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "remote_local_runner_preview/tests.rs"]
mod tests;

fn validate_session_binding(
    response: &LocalRunnerPreviewObservation,
    request_value: &LocalRunnerPreviewRequest,
) -> Result<(), RemoteError> {
    response.session_runner_receipt.validate().map_err(|_| {
        RemoteError("Forge API returned an invalid local Runner session receipt".into())
    })?;
    let intent = &response.runner_execution_intent;
    let session = &response.session_runner_receipt;
    let receipt = &session.receipt_observation;
    if session.owner != request_value.intent.owner
        || session.conversation_id != request_value.intent.conversation_id
        || session.prompt_id != request_value.intent.prompt_receipt.prompt_id
        || session.run_id != request_value.intent.run_reference.run_id
        || receipt.command_id != intent.command_id
        || receipt.attempt_id != intent.attempt_id
        || receipt.target_id != intent.target_id
        || receipt.command_sha256 != intent.command_sha256
        || receipt.observed_at_ms != response.observed_at_ms
        || response.command_id != intent.command_id
        || response.attempt_id != intent.attempt_id
        || response.target_id != intent.target_id
        || response.command_sha256 != intent.command_sha256
        || response.disposition_kind != receipt.disposition_kind
    {
        return Err(RemoteError(
            "Forge API returned a local Runner preview with a different identity binding".into(),
        ));
    }
    Ok(())
}
