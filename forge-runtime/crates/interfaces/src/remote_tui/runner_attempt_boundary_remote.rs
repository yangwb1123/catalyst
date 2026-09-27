use std::io::Write;

use serde_json::Value;

use super::{
    RemoteClient, RemoteError,
    state::{TuiState, io_error},
};

/// Posts one selected-session-bound Attempt lifecycle preview. The TUI keeps
/// the file-only input boundary so its interactive stdin remains available.
pub(super) async fn remote_preview<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some((request, conversation_id, run_id)) = read_request(argument, writer)? else {
        return Ok(());
    };
    if state.selected_id.as_deref() != Some(conversation_id.as_str()) {
        writeln!(
            writer,
            "Runner Attempt boundary requires the selected session to match conversation {conversation_id}."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    if !ensure_instance_boundary(client, state, &conversation_id, &request, writer).await? {
        return Ok(());
    }
    post_and_render(client, state, &request, &conversation_id, &run_id, writer).await
}

fn read_request<W: Write>(
    argument: &str,
    writer: &mut W,
) -> Result<Option<(Value, String, String)>, RemoteError> {
    let Some(input) = input_path(argument) else {
        usage(writer)?;
        return Ok(None);
    };
    let request = match super::super::runner_attempt_boundary::read_tui_request(input) {
        Ok(request) => request,
        Err(error) => return report_input_error(writer, error),
    };
    let (conversation_id, run_id) =
        match super::super::runner_attempt_boundary::conversation_and_run(&request) {
            Ok(ids) => ids,
            Err(error) => return report_input_error(writer, error),
        };
    Ok(Some((request, conversation_id, run_id)))
}

async fn ensure_instance_boundary<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    conversation_id: &str,
    request: &Value,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    if !require_resource_pair(state, writer)? || !ensure_visibility(state, conversation_id, writer)?
    {
        return Ok(false);
    }
    if !super::writes::refresh_explicit_inventory_resource_observations(
        client,
        state,
        "Runner Attempt boundary",
        writer,
    )
    .await?
        || !ensure_visibility(state, conversation_id, writer)?
    {
        return Ok(false);
    }
    if state.client_instance_filter.is_none() {
        return Ok(true);
    }
    if !super::writes::ensure_inventory_resource_converged(
        state,
        "Runner Attempt boundary",
        writer,
    )? {
        return Ok(false);
    }
    let target_id = super::super::runner_attempt_boundary::target_id(request)?;
    ensure_target_in_resource_view(state, target_id, writer)
}

fn require_resource_pair<W: Write>(state: &TuiState, writer: &mut W) -> Result<bool, RemoteError> {
    if state.client_instance_filter.is_none()
        || (state.client_instance_session_view_observed.is_some()
            && state.client_instance_resource_view_observed.is_some()
            && state.device_inventory_v2_observed.is_some())
    {
        return Ok(true);
    }
    writeln!(
        writer,
        "Runner Attempt boundary blocked by selected client-instance resource projection: open converged session/resource and inventory/resource observations first. No request was sent."
    )
    .map_err(io_error)?;
    Ok(false)
}

fn ensure_visibility<W: Write>(
    state: &TuiState,
    conversation_id: &str,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    super::commands::ensure_conversation_visible_to_client_instance(state, conversation_id, writer)
}

fn ensure_target_in_resource_view<W: Write>(
    state: &TuiState,
    target_id: &str,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    let target_observed = state
        .client_instance_resource_view_observed
        .as_ref()
        .and_then(|resource| resource.get("devices"))
        .and_then(Value::as_array)
        .is_some_and(|devices| {
            devices.iter().any(|device| {
                device.get("device_id").and_then(Value::as_str) == Some(target_id)
                    || device.get("runner_instance_id").and_then(Value::as_str) == Some(target_id)
            })
        });
    if target_observed {
        return Ok(true);
    }
    writeln!(
        writer,
        "Runner Attempt boundary blocked by selected client-instance resource projection: target {target_id:?} is absent from the current resource observation. No request was sent."
    )
    .map_err(io_error)?;
    Ok(false)
}

async fn post_and_render<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    request: &Value,
    conversation_id: &str,
    run_id: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let response = match client
        .preview_runner_attempt_boundary(conversation_id, run_id, request)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            writeln!(writer, "Runner Attempt boundary request failed: {error}")
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
    match super::super::runner_attempt_boundary::validate_response(
        &response,
        request,
        conversation_id,
        run_id,
    ) {
        Ok(()) => super::super::runner_attempt_boundary::render_human(&response, writer)
            .map_err(io_error)?,
        Err(error) => writeln!(
            writer,
            "Runner Attempt boundary response failed validation: {error}"
        )
        .map_err(io_error)?,
    }
    Ok(())
}

fn input_path(argument: &str) -> Option<&str> {
    let suffix = argument.strip_prefix("--input")?;
    if !suffix.chars().next().is_some_and(char::is_whitespace) {
        return None;
    }
    let input = suffix.trim();
    (!input.is_empty() && input != "-").then_some(input)
}

fn usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use runner-attempt-boundary-remote-preview --input FILE. The request is metadata-only; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}

fn report_input_error<W: Write>(
    writer: &mut W,
    error: RemoteError,
) -> Result<Option<(Value, String, String)>, RemoteError> {
    writeln!(writer, "Runner Attempt boundary input failed: {error}").map_err(io_error)?;
    Ok(None)
}
