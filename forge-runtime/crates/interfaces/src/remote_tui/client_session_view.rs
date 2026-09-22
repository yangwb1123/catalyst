use std::io::Write;

use super::{RemoteError, state::io_error};

/// Shows one bounded client-instance/session-view contract file locally. The
/// TUI accepts a filesystem path so its interactive input remains available.
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
    let command = crate::args::DeviceCommand::ClientSessionViewPreview {
        input: input.to_owned(),
    };
    match crate::device_client_session_view_command::execute(&command) {
        Ok(output) => {
            crate::device_client_session_view_command::write_output(&output, false, writer)
                .map_err(io_error)?
        }
        Err(error) => writeln!(
            writer,
            "Client-instance/session-view preview failed: {error}"
        )
        .map_err(io_error)?,
    }
    Ok(())
}

fn usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use client-session-view-preview --input FILE. The file is a caller-supplied offline declaration; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}
