use super::{RemoteError, io_error};
use crate::args::{DeviceCommand, DevicePlacementCommand};
use std::io::Write;

/// Renders two bounded local contract files without consuming the interactive
/// input stream or contacting a device endpoint.
pub(super) fn show_run_intent_preview<W: Write>(
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(suffix) = argument.strip_prefix("--input") else {
        return write_run_intent_usage(writer);
    };
    if !suffix.chars().next().is_some_and(char::is_whitespace) {
        return write_run_intent_usage(writer);
    }
    let Some((input, placement_input)) = suffix.trim().split_once(" --placement-input ") else {
        return write_run_intent_usage(writer);
    };
    let input = input.trim();
    let placement_input = placement_input.trim();
    if input.is_empty() || placement_input.is_empty() || input == "-" || placement_input == "-" {
        return write_run_intent_usage(writer);
    }
    let command = DeviceCommand::Placement(DevicePlacementCommand::RunIntentPreview {
        input: input.to_owned(),
        placement_input: placement_input.to_owned(),
    });
    match crate::device_run_intent_command::execute(&command) {
        Ok(output) => crate::device_run_intent_command::write_output(&output, false, writer)
            .map_err(io_error)?,
        Err(error) => writeln!(writer, "Run-intent preview failed: {error}").map_err(io_error)?,
    }
    Ok(())
}

pub(super) fn write_run_intent_usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use run-intent-preview --input RUN_FILE --placement-input SESSION_FILE. Both inputs are offline files; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}
