use std::{
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use forge_runtime_domain::execution::session_runner_receipt::SessionRunnerReceiptObservation;
use serde_json::{Map, Value};

use super::RemoteError;

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const SCHEMA_VERSION: &str = "forge.session-runner-receipt-observation/v1";

/// Reads one bounded canonical session receipt observation from a file or
/// stdin. The value is caller supplied; this function performs no Hub read.
pub(super) fn read_request(input: &str) -> Result<Value, RemoteError> {
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes).map_err(|_| {
        RemoteError("remote session Runner receipt input contains duplicate JSON keys".into())
    })?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| RemoteError("remote session Runner receipt input is invalid JSON".into()))?;
    validate_value(&value)?;
    Ok(value)
}

pub(super) fn read_tui_request(input: &str) -> Result<Value, RemoteError> {
    if input == "-" {
        return Err(RemoteError(
            "remote TUI session Runner receipt preview requires a file path; '-' belongs to the standalone CLI"
                .into(),
        ));
    }
    read_request(input)
}

pub(super) fn conversation_and_run(value: &Value) -> Result<(&str, &str), RemoteError> {
    let object = value.as_object().ok_or_else(|| {
        RemoteError("remote session Runner receipt observation is not an object".into())
    })?;
    let conversation_id = object
        .get("conversation_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RemoteError("remote session Runner receipt Conversation is invalid".into())
        })?;
    let run_id = object
        .get("run_id")
        .and_then(Value::as_str)
        .ok_or_else(|| RemoteError("remote session Runner receipt Run is invalid".into()))?;
    super::validation::validate_conversation_id(conversation_id)?;
    super::validation::validate_entity_id(run_id, "Run")?;
    Ok((conversation_id, run_id))
}

/// Revalidates a response against the exact request envelope and URL binding.
/// The Go route is a canonical echo, so any owner/path/binding or authority
/// drift is rejected before TUI/CLI rendering.
pub(super) fn validate_response(
    response: &Value,
    request: &Value,
    conversation_id: &str,
    run_id: &str,
) -> Result<(), RemoteError> {
    validate_value(response)?;
    let (response_conversation_id, response_run_id) = conversation_and_run(response)?;
    if response_conversation_id != conversation_id || response_run_id != run_id {
        return Err(RemoteError(
            "Forge API returned a session Runner receipt with a different path binding".into(),
        ));
    }
    let request_object = request.as_object().ok_or_else(|| {
        RemoteError("remote session Runner receipt request is not an object".into())
    })?;
    let response_object = response.as_object().ok_or_else(|| {
        RemoteError("Forge API returned an invalid session Runner receipt".into())
    })?;
    for field in ["owner", "conversation_id", "prompt_id", "run_id"] {
        if response_object.get(field) != request_object.get(field) {
            return Err(RemoteError(format!(
                "Forge API returned a session Runner receipt with a different {field}"
            )));
        }
    }
    if response != request {
        return Err(RemoteError(
            "Forge API returned a session Runner receipt that differs from the canonical request"
                .into(),
        ));
    }
    Ok(())
}

pub(super) fn validate_value(value: &Value) -> Result<(), RemoteError> {
    let object = value.as_object().ok_or_else(|| {
        RemoteError("remote session Runner receipt observation must be a JSON object".into())
    })?;
    if !exact_fields(
        object,
        [
            "schema_version",
            "evaluation_mode",
            "owner",
            "conversation_id",
            "prompt_id",
            "run_id",
            "receipt_observation",
            "prompt_run_binding_valid",
            "receipt_binding_valid",
            "preview_only",
            "selected_target_id",
            "authority",
        ],
    ) {
        return Err(RemoteError(
            "remote session Runner receipt observation has an invalid shape".into(),
        ));
    }
    if object.get("schema_version").and_then(Value::as_str) != Some(SCHEMA_VERSION)
        || !object.get("selected_target_id").is_some_and(Value::is_null)
    {
        return Err(RemoteError(
            "remote session Runner receipt observation schema or target selection is invalid"
                .into(),
        ));
    }
    for field in [
        "schema_version",
        "evaluation_mode",
        "owner",
        "conversation_id",
        "prompt_id",
        "run_id",
        "receipt_observation",
        "prompt_run_binding_valid",
        "receipt_binding_valid",
        "preview_only",
        "authority",
    ] {
        if object.get(field).is_some_and(Value::is_null) {
            return Err(RemoteError(
                "remote session Runner receipt observation contains null metadata".into(),
            ));
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
    let receipt = object
        .get("receipt_observation")
        .ok_or_else(|| RemoteError("remote session Runner receipt metadata is missing".into()))?;
    let receipt_object = receipt
        .as_object()
        .ok_or_else(|| RemoteError("remote session Runner receipt metadata is invalid".into()))?;
    if !exact_fields(
        receipt_object,
        [
            "schema_version",
            "evaluation_mode",
            "command_id",
            "command_sha256",
            "attempt_id",
            "target_id",
            "disposition_kind",
            "observed_at_ms",
            "receipt_valid",
            "preview_only",
            "uncertain",
            "reconciliation_required",
            "manual_review_required",
            "automatic_retry",
            "follow_up",
            "authority",
        ],
    ) {
        return Err(RemoteError(
            "remote session Runner receipt metadata has an invalid shape".into(),
        ));
    }
    if receipt_object.values().any(Value::is_null) {
        return Err(RemoteError(
            "remote session Runner receipt metadata contains null fields".into(),
        ));
    }
    exact_nested_fields(
        receipt_object.get("authority"),
        [
            "device_identity_verified",
            "command_persisted",
            "reservation_created",
            "execution_authorized",
            "dispatch_performed",
            "audit_published",
        ],
    )?;
    let observation: SessionRunnerReceiptObservation = serde_json::from_value(value.clone())
        .map_err(|_| RemoteError("remote session Runner receipt observation is invalid".into()))?;
    observation
        .validate()
        .map_err(|error| RemoteError(error.to_string()))
}

pub(super) fn render_human(value: &Value, writer: &mut impl Write) -> io::Result<()> {
    let object = value
        .as_object()
        .expect("validated session Runner receipt observation object");
    let owner = object["owner"].as_object().expect("validated owner object");
    let receipt = object["receipt_observation"]
        .as_object()
        .expect("validated receipt object");
    writeln!(
        writer,
        "authenticated session Runner terminal receipt observation [{}]",
        object["schema_version"].as_str().unwrap_or_default()
    )?;
    writeln!(
        writer,
        "owner={} conversation={} prompt={} run={}",
        owner["subject"].as_str().unwrap_or_default(),
        object["conversation_id"].as_str().unwrap_or_default(),
        object["prompt_id"].as_str().unwrap_or_default(),
        object["run_id"].as_str().unwrap_or_default()
    )?;
    writeln!(
        writer,
        "receipt_command={} attempt={} target={} disposition={} observed_at_ms={} receipt_valid={} uncertain={}",
        receipt["command_id"].as_str().unwrap_or_default(),
        receipt["attempt_id"].as_str().unwrap_or_default(),
        receipt["target_id"].as_str().unwrap_or_default(),
        receipt["disposition_kind"].as_str().unwrap_or_default(),
        receipt["observed_at_ms"].as_u64().unwrap_or_default(),
        receipt["receipt_valid"].as_bool().unwrap_or(false),
        receipt["uncertain"].as_bool().unwrap_or(false)
    )?;
    writeln!(
        writer,
        "binding: prompt_run_binding_valid={} receipt_binding_valid={} preview_only={} selected_target=none",
        object["prompt_run_binding_valid"]
            .as_bool()
            .unwrap_or(false),
        object["receipt_binding_valid"].as_bool().unwrap_or(false),
        object["preview_only"].as_bool().unwrap_or(false)
    )?;
    writeln!(
        writer,
        "follow_up={} reconciliation_required={} manual_review_required={} automatic_retry={}",
        receipt["follow_up"].as_str().unwrap_or_default(),
        receipt["reconciliation_required"]
            .as_bool()
            .unwrap_or(false),
        receipt["manual_review_required"].as_bool().unwrap_or(false),
        receipt["automatic_retry"].as_bool().unwrap_or(false)
    )?;
    writeln!(
        writer,
        "authority: identity_verified=false receipt_persisted=false execution_authorized=false dispatch_performed=false audit_published=false"
    )
}

fn exact_fields<const N: usize>(object: &Map<String, Value>, fields: [&str; N]) -> bool {
    object.len() == N && fields.into_iter().all(|field| object.contains_key(field))
}

fn exact_nested_fields<const N: usize>(
    value: Option<&Value>,
    fields: [&str; N],
) -> Result<(), RemoteError> {
    let object = value.and_then(Value::as_object).ok_or_else(|| {
        RemoteError("remote session Runner receipt nested object is invalid".into())
    })?;
    if !exact_fields(object, fields) {
        return Err(RemoteError(
            "remote session Runner receipt nested object has an invalid shape".into(),
        ));
    }
    if object.values().any(Value::is_null) {
        return Err(RemoteError(
            "remote session Runner receipt nested object contains null fields".into(),
        ));
    }
    Ok(())
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, RemoteError> {
    let mut bytes = Vec::new();
    if input == "-" {
        io::stdin()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| {
                RemoteError("remote session Runner receipt input could not be read".into())
            })?;
    } else {
        File::open(Path::new(input))
            .map_err(|_| {
                RemoteError("remote session Runner receipt input could not be opened".into())
            })?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| {
                RemoteError("remote session Runner receipt input could not be read".into())
            })?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(RemoteError(format!(
            "remote session Runner receipt input exceeds {MAX_INPUT_BYTES} bytes"
        )));
    }
    Ok(bytes)
}
