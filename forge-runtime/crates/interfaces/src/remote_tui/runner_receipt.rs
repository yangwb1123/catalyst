use std::io::Write;

use super::{RemoteError, state::io_error};

/// Shows one bounded terminal-receipt contract file locally. The TUI accepts
/// paths only so interactive stdin remains available for commands.
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
    let command = crate::args::DeviceCommand::RunnerReceiptPreview {
        input: input.to_owned(),
    };
    match crate::device_runner_receipt_command::execute(&command) {
        Ok(output) => crate::device_runner_receipt_command::write_output(&output, false, writer)
            .map_err(io_error)?,
        Err(error) => {
            writeln!(writer, "Runner terminal receipt preview failed: {error}")
                .map_err(io_error)?;
        }
    }
    Ok(())
}

fn usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use runner-receipt-preview --input FILE. The file is a caller-supplied offline declaration; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}
