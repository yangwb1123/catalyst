use std::io::Write;

use super::{RemoteError, state::io_error};

/// Shows one bounded Run execution-evidence contract file locally. The TUI
/// accepts paths only so its interactive stdin remains available.
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
    let command = crate::args::DeviceCommand::RunExecutionEvidencePreview {
        input: input.to_owned(),
    };
    match crate::device_run_execution_evidence_command::execute(&command) {
        Ok(output) => {
            crate::device_run_execution_evidence_command::write_output(&output, false, writer)
                .map_err(io_error)?
        }
        Err(error) => {
            writeln!(writer, "Run execution-evidence preview failed: {error}").map_err(io_error)?
        }
    }
    Ok(())
}

/// Posts one bounded pair of content-free observations to the authenticated
/// Run evidence candidate after binding it to the selected session.
pub(super) async fn remote_preview<W: Write>(
    client: &super::RemoteClient,
    state: &mut super::state::TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(suffix) = argument.strip_prefix("--input") else {
        return remote_usage(writer);
    };
    if !suffix.chars().next().is_some_and(char::is_whitespace) {
        return remote_usage(writer);
    }
    let input = suffix.trim();
    if input.is_empty() || input == "-" {
        return remote_usage(writer);
    }
    let request = match super::super::run_execution_evidence::read_tui_request(input) {
        Ok(request) => request,
        Err(error) => {
            writeln!(writer, "Run execution evidence input failed: {error}")
                .map_err(super::state::io_error)?;
            return Ok(());
        }
    };
    let (conversation_id, run_id) =
        match super::super::run_execution_evidence::conversation_and_run(&request) {
            Ok(ids) => ids,
            Err(error) => {
                writeln!(writer, "Run execution evidence input failed: {error}")
                    .map_err(super::state::io_error)?;
                return Ok(());
            }
        };
    if state.selected_id.as_deref() != Some(conversation_id.as_str()) {
        writeln!(
            writer,
            "Run execution evidence preview requires the selected session to match conversation {conversation_id}."
        )
        .map_err(super::state::io_error)?;
        return Ok(());
    }
    if !super::commands::ensure_conversation_visible_to_client_instance(
        state,
        &conversation_id,
        writer,
    )? {
        return Ok(());
    }
    let response = match client
        .preview_run_execution_evidence(&conversation_id, &run_id, &request)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            writeln!(writer, "Run execution evidence request failed: {error}")
                .map_err(super::state::io_error)?;
            if cleared {
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(super::state::io_error)?;
            }
            return Ok(());
        }
    };
    match super::super::run_execution_evidence::validate_response(
        &response,
        &request,
        &conversation_id,
        &run_id,
    ) {
        Ok(()) => super::super::run_execution_evidence::render_human(&response, writer)
            .map_err(super::state::io_error)?,
        Err(error) => {
            writeln!(
                writer,
                "Run execution evidence response failed validation: {error}"
            )
            .map_err(super::state::io_error)?;
        }
    }
    Ok(())
}

fn usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use run-execution-evidence-preview --input FILE. The file is a caller-supplied offline declaration; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}

fn remote_usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use run-execution-evidence-remote-preview --input FILE. The pair is posted to the authenticated Run evidence route; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}
