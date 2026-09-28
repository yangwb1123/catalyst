use std::io::Write;

use super::{
    RemoteClient, RemoteError, runs,
    state::{TuiState, io_error},
};

/// Shows one bounded local Run observation contract file. The TUI accepts
/// paths only so its interactive stdin remains available.
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
    let command = crate::args::DeviceCommand::RunObservedPreview {
        input: input.to_owned(),
    };
    match crate::device_run_observed_command::execute(&command) {
        Ok(output) => crate::device_run_observed_command::write_output(&output, false, writer)
            .map_err(io_error)?,
        Err(error) => writeln!(writer, "Run observed preview failed: {error}").map_err(io_error)?,
    }
    Ok(())
}

/// Reads one `RunObserved` projection through the authenticated owner-bound
/// candidate. The Conversation is always taken from the selected session so
/// this command cannot silently cross session boundaries.
pub(super) async fn show<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(conversation_id) = runs::selected_conversation_id(state, writer)?.map(str::to_owned)
    else {
        return Ok(());
    };
    if !super::commands::ensure_conversation_visible_to_client_instance(
        state,
        &conversation_id,
        writer,
    )? {
        return Ok(());
    }
    let Some(run_id) = runs::parse_run_id_argument(argument.trim()) else {
        return usage(writer);
    };
    let observation = match client.read_run_observation(&conversation_id, &run_id).await {
        Ok(observation) => observation,
        Err(error) => {
            report_read_failure(state, &conversation_id, &error, writer)?;
            return Ok(());
        }
    };
    state.selected_run_id = Some(run_id);
    state.selected_run_observed = Some(observation.clone());
    crate::device_run_observed_command::write_remote_output(&observation, writer).map_err(io_error)
}

fn usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use run-observed RUN_ID with the selected session, or run-observed-preview --input FILE for a caller-supplied offline declaration; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}

fn report_read_failure<W: Write>(
    state: &mut TuiState,
    conversation_id: &str,
    error: &RemoteError,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let cleared = super::clear_session_view_after_authorization_error(state, error);
    let dropped = if cleared {
        false
    } else {
        super::commands::drop_selected_session_after_read_rejection(state, conversation_id, error)
    };
    writeln!(writer, "Run observed request failed: {error}").map_err(io_error)?;
    if cleared {
        writeln!(
            writer,
            "Local session view cleared after authorization failure."
        )
        .map_err(io_error)?;
    } else if dropped {
        writeln!(
            writer,
            "Selected session was removed after its owner Run read was rejected."
        )
        .map_err(io_error)?;
    }
    Ok(())
}
