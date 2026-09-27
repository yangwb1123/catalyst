use std::io::Write;

use serde_json::Value;

use super::{
    RemoteClient, RemoteError,
    state::{TuiState, io_error},
};

/// Posts one bounded local Runner execution-readiness preview after requiring
/// the input Conversation to be the selected owner-scoped session.
pub(super) async fn preview<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(request) = read_request(argument, writer)? else {
        return Ok(());
    };
    let Some((conversation_id, intent_id)) = read_binding(&request, writer)? else {
        return Ok(());
    };
    if !super::commands::ensure_conversation_visible_to_client_instance(
        state,
        &conversation_id,
        writer,
    )? {
        return Ok(());
    }
    if state.selected_id.as_deref() != Some(conversation_id.as_str()) {
        writeln!(
            writer,
            "Local Runner preview requires the selected session to match conversation {conversation_id}."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    if !ensure_instance_boundary(client, state, &conversation_id, &request, writer).await? {
        return Ok(());
    }
    request_and_render(
        client,
        state,
        &conversation_id,
        &intent_id,
        &request,
        writer,
    )
    .await
}

fn read_request<W: Write>(argument: &str, writer: &mut W) -> Result<Option<Value>, RemoteError> {
    let Some(input) = argument.strip_prefix("--input") else {
        write_remote_usage(writer)?;
        return Ok(None);
    };
    if !input.chars().next().is_some_and(char::is_whitespace) {
        write_remote_usage(writer)?;
        return Ok(None);
    }
    let input = input.trim();
    if input.is_empty() || input == "-" {
        write_remote_usage(writer)?;
        return Ok(None);
    }
    match super::super::local_runner_preview::read_tui_request(input) {
        Ok(request) => Ok(Some(request)),
        Err(error) => {
            writeln!(writer, "Local Runner preview input failed: {error}").map_err(io_error)?;
            Ok(None)
        }
    }
}

fn read_binding<W: Write>(
    request: &Value,
    writer: &mut W,
) -> Result<Option<(String, String)>, RemoteError> {
    match super::super::local_runner_preview::conversation_and_intent(request) {
        Ok((conversation_id, intent_id)) => {
            Ok(Some((conversation_id.to_owned(), intent_id.to_owned())))
        }
        Err(error) => {
            writeln!(writer, "Local Runner preview input failed: {error}").map_err(io_error)?;
            Ok(None)
        }
    }
}

async fn ensure_instance_boundary<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    conversation_id: &str,
    request: &Value,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    if state.client_instance_filter.is_none() {
        return Ok(true);
    }
    if state.device_inventory_v2_observed.is_none()
        || state.client_instance_resource_view_observed.is_none()
    {
        writeln!(
            writer,
            "Local Runner preview blocked by selected client-instance resource projection: open converged inventory/resource observations first. No request was sent."
        )
        .map_err(io_error)?;
        return Ok(false);
    }
    if !super::writes::refresh_explicit_inventory_resource_observations(
        client,
        state,
        "Local Runner preview",
        writer,
    )
    .await?
    {
        return Ok(false);
    }
    if !super::commands::ensure_conversation_visible_to_client_instance(
        state,
        conversation_id,
        writer,
    )? || !super::writes::ensure_inventory_resource_converged(
        state,
        "Local Runner preview",
        writer,
    )? {
        return Ok(false);
    }
    let target_id = match super::super::local_runner_preview::target_id(request) {
        Ok(target_id) => target_id,
        Err(error) => {
            writeln!(writer, "Local Runner preview input failed: {error}").map_err(io_error)?;
            return Ok(false);
        }
    };
    ensure_target_in_resource_view(state, target_id, writer)
}

async fn request_and_render<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    conversation_id: &str,
    intent_id: &str,
    request: &Value,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let response = match client
        .preview_local_runner_execution_readiness(conversation_id, intent_id, request)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            writeln!(writer, "Local Runner preview request failed: {error}").map_err(io_error)?;
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
    render_response(&response, request, conversation_id, intent_id, writer)
}

fn render_response<W: Write>(
    response: &Value,
    request: &Value,
    conversation_id: &str,
    intent_id: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    match super::super::local_runner_preview::validate_response(
        response,
        request,
        conversation_id,
        intent_id,
    ) {
        Ok(()) => {
            super::super::local_runner_preview::render_human(response, writer).map_err(io_error)
        }
        Err(error) => writeln!(
            writer,
            "Local Runner preview response failed validation: {error}"
        )
        .map_err(io_error),
    }
}

fn ensure_target_in_resource_view<W: Write>(
    state: &TuiState,
    target_id: &str,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    let target_observed = state
        .client_instance_resource_view_observed
        .as_ref()
        .and_then(|resource_view| resource_view.get("devices"))
        .and_then(Value::as_array)
        .is_some_and(|devices| {
            devices.iter().any(|device| {
                device.get("runner_instance_id").and_then(Value::as_str) == Some(target_id)
                    || device.get("device_id").and_then(Value::as_str) == Some(target_id)
            })
        });
    if target_observed {
        return Ok(true);
    }
    writeln!(
        writer,
        "Local Runner preview blocked by selected client-instance resource projection: target {target_id:?} is absent from the current resource observation. No request was sent."
    )
    .map_err(io_error)?;
    Ok(false)
}

fn write_remote_usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use runner-execution-readiness-preview --input FILE. The file is posted to the authenticated test-only local Runner preview route; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}
