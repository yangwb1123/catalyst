use std::io::Write;

use super::{RemoteError, state::io_error};

/// Renders the pure device identity binding contract locally. It does not
/// verify cryptographic material or consume a challenge.
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
    let command = crate::args::DeviceCommand::IdentityProofPreview {
        input: input.to_owned(),
    };
    match crate::device_identity_proof_command::execute(&command) {
        Ok(output) => crate::device_identity_proof_command::write_output(&output, false, writer)
            .map_err(io_error)?,
        Err(error) => {
            writeln!(writer, "Identity proof preview failed: {error}").map_err(io_error)?
        }
    }
    Ok(())
}

fn usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use identity-proof-preview --input FILE. The file is a pure offline binding declaration; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}
