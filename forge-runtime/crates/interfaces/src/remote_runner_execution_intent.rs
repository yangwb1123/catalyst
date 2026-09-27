//! Strict Runtime consumer for the authenticated Runner execution-intent
//! preview. The request is a value-only binding and the response is accepted
//! only when it is exactly the pure, all-false-authority observation derived
//! from that request.

use std::{
    fs::File,
    io::{self, Read, Write},
};

use forge_runtime_domain::execution::runner_execution_intent::{
    RunnerExecutionIntentRequest, observe_runner_execution_intent,
};
use serde_json::Value;

use super::RemoteError;

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;

pub(super) fn read_request(input: &str) -> Result<Value, RemoteError> {
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes).map_err(|_| {
        RemoteError("remote Runner execution-intent input contains duplicate JSON keys".into())
    })?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| RemoteError("remote Runner execution-intent input is invalid JSON".into()))?;
    validate_request(&value)?;
    Ok(value)
}

pub(super) fn read_tui_request(input: &str) -> Result<Value, RemoteError> {
    if input == "-" {
        return Err(RemoteError(
            "remote TUI Runner execution-intent preview requires a file path; '-' belongs to the standalone CLI".into(),
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
        .get("run_reference")
        .and_then(|run| run.get("run_id"))
        .and_then(Value::as_str)
        .ok_or_else(invalid_request)?;
    Ok((conversation_id.to_owned(), run_id.to_owned()))
}

pub(super) fn validate_request(value: &Value) -> Result<(), RemoteError> {
    let object = value.as_object().ok_or_else(invalid_request)?;
    if object.len() != 6
        || ![
            "owner",
            "conversation_id",
            "prompt_receipt",
            "run_reference",
            "execution_intent",
            "command",
        ]
        .iter()
        .all(|field| object.contains_key(*field))
    {
        return Err(invalid_request());
    }
    let request: RunnerExecutionIntentRequest =
        serde_json::from_value(value.clone()).map_err(|_| invalid_request())?;
    observe_runner_execution_intent(request).map_err(|_| invalid_request())?;
    Ok(())
}

pub(super) fn validate_response(
    value: &Value,
    request: &Value,
    conversation_id: &str,
    run_id: &str,
) -> Result<(), RemoteError> {
    validate_request(request)?;
    let (request_conversation_id, request_run_id) = conversation_and_run(request)?;
    if request_conversation_id != conversation_id || request_run_id != run_id {
        return Err(RemoteError(
            "remote Runner execution-intent preview request does not match the URL path".into(),
        ));
    }
    let typed_request: RunnerExecutionIntentRequest =
        serde_json::from_value(request.clone()).map_err(|_| invalid_request())?;
    let expected = observe_runner_execution_intent(typed_request).map_err(|_| invalid_request())?;
    let expected_value = serde_json::to_value(expected).map_err(|_| invalid_response())?;
    if value != &expected_value {
        return Err(RemoteError(
            "Forge API returned an invalid Runner execution-intent preview".into(),
        ));
    }
    Ok(())
}

pub(super) fn render_human(value: &Value, writer: &mut impl Write) -> io::Result<()> {
    let object = value.as_object().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "invalid Runner intent response")
    })?;
    writeln!(writer, "remote Runner execution intent preview")?;
    writeln!(
        writer,
        "owner={} conversation={} prompt={} run={} attempt={} command={} target={}",
        object
            .get("owner")
            .and_then(|owner| owner.get("subject"))
            .and_then(Value::as_str)
            .unwrap_or(""),
        object
            .get("conversation_id")
            .and_then(Value::as_str)
            .unwrap_or(""),
        object
            .get("prompt_id")
            .and_then(Value::as_str)
            .unwrap_or(""),
        object.get("run_id").and_then(Value::as_str).unwrap_or(""),
        object
            .get("attempt_id")
            .and_then(Value::as_str)
            .unwrap_or(""),
        object
            .get("command_id")
            .and_then(Value::as_str)
            .unwrap_or(""),
        object
            .get("target_id")
            .and_then(Value::as_str)
            .unwrap_or(""),
    )?;
    writeln!(
        writer,
        "binding: prompt_run_binding_valid={} runner_command_binding_valid={} preview_only={} selected_target=none",
        object
            .get("prompt_run_binding_valid")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        object
            .get("runner_command_binding_valid")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        object
            .get("preview_only")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    )?;
    writeln!(
        writer,
        "authority: device_identity_verified=false command_persisted=false reservation_created=false execution_authorized=false dispatch_performed=false audit_published=false"
    )
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, RemoteError> {
    let mut bytes = Vec::new();
    if input == "-" {
        io::stdin()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("could not read Runner execution-intent input".into()))?;
    } else {
        File::open(input)
            .map_err(|_| RemoteError("could not open Runner execution-intent input".into()))?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("could not read Runner execution-intent input".into()))?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(RemoteError(
            "Runner execution-intent input exceeds the size limit".into(),
        ));
    }
    Ok(bytes)
}

fn invalid_request() -> RemoteError {
    RemoteError("remote Runner execution-intent input is invalid".into())
}

fn invalid_response() -> RemoteError {
    RemoteError("Forge API returned an invalid Runner execution-intent preview".into())
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{conversation_and_run, read_request, render_human, validate_response};

    fn request_path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/contracts/fixtures/forge-runner-execution-intent-request-v1.json")
    }

    fn response() -> Value {
        let request: forge_runtime_domain::execution::runner_execution_intent::RunnerExecutionIntentRequest =
            serde_json::from_str(include_str!(
                "../../../../docs/contracts/fixtures/forge-runner-execution-intent-request-v1.json"
            ))
            .expect("Runner execution-intent request fixture");
        serde_json::to_value(
            forge_runtime_domain::execution::runner_execution_intent::observe_runner_execution_intent(request)
                .expect("Runner execution-intent observation"),
        )
        .unwrap()
    }

    #[test]
    fn request_is_strictly_bound_to_conversation_and_run() {
        let request = read_request(request_path().to_str().unwrap()).unwrap();
        assert_eq!(
            conversation_and_run(&request).unwrap(),
            ("conversation-001".into(), "run-001".into())
        );
        validate_response(&response(), &request, "conversation-001", "run-001").unwrap();
        let mut foreign = response();
        foreign["run_id"] = json!("run-002");
        assert!(validate_response(&foreign, &request, "conversation-001", "run-001").is_err());
    }

    #[test]
    fn human_output_never_contains_command_arguments_or_fence() {
        let mut output = Vec::new();
        render_human(&response(), &mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("remote Runner execution intent preview"));
        assert!(output.contains("selected_target=none"));
        assert!(!output.contains("fence-001"));
        assert!(!output.contains("forge-task"));
    }
}
