//! Strict authenticated transport for the injected Runner dispatch-plan
//! preview candidate.
//!
//! The input deliberately reuses the already bounded Run/Attempt/lease
//! preflight request.  Its `dispatch_plan` object is the only JSON body sent
//! to Core.  This keeps the candidate path bound to the same owner, Run, and
//! lease declarations while never turning the preview into dispatch
//! authority.

use std::{io, io::Write};

use serde_json::Value;

use super::{RemoteError, run_attempt_lease_dispatch_preflight as preflight};

pub(super) fn read_request(input: &str) -> Result<Value, RemoteError> {
    preflight::read_request(input)
}

pub(super) fn read_tui_request(input: &str) -> Result<Value, RemoteError> {
    preflight::read_tui_request(input)
}

pub(super) fn conversation_and_run(request: &Value) -> Result<(String, String), RemoteError> {
    preflight::conversation_and_run(request)
}

pub(super) fn dispatch_plan(request: &Value) -> Result<Value, RemoteError> {
    let object = request.as_object().ok_or_else(|| invalid_request())?;
    object
        .get("dispatch_plan")
        .filter(|value| value.is_object())
        .cloned()
        .ok_or_else(invalid_request)
}

pub(super) fn validate_response(
    value: &Value,
    request: &Value,
    conversation_id: &str,
    run_id: &str,
) -> Result<(), RemoteError> {
    let (request_conversation_id, request_run_id) = preflight::conversation_and_run(request)?;
    if request_conversation_id != conversation_id || request_run_id != run_id {
        return Err(RemoteError(
            "remote Runner dispatch-plan preview request does not match the URL path".into(),
        ));
    }
    crate::device_runner_dispatch_plan_command::validate_remote_response(value).map_err(|_| {
        RemoteError("Forge API returned an invalid Runner dispatch-plan preview".into())
    })?;

    let root = request.as_object().ok_or_else(|| invalid_request())?;
    let plan = root
        .get("dispatch_plan")
        .and_then(Value::as_object)
        .ok_or_else(invalid_request)?;
    let placement = plan
        .get("placement_request")
        .and_then(Value::as_object)
        .ok_or_else(invalid_request)?;
    let intent = plan
        .get("runner_execution_intent")
        .and_then(Value::as_object)
        .ok_or_else(invalid_request)?;
    let lease = plan
        .get("lease")
        .and_then(Value::as_object)
        .ok_or_else(invalid_request)?;
    let response = value.as_object().ok_or_else(|| {
        RemoteError("Forge API returned an invalid Runner dispatch-plan preview".into())
    })?;

    for (response_field, expected) in [
        ("owner_declaration", intent.get("owner")),
        ("conversation_id", intent.get("conversation_id")),
        ("run_id", intent.get("run_id")),
        ("attempt_id", intent.get("attempt_id")),
        ("attempt_state", plan.get("attempt_state")),
        ("command_id", intent.get("command_id")),
        ("command_sha256", intent.get("command_sha256")),
        ("intent_target_id", intent.get("target_id")),
        ("lease_epoch", lease.get("epoch")),
        ("evaluated_at_ms", placement.get("evaluated_at_ms")),
    ] {
        if response.get(response_field) != expected {
            return Err(RemoteError(format!(
                "Forge API returned a Runner dispatch-plan preview with a different {response_field}"
            )));
        }
    }
    if response.get("candidate_count").and_then(Value::as_u64)
        != placement
            .get("devices")
            .and_then(Value::as_array)
            .map(|devices| devices.len() as u64)
    {
        return Err(RemoteError(
            "Forge API returned a Runner dispatch-plan preview with a different candidate count"
                .into(),
        ));
    }
    let evaluated_at_ms = placement
        .get("evaluated_at_ms")
        .and_then(Value::as_u64)
        .ok_or_else(invalid_request)?;
    let issued_at_ms = lease
        .get("issued_at_ms")
        .and_then(Value::as_u64)
        .ok_or_else(invalid_request)?;
    let expires_at_ms = lease
        .get("expires_at_ms")
        .and_then(Value::as_u64)
        .ok_or_else(invalid_request)?;
    let expected_lease_active = evaluated_at_ms >= issued_at_ms && evaluated_at_ms < expires_at_ms;
    if response.get("lease_active") != Some(&Value::Bool(expected_lease_active)) {
        return Err(RemoteError(
            "Forge API returned a Runner dispatch-plan preview with a different lease state".into(),
        ));
    }
    Ok(())
}

pub(super) fn render_human(value: &Value, writer: &mut impl Write) -> io::Result<()> {
    crate::device_runner_dispatch_plan_command::write_remote_output(value, writer)
}

fn invalid_request() -> RemoteError {
    RemoteError("remote Runner dispatch-plan preview input is invalid".into())
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{
        conversation_and_run, dispatch_plan, read_request, render_human, validate_response,
    };

    fn request_path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../../docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-request-v1.json",
        )
    }

    fn response() -> Value {
        serde_json::from_str(include_str!(
            "../../../../docs/contracts/fixtures/forge-runner-dispatch-plan-preview-v1.json"
        ))
        .expect("Runner dispatch-plan preview fixture")
    }

    fn response_for(request: &Value) -> Value {
        let mut response = response();
        response["command_sha256"] =
            request["dispatch_plan"]["runner_execution_intent"]["command_sha256"].clone();
        response
    }

    #[test]
    fn request_reuses_strict_preflight_and_extracts_the_bound_plan() {
        let request = read_request(request_path().to_str().unwrap()).unwrap();
        assert_eq!(
            conversation_and_run(&request).unwrap(),
            ("conversation-001".into(), "run-001".into())
        );
        let plan = dispatch_plan(&request).unwrap();
        assert_eq!(plan["attempt_state"], json!("accepted"));
        assert_eq!(plan["lease"]["target_id"], json!("runner-1"));
    }

    #[test]
    fn response_requires_request_binding_and_false_authority() {
        let request = read_request(request_path().to_str().unwrap()).unwrap();
        validate_response(
            &response_for(&request),
            &request,
            "conversation-001",
            "run-001",
        )
        .unwrap();

        for (field, replacement) in [
            ("run_id", json!("run-002")),
            (
                "owner_declaration",
                json!({"issuer":"https://id.example","subject":"other","tenant_id":"tenant-1"}),
            ),
            ("selected_target_id", json!("runner-1")),
        ] {
            let mut mutated = response_for(&request);
            mutated[field] = replacement;
            assert!(validate_response(&mutated, &request, "conversation-001", "run-001").is_err());
        }

        let mut authority = response();
        authority["authority"]["dispatch_performed"] = json!(true);
        assert!(validate_response(&authority, &request, "conversation-001", "run-001").is_err());
    }

    #[test]
    fn human_output_is_metadata_only() {
        let mut output = Vec::new();
        render_human(&response(), &mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("offline Runner dispatch-plan preview"));
        assert!(output.contains("selected_target=none"));
        assert!(output.contains("dispatch_performed=false"));
        assert!(!output.contains("fence-001"));
    }
}
