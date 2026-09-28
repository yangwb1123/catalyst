use super::{RemoteClient, RemoteError, TuiState, io_error};
use std::io::Write;

pub(super) async fn placement_preview<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(input) = argument.strip_prefix("--input") else {
        return write_placement_usage(writer);
    };
    if !input.chars().next().is_some_and(char::is_whitespace) {
        return write_placement_usage(writer);
    }
    let input = input.trim();
    if input.is_empty() || input == "-" {
        return write_placement_usage(writer);
    }
    let request = match super::super::super::placement::read_tui_request(input) {
        Ok(request) => request,
        Err(error) => {
            writeln!(writer, "Placement preview input failed: {error}").map_err(io_error)?;
            return Ok(());
        }
    };
    let response = match client.preview_device_placement(&request).await {
        Ok(response) => response,
        Err(error) => {
            report_request_failure(state, &error, "Placement preview", writer)?;
            return Ok(());
        }
    };
    match super::super::super::placement::validate_response(&response, &request) {
        Ok(result) => {
            super::super::super::placement::render_human(&result, writer).map_err(io_error)?;
        }
        Err(error) => {
            writeln!(
                writer,
                "Placement preview response failed validation: {error}"
            )
            .map_err(io_error)?;
        }
    }
    Ok(())
}

pub(super) async fn placement_registry_preview<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(input) = argument.strip_prefix("--input") else {
        return write_placement_registry_usage(writer);
    };
    if !input.chars().next().is_some_and(char::is_whitespace) {
        return write_placement_registry_usage(writer);
    }
    let input = input.trim();
    if input.is_empty() || input == "-" {
        return write_placement_registry_usage(writer);
    }
    let request = match super::super::super::placement_registry::read_tui_request(input) {
        Ok(request) => request,
        Err(error) => {
            writeln!(writer, "Registry placement preview input failed: {error}")
                .map_err(io_error)?;
            return Ok(());
        }
    };
    let response = match client.preview_device_placement_registry(&request).await {
        Ok(response) => response,
        Err(error) => {
            report_request_failure(state, &error, "Registry placement preview", writer)?;
            return Ok(());
        }
    };
    match super::super::super::placement_registry::validate_response(&response) {
        Ok(result) => {
            super::super::super::placement_registry::render_human(&result, writer)
                .map_err(io_error)?;
        }
        Err(error) => {
            writeln!(
                writer,
                "Registry placement preview response failed validation: {error}"
            )
            .map_err(io_error)?;
        }
    }
    Ok(())
}

pub(super) fn write_placement_usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use placement-preview --input FILE. The file is a caller-supplied offline declaration; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}

pub(super) fn write_placement_registry_usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use placement-registry-preview --input FILE. The request contains only placement requirements; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}

fn report_request_failure<W: Write>(
    state: &mut TuiState,
    error: &RemoteError,
    operation: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let cleared = super::super::clear_session_view_after_authorization_error(state, error);
    writeln!(writer, "{operation} request failed: {error}").map_err(io_error)?;
    if cleared {
        writeln!(
            writer,
            "Local session view cleared after authorization failure."
        )
        .map_err(io_error)?;
    }
    Ok(())
}
