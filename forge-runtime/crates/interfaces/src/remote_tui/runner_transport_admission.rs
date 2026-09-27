use std::io::Write;

use serde_json::Value;

use super::{
    RemoteClient, RemoteError,
    state::{TuiState, io_error},
};

/// Posts one owner/Run-bound transport admission preview for the selected
/// session. The response is metadata only and never renders transport or
/// fencing material.
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
            "Runner transport admission requires the selected session to match conversation {conversation_id}."
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
    let Some(suffix) = argument.strip_prefix("--input") else {
        return report_usage(writer);
    };
    if !suffix.chars().next().is_some_and(char::is_whitespace) {
        return report_usage(writer);
    }
    let input = suffix.trim();
    if input.is_empty() || input == "-" {
        return report_usage(writer);
    }
    let request = match super::super::runner_transport_admission::read_tui_request(input) {
        Ok(request) => request,
        Err(error) => return report_input_error(writer, error),
    };
    let (conversation_id, run_id) =
        match super::super::runner_transport_admission::conversation_and_run(&request) {
            Ok(ids) => ids,
            Err(error) => return report_input_error(writer, error),
        };
    Ok(Some((request, conversation_id, run_id)))
}

fn report_usage<W: Write>(writer: &mut W) -> Result<Option<(Value, String, String)>, RemoteError> {
    usage(writer)?;
    Ok(None)
}

fn report_input_error<W: Write>(
    writer: &mut W,
    error: RemoteError,
) -> Result<Option<(Value, String, String)>, RemoteError> {
    writeln!(writer, "Runner transport admission input failed: {error}").map_err(io_error)?;
    Ok(None)
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
        "Runner transport admission",
        writer,
    )
    .await?
    {
        return Ok(false);
    }
    if !ensure_visibility(state, conversation_id, writer)? {
        return Ok(false);
    }
    if state.client_instance_filter.is_some()
        && !super::writes::ensure_inventory_resource_converged(
            state,
            "Runner transport admission",
            writer,
        )?
    {
        return Ok(false);
    }
    if state.client_instance_filter.is_some() {
        let target_id = super::super::runner_transport_admission::target_id(request)?;
        return ensure_target_in_resource_view(state, target_id, writer);
    }
    Ok(true)
}

fn require_resource_pair<W: Write>(state: &TuiState, writer: &mut W) -> Result<bool, RemoteError> {
    if state.client_instance_filter.is_none()
        || (state.device_inventory_v2_observed.is_some()
            && state.client_instance_resource_view_observed.is_some())
    {
        return Ok(true);
    }
    writeln!(
        writer,
        "Runner transport admission blocked by selected client-instance resource projection: open converged inventory/resource observations first. No request was sent."
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
        "Runner transport admission blocked by selected client-instance resource projection: target {target_id:?} is absent from the current resource observation. No request was sent."
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
        .preview_runner_transport_admission(conversation_id, run_id, request)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            writeln!(writer, "Runner transport admission request failed: {error}")
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
    match super::super::runner_transport_admission::validate_response(
        &response,
        request,
        conversation_id,
        run_id,
    ) {
        Ok(()) => super::super::runner_transport_admission::render_human(&response, writer)
            .map_err(io_error)?,
        Err(error) => writeln!(
            writer,
            "Runner transport admission response failed validation: {error}"
        )
        .map_err(io_error)?,
    }
    Ok(())
}

fn usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(writer, "Use runner-transport-admission-remote-preview --input FILE. The request is a transport admission preview; '-' is reserved for the standalone CLI.").map_err(io_error)
}
