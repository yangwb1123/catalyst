use std::io::{self, Write};

use crate::args::DeviceCommand;

use super::super::RemoteError;

pub(super) fn preview<W: Write>(argument: &str, writer: &mut W) -> Result<(), RemoteError> {
    let input = argument
        .strip_prefix("--input")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            RemoteError(
                "Use execution-lease-checkpoint-preview --input FILE. The file is a caller-supplied offline declaration; '-' is reserved for the standalone CLI.".into(),
            )
        })?;
    let command = DeviceCommand::ExecutionLeaseCheckpointPreview {
        input: input.to_owned(),
    };
    match crate::device_execution_lease_checkpoint_command::execute(&command) {
        Ok(output) => {
            crate::device_execution_lease_checkpoint_command::write_output(&output, false, writer)
                .map_err(|error| io_error(&error))
        }
        Err(error) => Err(RemoteError(format!(
            "Execution lease checkpoint preview failed: {error}"
        ))),
    }
}

fn io_error(error: &io::Error) -> RemoteError {
    RemoteError(format!(
        "failed to write execution lease checkpoint preview: {error}"
    ))
}
