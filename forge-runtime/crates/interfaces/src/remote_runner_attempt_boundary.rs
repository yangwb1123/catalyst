//! Strict authenticated consumer for the Runner Attempt lifecycle preview.
//!
//! The request is the same redacted execution-boundary declaration accepted by
//! Core with one caller-declared lifecycle transition. The response is a
//! metadata-only observation; this module never persists an Attempt, mutates a
//! lease, opens Runner transport, or authorizes argv execution.

use std::{
    fs::File,
    io::{self, Read, Write},
};

use forge_runtime_domain::execution::{
    runner_attempt_boundary::{RunnerAttemptBoundaryObservation, decode},
    runner_command::RunnerCommand,
};
use serde_json::{Map, Value};

use super::RemoteError;

const MAX_INPUT_BYTES: usize = 512 * 1024;
const TRANSITIONS: [&str; 7] = [
    "accept",
    "begin_starting",
    "observe_running",
    "observe_interrupted",
    "observe_completed",
    "observe_failed",
    "observe_effect_outcome_uncertain",
];

pub(super) fn read_request(input: &str) -> Result<Value, RemoteError> {
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes).map_err(|_| {
        RemoteError("remote Runner Attempt boundary input contains duplicate JSON keys".into())
    })?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| invalid_request())?;
    validate_request(&value)?;
    Ok(value)
}

pub(super) fn read_tui_request(input: &str) -> Result<Value, RemoteError> {
    if input == "-" {
        return Err(RemoteError(
            "remote TUI Runner Attempt boundary requires a file path; '-' belongs to the standalone CLI".into(),
        ));
    }
    read_request(input)
}

pub(super) fn conversation_and_run(value: &Value) -> Result<(String, String), RemoteError> {
    validate_request(value)?;
    super::runner_execution_boundary::conversation_and_run(value)
}

pub(super) fn target_id(value: &Value) -> Result<&str, RemoteError> {
    super::runner_execution_boundary::target_id(value)
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
            "transition",
        ],
    ) {
        return Err(invalid_request());
    }
    let transition = object
        .get("transition")
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    if !TRANSITIONS.contains(&transition) {
        return Err(invalid_request());
    }
    let mut execution_request = object.clone();
    execution_request.remove("transition");
    super::runner_execution_boundary::validate_request(&Value::Object(execution_request))
}

pub(super) fn validate_response(
    value: &Value,
    request: &Value,
    conversation_id: &str,
    run_id: &str,
) -> Result<(), RemoteError> {
    validate_request(request)?;
    let observation = decode_observation(value)?;
    observation.validate().map_err(|_| invalid_response())?;
    let request_object = request.as_object().ok_or_else(invalid_request)?;
    let owner = request_object.get("owner").ok_or_else(invalid_request)?;
    let command: RunnerCommand = serde_json::from_value(
        request_object
            .get("command")
            .cloned()
            .ok_or_else(invalid_request)?,
    )
    .map_err(|_| invalid_request())?;
    let attempt_id = string_field(request_object, "attempt_id")?;
    let attempt_state = string_field(request_object, "attempt_state")?;
    let transition = string_field(request_object, "transition")?;
    if serde_json::to_value(&observation.owner).map_err(|_| invalid_response())? != *owner
        || observation.conversation_id != conversation_id
        || observation.run_id != run_id
        || observation.attempt_id != attempt_id
        || observation.current_attempt_state != attempt_state
        || observation.transition != transition
        || observation.command_id != command.command_id
        || observation.target_id != command.lease_proof.target_id
        || observation.lease_epoch != command.lease_proof.epoch
    {
        return Err(invalid_response());
    }
    Ok(())
}

pub(super) fn render_human(value: &Value, writer: &mut impl Write) -> io::Result<()> {
    let observation = decode_observation(value).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid Runner Attempt boundary response",
        )
    })?;
    writeln!(
        writer,
        "Runner Attempt boundary [{}] owner={}/{}/{} conversation={} run={} attempt={} command={} target={} epoch={}",
        observation.schema_version,
        observation.owner.issuer,
        observation.owner.subject,
        observation.owner.tenant_id,
        observation.conversation_id,
        observation.run_id,
        observation.attempt_id,
        observation.command_id,
        observation.target_id,
        observation.lease_epoch
    )?;
    writeln!(
        writer,
        "lifecycle: {} -> {} transition={} execution_boundary_ready={} transition_valid={} transition_dispatchable={} attempt_boundary_ready={}",
        observation.current_attempt_state,
        observation.next_attempt_state,
        observation.transition,
        observation.execution_boundary_ready,
        observation.attempt_transition_valid,
        observation.attempt_transition_dispatchable,
        observation.attempt_boundary_ready
    )?;
    writeln!(
        writer,
        "rejection_reasons={:?}; preview_only=true authority: attempt_persisted=false reservation_created=false execution_authorized=false dispatch_performed=false audit_published=false (fencing token, argv, workspace, transport payload, and Runner output withheld)",
        observation.rejection_reasons
    )
}

fn decode_observation(value: &Value) -> Result<RunnerAttemptBoundaryObservation, RemoteError> {
    let bytes = serde_json::to_vec(value).map_err(|_| invalid_response())?;
    decode(&bytes).map_err(|_| invalid_response())
}

fn string_field<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RemoteError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)
}

fn exact_fields<const N: usize>(object: &Map<String, Value>, fields: [&str; N]) -> bool {
    object.len() == N && fields.into_iter().all(|field| object.contains_key(field))
}

fn invalid_request() -> RemoteError {
    RemoteError("remote Runner Attempt boundary input is invalid".into())
}

fn invalid_response() -> RemoteError {
    RemoteError("Forge API returned an invalid Runner Attempt boundary".into())
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, RemoteError> {
    let mut bytes = Vec::with_capacity(MAX_INPUT_BYTES.min(64 * 1024));
    if input == "-" {
        io::stdin()
            .lock()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("failed to read Runner Attempt boundary input".into()))?;
    } else {
        File::open(input)
            .map_err(|_| RemoteError("failed to open Runner Attempt boundary input".into()))?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("failed to read Runner Attempt boundary input".into()))?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(RemoteError(
            "Runner Attempt boundary input exceeds size limit".into(),
        ));
    }
    Ok(bytes)
}
