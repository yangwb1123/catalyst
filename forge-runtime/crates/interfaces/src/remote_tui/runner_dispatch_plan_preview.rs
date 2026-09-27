use std::io::Write;

use serde_json::Value;

use super::{
    RemoteClient, RemoteError,
    state::{TuiState, io_error},
};

/// Posts one bounded Runner dispatch-plan declaration to the authenticated
/// candidate after binding it to the selected owner-scoped session.
pub(super) async fn remote_preview<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some((request, conversation_id, run_id, dispatch_plan)) =
        read_remote_request(argument, writer)?
    else {
        return Ok(());
    };
    if state.selected_id.as_deref() != Some(conversation_id.as_str()) {
        writeln!(
            writer,
            "Runner dispatch-plan preview requires the selected session to match conversation {conversation_id}."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    if !ensure_dispatch_plan_boundary(client, state, &conversation_id, &dispatch_plan, writer)
        .await?
    {
        return Ok(());
    }
    post_and_render(
        client,
        state,
        &request,
        &conversation_id,
        &run_id,
        &dispatch_plan,
        writer,
    )
    .await
}

fn read_remote_request<W: Write>(
    argument: &str,
    writer: &mut W,
) -> Result<Option<(Value, String, String, Value)>, RemoteError> {
    let Some(suffix) = argument.strip_prefix("--input") else {
        usage(writer)?;
        return Ok(None);
    };
    if !suffix.chars().next().is_some_and(char::is_whitespace) {
        usage(writer)?;
        return Ok(None);
    }
    let input = suffix.trim();
    if input.is_empty() || input == "-" {
        usage(writer)?;
        return Ok(None);
    }
    let request = match super::super::runner_dispatch_plan_preview::read_tui_request(input) {
        Ok(request) => request,
        Err(error) => return report_input_error(writer, error),
    };
    let (conversation_id, run_id) =
        match super::super::runner_dispatch_plan_preview::conversation_and_run(&request) {
            Ok(ids) => ids,
            Err(error) => return report_input_error(writer, error),
        };
    let dispatch_plan = match super::super::runner_dispatch_plan_preview::dispatch_plan(&request) {
        Ok(plan) => plan,
        Err(error) => return report_input_error(writer, error),
    };
    Ok(Some((request, conversation_id, run_id, dispatch_plan)))
}

fn report_input_error<W: Write>(
    writer: &mut W,
    error: RemoteError,
) -> Result<Option<(Value, String, String, Value)>, RemoteError> {
    writeln!(writer, "Runner dispatch-plan preview input failed: {error}").map_err(io_error)?;
    Ok(None)
}

async fn ensure_dispatch_plan_boundary<W: Write>(
    client: &RemoteClient,
    state: &mut super::state::TuiState,
    conversation_id: &str,
    dispatch_plan: &Value,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    if !require_dispatch_resource_pair(state, writer)?
        || !ensure_dispatch_visibility(state, conversation_id, writer)?
    {
        return Ok(false);
    }
    if !super::writes::refresh_explicit_inventory_resource_observations(
        client,
        state,
        "Runner dispatch-plan preview",
        writer,
    )
    .await?
    {
        return Ok(false);
    }
    if !ensure_dispatch_visibility(state, conversation_id, writer)? {
        return Ok(false);
    }
    if state.client_instance_filter.is_some()
        && !super::writes::ensure_inventory_resource_converged(
            state,
            "Runner dispatch-plan preview",
            writer,
        )?
    {
        return Ok(false);
    }
    if state.client_instance_filter.is_some()
        && !ensure_dispatch_plan_targets_in_resource_view(state, dispatch_plan, writer)?
    {
        return Ok(false);
    }
    Ok(true)
}

fn require_dispatch_resource_pair<W: Write>(
    state: &super::state::TuiState,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    if state.client_instance_filter.is_none()
        || (state.device_inventory_v2_observed.is_some()
            && state.client_instance_resource_view_observed.is_some())
    {
        return Ok(true);
    }
    writeln!(
        writer,
        "Runner dispatch-plan preview blocked by selected client-instance resource projection: open converged inventory/resource observations first. No request was sent."
    )
    .map_err(io_error)?;
    Ok(false)
}

fn ensure_dispatch_visibility<W: Write>(
    state: &super::state::TuiState,
    conversation_id: &str,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    super::commands::ensure_conversation_visible_to_client_instance(state, conversation_id, writer)
}

async fn post_and_render<W: Write>(
    client: &RemoteClient,
    state: &mut super::state::TuiState,
    request: &Value,
    conversation_id: &str,
    run_id: &str,
    dispatch_plan: &Value,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let response = match client
        .preview_runner_dispatch_plan(conversation_id, run_id, dispatch_plan)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            writeln!(
                writer,
                "Runner dispatch-plan preview request failed: {error}"
            )
            .map_err(io_error)?;
            if cleared {
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(io_error)?;
            }
            return Ok(());
        }
    };
    match super::super::runner_dispatch_plan_preview::validate_response(
        &response,
        request,
        conversation_id,
        run_id,
    ) {
        Ok(()) => super::super::runner_dispatch_plan_preview::render_human(&response, writer)
            .map_err(io_error)?,
        Err(error) => {
            writeln!(
                writer,
                "Runner dispatch-plan preview response failed validation: {error}"
            )
            .map_err(io_error)?;
        }
    }
    Ok(())
}

/// Keeps a selected instance's dispatch-plan candidate behind the latest
/// owner-bound resource image. The candidate remains a planning-only POST;
/// this check only prevents stale or foreign target identities from escaping
/// the local instance projection.
fn ensure_dispatch_plan_targets_in_resource_view<W: Write>(
    state: &super::state::TuiState,
    dispatch_plan: &Value,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    let Some(resource_view) = state.client_instance_resource_view_observed.as_ref() else {
        return Ok(true);
    };
    let Some(resource_devices) = resource_view.get("devices").and_then(Value::as_array) else {
        writeln!(
            writer,
            "Runner dispatch-plan preview blocked by selected client-instance resource projection: devices are missing. No request was sent."
        )
        .map_err(super::state::io_error)?;
        return Ok(false);
    };
    let targets = dispatch_plan_target_ids(dispatch_plan)?;
    for target in targets {
        if resource_devices.iter().any(|device| {
            device.get("device_id").and_then(Value::as_str) == Some(target.as_str())
                || device.get("runner_instance_id").and_then(Value::as_str) == Some(target.as_str())
        }) {
            continue;
        }
        writeln!(
            writer,
            "Runner dispatch-plan preview blocked by selected client-instance resource projection: target {target:?} is absent from the current resource observation. No request was sent."
        )
        .map_err(super::state::io_error)?;
        return Ok(false);
    }
    Ok(true)
}

fn dispatch_plan_target_ids(dispatch_plan: &Value) -> Result<Vec<String>, RemoteError> {
    let object = dispatch_plan
        .as_object()
        .ok_or_else(|| RemoteError("Runner dispatch-plan preview input is invalid".into()))?;
    let intent = object
        .get("runner_execution_intent")
        .and_then(Value::as_object)
        .ok_or_else(|| RemoteError("Runner dispatch-plan preview input is invalid".into()))?;
    let lease = object
        .get("lease")
        .and_then(Value::as_object)
        .ok_or_else(|| RemoteError("Runner dispatch-plan preview input is invalid".into()))?;
    let intent_target = intent
        .get("target_id")
        .and_then(Value::as_str)
        .ok_or_else(|| RemoteError("Runner dispatch-plan preview input is invalid".into()))?;
    let lease_target = lease
        .get("target_id")
        .and_then(Value::as_str)
        .ok_or_else(|| RemoteError("Runner dispatch-plan preview input is invalid".into()))?;
    let mut targets = vec![intent_target.to_owned(), lease_target.to_owned()];
    if let Some(devices) = object
        .get("placement_request")
        .and_then(Value::as_object)
        .and_then(|placement| placement.get("devices"))
        .and_then(Value::as_array)
    {
        for device in devices {
            let target = device
                .get("device_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    RemoteError("Runner dispatch-plan preview input is invalid".into())
                })?;
            if !targets.iter().any(|known| known == target) {
                targets.push(target.to_owned());
            }
        }
    }
    Ok(targets)
}

fn usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use runner-dispatch-plan-remote-preview --input FILE. The input is a caller-supplied Run/Attempt/lease declaration; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}
