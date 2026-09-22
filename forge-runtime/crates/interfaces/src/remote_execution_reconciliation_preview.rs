//! Strict authenticated consumer for the execution-reconciliation preview.
//!
//! The Coordinator endpoint accepts one caller-supplied restart image and
//! returns a content-free classification. This client validates the same pure
//! domain projection before rendering it; it never retries the POST, renews a
//! lease, selects a target, or treats reconciliation as an instruction.

use std::{
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use forge_runtime_domain::execution::reconciliation::{
    ReconciliationInput, ReconciliationObservation, observe,
};
use serde_json::{Map, Value};

use super::{RemoteError, validation};

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;

/// Reads one bounded, exact restart image from a file or stdin. Duplicate
/// keys are rejected before serde's last-key-wins decoder can hide a binding.
pub(super) fn read_request(input: &str) -> Result<Value, RemoteError> {
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes).map_err(|_| {
        RemoteError("remote execution reconciliation input contains duplicate JSON keys".into())
    })?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| RemoteError("remote execution reconciliation input is invalid JSON".into()))?;
    validate_request(&value)?;
    Ok(value)
}

/// The TUI uses a named file so an interactive terminal never consumes its
/// own input stream. `-` remains available to the standalone CLI.
pub(super) fn read_tui_request(input: &str) -> Result<Value, RemoteError> {
    if input == "-" {
        return Err(RemoteError(
            "remote execution reconciliation TUI input must be a file".into(),
        ));
    }
    read_request(input)
}

pub(super) fn conversation_and_run(value: &Value) -> Result<(String, String), RemoteError> {
    validate_request(value)?;
    let object = value.as_object().ok_or_else(|| {
        RemoteError("remote execution reconciliation request must be an object".into())
    })?;
    let conversation_id = object
        .get("conversation_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RemoteError("remote execution reconciliation Conversation is invalid".into())
        })?;
    let run_id = object
        .get("run_id")
        .and_then(Value::as_str)
        .ok_or_else(|| RemoteError("remote execution reconciliation Run is invalid".into()))?;
    validation::validate_conversation_id(conversation_id)?;
    validation::validate_entity_id(run_id, "Run")?;
    Ok((conversation_id.to_owned(), run_id.to_owned()))
}

pub(super) fn validate_response(
    value: &Value,
    request: &Value,
    conversation_id: &str,
    run_id: &str,
) -> Result<(), RemoteError> {
    let input = decode_request(request)?;
    let (expected_conversation, expected_run) = conversation_and_run(request)?;
    if expected_conversation != conversation_id || expected_run != run_id {
        return Err(RemoteError(
            "remote execution reconciliation request does not match the URL path".into(),
        ));
    }
    let response: ReconciliationObservation =
        serde_json::from_value(value.clone()).map_err(|_| {
            RemoteError("Forge API returned an invalid execution reconciliation observation".into())
        })?;
    response.validate().map_err(|_| {
        RemoteError("Forge API returned an invalid execution reconciliation observation".into())
    })?;
    let expected = observe(input)
        .map_err(|_| RemoteError("remote execution reconciliation request is invalid".into()))?;
    if response != expected {
        return Err(RemoteError(
            "Forge API returned an execution reconciliation observation with a different binding or classification".into(),
        ));
    }
    Ok(())
}

pub(super) fn validate_request(value: &Value) -> Result<(), RemoteError> {
    let object = value.as_object().ok_or_else(|| invalid_request())?;
    if !exact_fields(
        object,
        [
            "owner",
            "conversation_id",
            "run_id",
            "attempt_id",
            "command_id",
            "target_id",
            "run_status",
            "attempt_state",
            "lease",
            "observed_at_ms",
            "terminal",
        ],
    ) {
        return Err(invalid_request());
    }
    let input: ReconciliationInput =
        serde_json::from_value(value.clone()).map_err(|_| invalid_request())?;
    validation::validate_conversation_id(&input.conversation_id)?;
    validation::validate_entity_id(&input.run_id, "Run")?;
    validation::validate_entity_id(&input.attempt_id, "Attempt")?;
    validation::validate_entity_id(&input.command_id, "command")?;
    validation::validate_entity_id(&input.target_id, "target")?;
    observe(input).map_err(|_| invalid_request())?;
    Ok(())
}

fn decode_request(value: &Value) -> Result<ReconciliationInput, RemoteError> {
    validate_request(value)?;
    serde_json::from_value(value.clone()).map_err(|_| invalid_request())
}

pub(super) fn render_human(value: &Value, writer: &mut impl Write) -> io::Result<()> {
    let observation: ReconciliationObservation =
        serde_json::from_value(value.clone()).expect("validated reconciliation observation");
    writeln!(
        writer,
        "execution reconciliation preview [{}]",
        observation.schema_version
    )?;
    writeln!(
        writer,
        "owner={} conversation={} run={} attempt={} command={} target={}",
        observation.owner.subject,
        observation.conversation_id,
        observation.run_id,
        observation.attempt_id,
        observation.command_id,
        observation.target_id
    )?;
    writeln!(
        writer,
        "run_status={} attempt_state={} lease_epoch={} lease_active={} observed_at_ms={}",
        observation.run_status,
        observation.attempt_state,
        observation.lease_epoch,
        observation.lease_active,
        observation.observed_at_ms
    )?;
    writeln!(
        writer,
        "next_observation={} terminal={} disposition={} aligned={} reconciliation_required={} manual_review_required={} automatic_retry=false",
        observation.next_observation,
        observation.terminal_observed,
        observation.terminal_disposition,
        observation.terminal_state_aligned,
        observation.reconciliation_required,
        observation.manual_review_required
    )?;
    writeln!(
        writer,
        "preview_only=true authority: identity_verified=false run_authoritative=false attempt_persisted=false lease_issued=false terminal_persisted=false reservation_created=false execution_authorized=false dispatch_performed=false audit_published=false"
    )
}

fn exact_fields<const N: usize>(object: &Map<String, Value>, fields: [&str; N]) -> bool {
    object.len() == N && fields.into_iter().all(|field| object.contains_key(field))
}

fn invalid_request() -> RemoteError {
    RemoteError("remote execution reconciliation input is invalid".into())
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, RemoteError> {
    let mut bytes = Vec::with_capacity(MAX_INPUT_BYTES.min(64 * 1024));
    if input == "-" {
        io::stdin()
            .lock()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("could not read execution reconciliation stdin".into()))?;
    } else {
        let path = Path::new(input);
        let metadata = std::fs::metadata(path)
            .map_err(|_| RemoteError("could not read execution reconciliation input".into()))?;
        if !metadata.is_file() {
            return Err(RemoteError(
                "execution reconciliation input must be a regular file".into(),
            ));
        }
        File::open(path)
            .map_err(|_| RemoteError("could not read execution reconciliation input".into()))?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("could not read execution reconciliation input".into()))?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(RemoteError(format!(
            "execution reconciliation input exceeds {MAX_INPUT_BYTES} bytes"
        )));
    }
    Ok(bytes)
}
