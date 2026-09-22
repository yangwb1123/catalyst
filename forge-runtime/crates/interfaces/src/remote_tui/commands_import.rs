use std::{io::Write, path::Path};

use super::super::state::{TuiState, io_error};
use super::super::{RemoteClient, RemoteError};

pub(super) async fn import_session<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    state_dir: Option<&Path>,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let mut tokens = argument.split_whitespace();
    let Some(conversation_id) = tokens.next() else {
        return write_usage(writer);
    };
    let mut confirmation = None;
    while let Some(option) = tokens.next() {
        if option != "--confirm" || confirmation.is_some() {
            return write_usage(writer);
        }
        let Some(value) = tokens.next() else {
            return write_usage(writer);
        };
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            writeln!(
                writer,
                "Import confirmation must be a lowercase SHA-256 digest."
            )
            .map_err(io_error)?;
            return Ok(());
        }
        confirmation = Some(value);
    }

    let preview =
        match super::super::super::import::prepare_preview(client, state_dir, conversation_id) {
            Ok(preview) => preview,
            Err(error) => {
                writeln!(writer, "Local conversation import preview failed: {error}")
                    .map_err(io_error)?;
                return Ok(());
            }
        };
    let Some(confirmation) = confirmation else {
        let output = super::super::super::import::render_preview_text(&preview)?;
        writer.write_all(output.as_bytes()).map_err(io_error)?;
        return Ok(());
    };
    if confirmation != preview.digest {
        writeln!(
            writer,
            "Import confirmation does not match the current preview; no data was uploaded."
        )
        .map_err(io_error)?;
        let output = super::super::super::import::render_preview_text(&preview)?;
        writer.write_all(output.as_bytes()).map_err(io_error)?;
        return Ok(());
    }
    match super::super::super::import::confirm_preview(client, &preview).await {
        Ok(result) => {
            let output = super::super::super::import::render_import_result(
                &result,
                preview.source.prompts.len(),
            )?;
            writer.write_all(output.as_bytes()).map_err(io_error)?;
        }
        Err(error) => {
            let cleared = super::super::clear_session_view_after_authorization_error(state, &error);
            writeln!(writer, "Local conversation import failed: {error}").map_err(io_error)?;
            if cleared {
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(io_error)?;
            }
        }
    }
    Ok(())
}

fn write_usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use import LOCAL_CONVERSATION_ID [--confirm SHA256]."
    )
    .map_err(io_error)
}
