use std::{
    io::{BufRead, Read, Write},
    path::Path,
};

use super::state::{TuiState, io_error, refresh_sessions, render, write_pending_recovery};
use super::{MAX_TUI_INPUT_BYTES, MAX_TUI_INPUT_READ_BYTES, commands::dispatch_command};
use super::{RemoteClient, RemoteError};

pub(super) async fn run_with_io<R: BufRead, W: Write>(
    client: &RemoteClient,
    state_dir: Option<&Path>,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let mut state = TuiState {
        change_cursor: client.saved_change_cursor()?,
        ..TuiState::default()
    };
    refresh_sessions(client, &mut state, false).await?;
    let mut line = String::new();
    loop {
        render(&state, writer)?;
        write!(writer, "> ").map_err(io_error)?;
        writer.flush().map_err(io_error)?;
        if !read_input_line(reader, &mut line)? {
            if state.has_pending_write() {
                write_pending_recovery(&state, writer)?;
            }
            break;
        }
        let command = line.trim_end_matches(['\r', '\n']);
        if command.len() > MAX_TUI_INPUT_BYTES {
            writeln!(writer, "Input exceeds the 256 KiB limit.").map_err(io_error)?;
            if state.has_pending_write() {
                write_pending_recovery(&state, writer)?;
            }
            break;
        }
        if dispatch_command(client, &mut state, state_dir, command, writer).await? {
            break;
        }
    }
    Ok(())
}

fn read_input_line<R: BufRead>(reader: &mut R, line: &mut String) -> Result<bool, RemoteError> {
    line.clear();
    (&mut *reader)
        .take(MAX_TUI_INPUT_READ_BYTES)
        .read_line(line)
        .map(|bytes_read| bytes_read != 0)
        .map_err(io_error)
}
