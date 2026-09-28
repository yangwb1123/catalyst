use std::io::Write;

use super::RemoteError;
use super::state::io_error;

/// Renders a local, metadata-only credential lifecycle candidate. The TUI
/// command is deliberately file-backed, so it cannot issue the Core POST or
/// replay a bearer token.
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
    let command = crate::args::DeviceCommand::CredentialCandidatePreview {
        input: input.to_owned(),
    };
    match crate::device_credential_candidate_command::execute(&command) {
        Ok(output) => {
            crate::device_credential_candidate_command::write_output(&output, false, writer)
                .map_err(io_error)?;
        }
        Err(error) => {
            writeln!(writer, "Credential candidate preview failed: {error}").map_err(io_error)?;
        }
    }
    Ok(())
}

fn usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use credential-candidate-preview --input FILE. This local decoder is request-free and carries no credential material."
    )
    .map_err(io_error)
}

/// Performs one explicitly requested owner-scoped metadata-only POST.  It is
/// intentionally separate from the local `credential-candidate-preview`
/// decoder, so startup and `sync` never invoke the endpoint.
pub(super) async fn remote_preview<W: Write>(
    client: &super::RemoteClient,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(input) = argument.strip_prefix("--input") else {
        return remote_usage(writer);
    };
    if !input.chars().next().is_some_and(char::is_whitespace) {
        return remote_usage(writer);
    }
    let input = input.trim();
    if input.is_empty() || input == "-" {
        return remote_usage(writer);
    }
    let request = match crate::remote_credential_candidate::read_tui_request(input) {
        Ok(request) => request,
        Err(error) => {
            writeln!(writer, "Remote credential-candidate input failed: {error}")
                .map_err(io_error)?;
            return Ok(());
        }
    };
    let response = match client.preview_device_credential_candidate(&request).await {
        Ok(response) => response,
        Err(error) => {
            writeln!(
                writer,
                "Remote credential-candidate request failed: {error}"
            )
            .map_err(io_error)?;
            return Ok(());
        }
    };
    if let Err(error) = crate::remote_credential_candidate::validate_response(&response, &request) {
        writeln!(
            writer,
            "Remote credential-candidate response failed validation: {error}"
        )
        .map_err(io_error)?;
        return Ok(());
    }
    crate::remote_credential_candidate::render_human(&response, writer)
}

fn remote_usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use credential-candidate --input FILE. This sends one authenticated metadata-only POST; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}
