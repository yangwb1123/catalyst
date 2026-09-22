use std::io::Write;

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
    let request = match super::super::runner_dispatch_plan_preview::read_tui_request(input) {
        Ok(request) => request,
        Err(error) => {
            writeln!(writer, "Runner dispatch-plan preview input failed: {error}")
                .map_err(io_error)?;
            return Ok(());
        }
    };
    let (conversation_id, run_id) =
        match super::super::runner_dispatch_plan_preview::conversation_and_run(&request) {
            Ok(ids) => ids,
            Err(error) => {
                writeln!(writer, "Runner dispatch-plan preview input failed: {error}")
                    .map_err(io_error)?;
                return Ok(());
            }
        };
    if state.selected_id.as_deref() != Some(conversation_id.as_str()) {
        writeln!(
            writer,
            "Runner dispatch-plan preview requires the selected session to match conversation {conversation_id}."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    let dispatch_plan = match super::super::runner_dispatch_plan_preview::dispatch_plan(&request) {
        Ok(plan) => plan,
        Err(error) => {
            writeln!(writer, "Runner dispatch-plan preview input failed: {error}")
                .map_err(io_error)?;
            return Ok(());
        }
    };
    let response = match client
        .preview_runner_dispatch_plan(&conversation_id, &run_id, &dispatch_plan)
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
        &request,
        &conversation_id,
        &run_id,
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

fn usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use runner-dispatch-plan-remote-preview --input FILE. The input is a caller-supplied Run/Attempt/lease declaration; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}
