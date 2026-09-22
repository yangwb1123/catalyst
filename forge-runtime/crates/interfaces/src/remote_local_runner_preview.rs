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

/// Validates the complete request envelope against the pure domain rules.
/// In particular, the lease proof is checked against the supplied grant and
/// the explicit observation time, but the grant is never adopted or renewed.
fn validate_request(value: &Value) -> Result<(), RemoteError> {
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
mod tests {
    use serde_json::{Value, json};

    use super::{conversation_and_intent, read_request, validate_response};

    fn request() -> Value {
        let fixture = serde_json::from_str::<Value>(include_str!(
            "../../../../docs/contracts/fixtures/forge-runner-execution-intent-v1.json"
        ))
        .expect("Runner intent fixture");
        let intent = json!({
            "owner": fixture["owner"],
            "conversation_id": fixture["conversation_id"],
            "prompt_receipt": fixture["prompt_receipt"],
            "run_reference": fixture["run_reference"],
            "execution_intent": fixture["execution_intent"],
            "command": fixture["command"]
        });
        json!({
            "intent": intent,
            "grant": {
                "v": 1,
                "attempt_id": "attempt-001",
                "target_id": "runner-1",
                "epoch": 1,
                "fencing_token": "fence-001",
                "issued_at_ms": 1000,
                "expires_at_ms": 11000
            },
            "observed_at_ms": 2000
        })
    }

    fn response(request: &Value) -> Value {
        let intent = &request["intent"];
        let intent_observation = json!({
            "schema_version": "forge.runner-execution-intent/v1",
            "evaluation_mode": "pure_runner_binding_only",
            "owner": intent["owner"],
            "conversation_id": intent["conversation_id"],
            "prompt_id": intent["prompt_receipt"]["prompt_id"],
            "run_id": intent["run_reference"]["run_id"],
            "attempt_id": intent["execution_intent"]["attempt_id"],
            "command_id": intent["execution_intent"]["command_id"],
            "target_id": intent["execution_intent"]["target_id"],
            "command_sha256": intent["execution_intent"]["command_sha256"],
            "idempotency_key": intent["execution_intent"]["idempotency_key"],
            "prompt_run_binding_valid": true,
            "runner_command_binding_valid": true,
            "preview_only": true,
            "selected_target_id": null,
            "authority": {
                "device_identity_verified": false,
                "command_persisted": false,
                "reservation_created": false,
                "execution_authorized": false,
                "dispatch_performed": false,
                "audit_published": false
            }
        });
        let receipt = json!({
            "schema_version": "forge.runner-command-terminal-receipt/v1",
            "evaluation_mode": "pure_runner_command_receipt_only",
            "command_id": intent["execution_intent"]["command_id"],
            "command_sha256": intent["execution_intent"]["command_sha256"],
            "attempt_id": intent["execution_intent"]["attempt_id"],
            "target_id": intent["execution_intent"]["target_id"],
            "disposition_kind": "completed",
            "observed_at_ms": 2000,
            "receipt_valid": true,
            "preview_only": true,
            "uncertain": false,
            "reconciliation_required": false,
            "manual_review_required": false,
            "automatic_retry": false,
            "follow_up": "none",
            "authority": {
                "device_identity_verified": false,
                "command_persisted": false,
                "reservation_created": false,
                "execution_authorized": false,
                "dispatch_performed": false,
                "audit_published": false
            }
        });
        json!({
            "schema_version": "forge.runner-local-execution-preview/v1",
            "evaluation_mode": "injected_local_runner_preview_only",
            "runner_execution_intent": intent_observation,
            "session_runner_receipt": {
                "schema_version": "forge.session-runner-receipt-observation/v1",
                "evaluation_mode": "pure_session_runner_receipt_binding_only",
                "owner": intent["owner"],
                "conversation_id": intent["conversation_id"],
                "prompt_id": intent["prompt_receipt"]["prompt_id"],
                "run_id": intent["run_reference"]["run_id"],
                "receipt_observation": receipt,
                "prompt_run_binding_valid": true,
                "receipt_binding_valid": true,
                "preview_only": true,
                "selected_target_id": null,
                "authority": {
                    "identity_verified": false,
                    "receipt_persisted": false,
                    "execution_authorized": false,
                    "dispatch_performed": false,
                    "audit_published": false
                }
            },
            "command_id": intent["execution_intent"]["command_id"],
            "attempt_id": intent["execution_intent"]["attempt_id"],
            "target_id": intent["execution_intent"]["target_id"],
            "command_sha256": intent["execution_intent"]["command_sha256"],
            "disposition_kind": "completed",
            "observed_at_ms": 2000,
            "output_bytes": 0,
            "exit_code": 0,
            "executor_invoked": true,
            "preview_only": true,
            "authority": {
                "device_identity_verified": false,
                "command_persisted": false,
                "reservation_created": false,
                "execution_authorized": false,
                "dispatch_performed": false,
                "audit_published": false
            }
        })
    }

    #[test]
    fn request_is_bounded_and_path_bound() {
        let request = request();
        assert_eq!(
            conversation_and_intent(&request).expect("path identities"),
            ("conversation-001", "intent-001")
        );

        let duplicate = serde_json::to_string(&request)
            .expect("request JSON")
            .replacen(
                "\"observed_at_ms\":2000",
                "\"observed_at_ms\":2000,\"observed_at_ms\":2000",
                1,
            );
        let path = tempfile::NamedTempFile::new().expect("request file");
        std::fs::write(path.path(), duplicate).expect("write request");
        assert!(read_request(path.path().to_str().expect("request path")).is_err());
    }

    #[test]
    fn response_requires_identity_and_false_authority() {
        let request = request();
        let response = response(&request);
        validate_response(&response, &request, "conversation-001", "intent-001")
            .expect("valid local preview response");

        let mut foreign = response.clone();
        foreign["runner_execution_intent"]["owner"]["subject"] = json!("other-user");
        assert!(validate_response(&foreign, &request, "conversation-001", "intent-001").is_err());

        let mut authority = response.clone();
        authority["authority"]["execution_authorized"] = json!(true);
        assert!(validate_response(&authority, &request, "conversation-001", "intent-001").is_err());

        assert!(validate_response(&response, &request, "conversation-002", "intent-001").is_err());
    }

    #[test]
    fn human_output_contains_metadata_only() {
        let request = request();
        let response = response(&request);
        let mut output = Vec::new();
        super::render_human(&response, &mut output).expect("human output");
        let output = String::from_utf8(output).expect("UTF-8 output");
        assert!(output.contains("authority: device_identity_verified=false"));
        assert!(!output.contains("forge-task"));
        assert!(!output.contains("fence-001"));
    }
}
