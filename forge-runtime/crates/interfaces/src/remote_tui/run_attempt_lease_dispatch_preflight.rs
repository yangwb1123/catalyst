use std::io::Write;

use super::{
    RemoteClient, RemoteError,
    state::{TuiState, io_error},
};

/// Shows one bounded Run/Attempt/lease preflight contract file locally. The
/// TUI accepts paths only so interactive stdin remains available for TUI
/// commands.
pub(super) fn preview<W: Write>(argument: &str, writer: &mut W) -> Result<(), RemoteError> {
    let Some(suffix) = argument.strip_prefix("--input") else {
        return usage(writer);
    };
    if !suffix.chars().next().is_some_and(char::is_whitespace) {
        return usage(writer);
    }
    let input = suffix.trim();
    if input.is_empty() || input == "-" {
        return usage(writer);
    }
    let command = crate::args::DeviceCommand::RunAttemptLeaseDispatchPreflightPreview {
        input: input.to_owned(),
    };
    match crate::device_run_attempt_lease_dispatch_preflight_command::execute(&command) {
        Ok(output) => crate::device_run_attempt_lease_dispatch_preflight_command::write_output(
            &output, false, writer,
        )
        .map_err(io_error)?,
        Err(error) => {
            writeln!(writer, "Run/Attempt/lease preflight failed: {error}").map_err(io_error)?;
        }
    }
    Ok(())
}

/// Posts one bounded Run/Attempt/lease declaration to the authenticated
/// private candidate after binding it to the selected owner-scoped session.
pub(super) async fn remote_preview<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(request) = read_request(argument, writer)? else {
        return Ok(());
    };
    let (conversation_id, run_id) =
        match super::super::run_attempt_lease_dispatch_preflight::conversation_and_run(&request) {
            Ok(ids) => ids,
            Err(error) => {
                writeln!(writer, "Run/Attempt/lease preflight input failed: {error}")
                    .map_err(io_error)?;
                return Ok(());
            }
        };
    if state.selected_id.as_deref() != Some(conversation_id.as_str()) {
        writeln!(
            writer,
            "Run/Attempt/lease preflight requires the selected session to match conversation {conversation_id}."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    if !super::commands::ensure_conversation_visible_to_client_instance(
        state,
        &conversation_id,
        writer,
    )? {
        return Ok(());
    }
    if !ensure_fresh_instance_projection(client, state, &conversation_id, writer).await? {
        return Ok(());
    }
    post_and_render(client, state, &request, &conversation_id, &run_id, writer).await
}

fn usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use run-attempt-lease-dispatch-preflight-preview --input FILE. The file is a caller-supplied offline declaration; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}

fn remote_usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use run-attempt-lease-dispatch-preflight-remote-preview --input FILE. The file is posted to the authenticated test-only candidate; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}

fn read_request<W: Write>(
    argument: &str,
    writer: &mut W,
) -> Result<Option<serde_json::Value>, RemoteError> {
    let Some(suffix) = argument.strip_prefix("--input") else {
        remote_usage(writer)?;
        return Ok(None);
    };
    if !suffix.chars().next().is_some_and(char::is_whitespace) {
        remote_usage(writer)?;
        return Ok(None);
    }
    let input = suffix.trim();
    if input.is_empty() || input == "-" {
        remote_usage(writer)?;
        return Ok(None);
    }
    let request = match super::super::run_attempt_lease_dispatch_preflight::read_tui_request(input)
    {
        Ok(request) => request,
        Err(error) => {
            writeln!(writer, "Run/Attempt/lease preflight input failed: {error}")
                .map_err(io_error)?;
            return Ok(None);
        }
    };
    Ok(Some(request))
}

async fn post_and_render<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    request: &serde_json::Value,
    conversation_id: &str,
    run_id: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let response = match client
        .preview_run_attempt_lease_dispatch_preflight(conversation_id, run_id, request)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            writeln!(
                writer,
                "Run/Attempt/lease preflight request failed: {error}"
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
    match super::super::run_attempt_lease_dispatch_preflight::validate_response(
        &response,
        request,
        conversation_id,
        run_id,
    ) {
        Ok(()) => {
            super::super::run_attempt_lease_dispatch_preflight::render_human(&response, writer)
                .map_err(io_error)?;
        }
        Err(error) => {
            writeln!(
                writer,
                "Run/Attempt/lease preflight response failed validation: {error}"
            )
            .map_err(io_error)?;
        }
    }
    Ok(())
}

async fn ensure_fresh_instance_projection<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    conversation_id: &str,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    // Refresh the explicit inventory/resource pair at the candidate boundary
    // just like the scheduler and other storage-only writes. A changed pair
    // may revoke the selected Conversation, so visibility is checked again
    // before the existing convergence guard and candidate POST.
    if !super::writes::refresh_explicit_inventory_resource_observations(
        client,
        state,
        "Run/Attempt/lease preflight",
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
    )? {
        return Ok(false);
    }
    if !super::writes::ensure_inventory_resource_converged(
        state,
        "Run/Attempt/lease preflight",
        writer,
    )? {
        return Ok(false);
    }
    Ok(true)
}
