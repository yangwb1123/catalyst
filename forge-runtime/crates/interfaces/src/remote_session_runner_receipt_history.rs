//! Strict authenticated consumer for the session Runner receipt-history
//! preview. The Coordinator returns a pure reduction of the caller-supplied
//! history; this client rechecks the same reduction before rendering it.

use std::{
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use forge_runtime_domain::execution::session_runner_receipt_history::{
    SessionRunnerReceiptHistoryObservation, SessionRunnerReceiptHistoryRequest,
    observe_session_runner_receipt_history,
};
use serde_json::{Map, Value};

use super::RemoteError;

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const SCHEMA_VERSION: &str = "forge.session-runner-receipt-history/v1";
const EVALUATION_MODE: &str = "pure_session_runner_receipt_history_only";
const MAX_RECEIPTS: usize = 16;

/// Reads one bounded canonical history from a file or stdin. Duplicate keys
/// are rejected before serde's last-key-wins decoder can hide drift.
pub(super) fn read_request(input: &str) -> Result<Value, RemoteError> {
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes).map_err(|_| {
        RemoteError(
            "remote session Runner receipt history input contains duplicate JSON keys".into(),
        )
    })?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| {
        RemoteError("remote session Runner receipt history input is invalid JSON".into())
    })?;
    validate_value(&value)?;
    Ok(value)
}

pub(super) fn read_tui_request(input: &str) -> Result<Value, RemoteError> {
    if input == "-" {
        return Err(RemoteError(
            "remote TUI session Runner receipt history preview requires a file path; '-' belongs to the standalone CLI".into(),
        ));
    }
    read_request(input)
}

pub(super) fn conversation_and_run(value: &Value) -> Result<(&str, &str), RemoteError> {
    let object = value.as_object().ok_or_else(|| {
        RemoteError("remote session Runner receipt history is not an object".into())
    })?;
    let conversation_id = object
        .get("conversation_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RemoteError("remote session Runner receipt history Conversation is invalid".into())
        })?;
    let run_id = object
        .get("run_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RemoteError("remote session Runner receipt history Run is invalid".into())
        })?;
    super::validation::validate_conversation_id(conversation_id)?;
    super::validation::validate_entity_id(run_id, "Run")?;
    Ok((conversation_id, run_id))
}

/// Validates the response against the exact request reduction and URL path.
/// The response is derived by Core, so any binding, lifecycle, summary, or
/// authority drift is rejected before CLI/TUI rendering.
pub(super) fn validate_response(
    response: &Value,
    request: &Value,
    conversation_id: &str,
    run_id: &str,
) -> Result<(), RemoteError> {
    validate_value(request)?;
    validate_value(response)?;
    let (request_conversation, request_run) = conversation_and_run(request)?;
    if request_conversation != conversation_id || request_run != run_id {
        return Err(RemoteError(
            "remote session Runner receipt history request does not match the URL path".into(),
        ));
    }
    let input: SessionRunnerReceiptHistoryObservation =
        serde_json::from_value(request.clone()).map_err(|_| invalid_history())?;
    let expected = observe_session_runner_receipt_history(SessionRunnerReceiptHistoryRequest {
        owner: input.owner.clone(),
        conversation_id: input.conversation_id.clone(),
        prompt_id: input.prompt_id.clone(),
        run_id: input.run_id.clone(),
        receipts: input.receipts.clone(),
    })
    .map_err(|_| invalid_history())?;
    let actual: SessionRunnerReceiptHistoryObservation =
        serde_json::from_value(response.clone()).map_err(|_| invalid_history())?;
    if actual != expected {
        return Err(RemoteError(
            "Forge API returned a session Runner receipt history with a different reduction".into(),
        ));
    }
    Ok(())
}

pub(super) fn validate_value(value: &Value) -> Result<(), RemoteError> {
    let object = value.as_object().ok_or_else(invalid_history)?;
    if !exact_fields(
        object,
        [
            "schema_version",
            "evaluation_mode",
            "owner",
            "conversation_id",
            "prompt_id",
            "run_id",
            "receipts",
            "attempt_count",
            "latest_attempt_id",
            "latest_command_id",
            "latest_target_id",
            "latest_disposition_kind",
            "latest_observed_at_ms",
            "reconciliation_required",
            "manual_review_required",
            "automatic_retry",
            "follow_up",
            "selected_target_id",
            "preview_only",
            "authority",
        ],
    ) {
        return Err(invalid_history());
    }
    if object.get("schema_version").and_then(Value::as_str) != Some(SCHEMA_VERSION)
        || object.get("evaluation_mode").and_then(Value::as_str) != Some(EVALUATION_MODE)
        || !object.get("selected_target_id").is_some_and(Value::is_null)
    {
        return Err(invalid_history());
    }
    if object.values().any(Value::is_null) {
        // selected_target_id is the one intentional null field.
        if object
            .iter()
            .any(|(field, value)| field != "selected_target_id" && value.is_null())
        {
            return Err(invalid_history());
        }
    }
    exact_nested_fields(object.get("owner"), ["issuer", "subject", "tenant_id"])?;
    exact_nested_fields(
        object.get("authority"),
        [
            "identity_verified",
            "receipt_persisted",
            "execution_authorized",
            "dispatch_performed",
            "audit_published",
        ],
    )?;
    let receipts = object
        .get("receipts")
        .and_then(Value::as_array)
        .ok_or_else(invalid_history)?;
    if receipts.is_empty() || receipts.len() > MAX_RECEIPTS {
        return Err(invalid_history());
    }
    for receipt in receipts {
        super::session_runner_receipt::validate_value(receipt)?;
    }
    let observation: SessionRunnerReceiptHistoryObservation =
        serde_json::from_value(value.clone()).map_err(|_| invalid_history())?;
    observation.validate().map_err(|_| invalid_history())
}

pub(super) fn render_human(value: &Value, writer: &mut impl Write) -> io::Result<()> {
    let observation: SessionRunnerReceiptHistoryObservation =
        serde_json::from_value(value.clone()).expect("validated session receipt history");
    writeln!(
        writer,
        "authenticated session Runner receipt history [{}]",
        observation.schema_version
    )?;
    writeln!(
        writer,
        "owner={} conversation={} prompt={} run={}",
        observation.owner.subject,
        observation.conversation_id,
        observation.prompt_id,
        observation.run_id
    )?;
    writeln!(writer, "attempt_count={}", observation.attempt_count)?;
    for (index, receipt) in observation.receipts.iter().enumerate() {
        let terminal = &receipt.receipt_observation;
        writeln!(
            writer,
            "attempt[{}]: command={} attempt={} target={} disposition={} observed_at_ms={} uncertain={}",
            index + 1,
            terminal.command_id,
            terminal.attempt_id,
            terminal.target_id,
            terminal.disposition_kind,
            terminal.observed_at_ms,
            terminal.uncertain
        )?;
    }
    writeln!(
        writer,
        "latest: command={} attempt={} target={} disposition={} observed_at_ms={}",
        observation.latest_command_id,
        observation.latest_attempt_id,
        observation.latest_target_id,
        observation.latest_disposition_kind,
        observation.latest_observed_at_ms
    )?;
    writeln!(
        writer,
        "follow_up={} reconciliation_required={} manual_review_required={} automatic_retry={}",
        observation.follow_up,
        observation.reconciliation_required,
        observation.manual_review_required,
        observation.automatic_retry
    )?;
    writeln!(
        writer,
        "binding: preview_only={} selected_target=none",
        observation.preview_only
    )?;
    writeln!(
        writer,
        "authority: identity_verified={} receipt_persisted={} execution_authorized={} dispatch_performed={} audit_published={}",
        observation.authority.identity_verified,
        observation.authority.receipt_persisted,
        observation.authority.execution_authorized,
        observation.authority.dispatch_performed,
        observation.authority.audit_published
    )
}

fn exact_fields<const N: usize>(object: &Map<String, Value>, fields: [&str; N]) -> bool {
    object.len() == N && fields.into_iter().all(|field| object.contains_key(field))
}

fn exact_nested_fields<const N: usize>(
    value: Option<&Value>,
    fields: [&str; N],
) -> Result<(), RemoteError> {
    let object = value
        .and_then(Value::as_object)
        .ok_or_else(invalid_history)?;
    if !exact_fields(object, fields) || object.values().any(Value::is_null) {
        return Err(invalid_history());
    }
    Ok(())
}

fn invalid_history() -> RemoteError {
    RemoteError("remote session Runner receipt history is invalid".into())
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, RemoteError> {
    let mut bytes = Vec::new();
    if input == "-" {
        io::stdin()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| {
                RemoteError("remote session Runner receipt history input could not be read".into())
            })?;
    } else {
        File::open(Path::new(input))
            .map_err(|_| {
                RemoteError(
                    "remote session Runner receipt history input could not be opened".into(),
                )
            })?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| {
                RemoteError("remote session Runner receipt history input could not be read".into())
            })?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(RemoteError(format!(
            "remote session Runner receipt history input exceeds {MAX_INPUT_BYTES} bytes"
        )));
    }
    Ok(bytes)
}
