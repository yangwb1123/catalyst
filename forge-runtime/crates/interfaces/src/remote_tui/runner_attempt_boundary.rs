use std::io::Write;

use super::{RemoteError, state::io_error};

/// Shows one bounded Runner Attempt lifecycle observation locally. The TUI
/// accepts a path only so interactive stdin remains available for commands.
/// The shared device consumer performs strict decoding and validation; this
/// surface only renders its redacted, preview-only result.
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
    let command = crate::args::DeviceCommand::RunnerAttemptBoundaryPreview {
        input: input.to_owned(),
    };
    match crate::device_runner_attempt_boundary_command::execute(&command) {
        Ok(output) => {
            crate::device_runner_attempt_boundary_command::write_output(&output, false, writer)
                .map_err(io_error)?
        }
        Err(error) => {
            writeln!(writer, "Runner Attempt boundary preview failed: {error}").map_err(io_error)?
        }
    }
    Ok(())
}

fn usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use runner-attempt-boundary-preview --input FILE. The file is a caller-supplied offline declaration; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}
