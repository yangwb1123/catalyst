use std::io::Write;

use super::state::io_error;
use crate::args::DeviceCommand;

/// Shows one bounded pending Run-intent contract file locally. The TUI accepts
/// a path only so interactive stdin remains available for TUI commands.
pub(super) fn preview<W: Write>(argument: &str, writer: &mut W) -> Result<(), super::RemoteError> {
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
    let command = DeviceCommand::PendingRunIntentPreview {
        input: input.to_owned(),
    };
    match crate::device_pending_run_intent_command::execute(&command) {
        Ok(output) => {
            crate::device_pending_run_intent_command::write_output(&output, false, writer)
                .map_err(io_error)?
        }
        Err(error) => {
            writeln!(writer, "Pending Run-intent preview failed: {error}").map_err(io_error)?
        }
    }
    Ok(())
}

fn usage<W: Write>(writer: &mut W) -> Result<(), super::RemoteError> {
    writeln!(
        writer,
        "Use pending-run-intent-preview --input FILE. The file is a caller-supplied offline declaration; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}
