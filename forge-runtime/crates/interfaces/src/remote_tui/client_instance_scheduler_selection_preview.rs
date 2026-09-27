use std::io::Write;

use crate::args::DeviceCommand;

use super::super::RemoteError;

/// Renders one caller-supplied instance-scoped scheduler projection.  The
/// TUI accepts a file path so its interactive stdin remains available; this
/// command never opens a remote route.
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
    let command = DeviceCommand::ClientInstanceSchedulerSelectionPreview {
        input: input.to_owned(),
    };
    match crate::device_client_instance_scheduler_selection_preview_command::execute(&command) {
        Ok(preview) => crate::device_client_instance_scheduler_selection_preview_command::write_output(
            &preview,
            false,
            writer,
        )
        .map_err(|error| RemoteError(error.to_string()))?,
        Err(error) => writeln!(
            writer,
            "Client-instance scheduler-selection preview failed: {error}"
        )
        .map_err(|error| RemoteError(error.to_string()))?,
    }
    Ok(())
}

fn usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use client-instance-scheduler-selection-preview --input FILE. The file is a caller-supplied metadata-only projection; '-' is reserved for the standalone CLI."
    )
    .map_err(|error| RemoteError(error.to_string()))
}

