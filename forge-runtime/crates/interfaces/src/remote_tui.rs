use std::io::{self, BufRead, IsTerminal, Read, Write};

use serde_json::Value;

use self::state::{
    PendingCreate, PendingPrompt, TuiState, io_error, json_text, new_idempotency_key,
    refresh_sessions, render, scope_filter_label, write_help, write_pending_recovery,
};
use super::{OwnedConversationEntry, RemoteClient, RemoteError};
use crate::args::{RemoteConversationScope, parse_scope};

#[path = "remote_tui/state.rs"]
mod state;

#[path = "remote_tui/sync.rs"]
mod sync;

#[path = "remote_tui/runs.rs"]
mod runs;

use self::sync::sync_command;

const MAX_TUI_INPUT_BYTES: usize = 256 * 1024;
const MAX_TUI_INPUT_READ_BYTES: u64 = 256 * 1024 + 1;

/// Starts a TUI after confirming both process streams are interactive terminals.
///
/// # Errors
///
/// Returns an error when terminal access, configuration, or the remote API fails.
pub(super) async fn run() -> Result<(), RemoteError> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(RemoteError(
            "remote tui requires an interactive terminal".into(),
        ));
    }
    let client = RemoteClient::from_env().await?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_with_io(&client, &mut stdin.lock(), &mut stdout.lock()).await
}

async fn run_with_io<R: BufRead, W: Write>(
    client: &RemoteClient,
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
        if dispatch_command(client, &mut state, command, writer).await? {
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

async fn dispatch_command<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    command: &str,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    let (verb, argument) = command
        .split_once(' ')
        .map_or((command, ""), |(verb, rest)| (verb, rest.trim_start()));
    if matches!(verb, "q" | "quit" | "exit") {
        return handle_exit(state, argument, writer);
    }
    match verb {
        "help" | "?" => write_help(writer)?,
        "filter" => filter_sessions(state, argument, writer)?,
        "sync" => sync_command(client, state, writer).await?,
        "runs" => runs::runs_command(client, state, argument, writer).await?,
        "timeline" => runs::timeline_command(client, state, argument, writer).await?,
        "older" | "history" => older_history(client, state, argument, writer).await?,
        "list" | "refresh" | "r" => refresh_command(client, state, false, writer).await?,
        "next" | "n" => refresh_command(client, state, true, writer).await?,
        "open" | "o" => open_session(client, state, argument, writer).await?,
        "create" | "c" => create_session(client, state, argument, writer).await?,
        "prompt" | "p" => send_new_prompt(client, state, argument, writer).await?,
        "retry" => retry_pending(client, state, writer).await?,
        "" => {}
        other => writeln!(writer, "Unknown command: {other}. Enter help for commands.")
            .map_err(io_error)?,
    }
    Ok(false)
}

fn filter_sessions<W: Write>(
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let selector = argument.trim();
    if selector == "clear" {
        state.scope_filter = None;
        writeln!(writer, "Scope filter cleared.").map_err(io_error)?;
        return Ok(());
    }
    let Ok(scope_filter) = parse_scope(selector) else {
        writeln!(
            writer,
            "Invalid scope filter. Use filter global|project:ID|group:ID or filter clear."
        )
        .map_err(io_error)?;
        return Ok(());
    };
    let label = scope_filter_label(&scope_filter);
    state.scope_filter = Some(scope_filter);
    writeln!(
        writer,
        "Scope filter set to {label}; this only organizes the displayed session list, not authorization or device identity."
    )
    .map_err(io_error)
}

fn handle_exit<W: Write>(
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    if state.has_pending_write() {
        if argument == "--discard-pending" {
            write_pending_recovery(state, writer)?;
            state.pending_prompt = None;
            state.pending_create = None;
            return Ok(true);
        }
        writeln!(writer, "A write may have been accepted. Enter retry, or quit --discard-pending to print recovery data and exit.")
            .map_err(io_error)?;
        return Ok(false);
    }
    if argument.is_empty() {
        return Ok(true);
    }
    writeln!(
        writer,
        "Use quit with no arguments, or quit --discard-pending."
    )
    .map_err(io_error)?;
    Ok(false)
}

async fn refresh_command<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    next_page: bool,
    writer: &mut W,
) -> Result<(), RemoteError> {
    match refresh_sessions(client, state, next_page).await {
        Ok(()) if next_page => Ok(()),
        Ok(()) => writeln!(writer, "Sessions refreshed.").map_err(io_error),
        Err(error) => {
            let action = if next_page {
                "Next page failed"
            } else {
                "Refresh failed"
            };
            writeln!(writer, "{action}: {error}").map_err(io_error)
        }
    }
}

async fn open_session<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    conversation_id: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if !state
        .conversations
        .iter()
        .chain(state.selected_entry.iter())
        .any(|entry| entry.conversation.get("id").and_then(Value::as_str) == Some(conversation_id))
    {
        writeln!(
            writer,
            "That session is not in the loaded pages. Enter list or next first."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    if state.selected_id.as_deref() != Some(conversation_id) {
        state.clear_prompt_history();
    }
    state.selected_id = Some(conversation_id.to_owned());
    match load_session_history(client, conversation_id).await {
        Ok(prompts) => {
            state.record_prompt_history(conversation_id, &prompts);
            writeln!(
                writer,
                "Opened session {} and refreshed Prompt history.",
                json_text(conversation_id)
            )
            .map_err(io_error)?;
        }
        Err(error) => writeln!(writer, "History request failed: {error}").map_err(io_error)?,
    }
    Ok(())
}

async fn load_session_history(
    client: &RemoteClient,
    conversation_id: &str,
) -> Result<Value, RemoteError> {
    client.list_prompts(conversation_id, None).await
}

async fn older_history<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if !argument.trim().is_empty() {
        writeln!(
            writer,
            "Use older with no arguments to load the next Prompt page."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    let Some(conversation_id) = state.selected_id.clone() else {
        writeln!(
            writer,
            "Open a session before loading older Prompt history."
        )
        .map_err(io_error)?;
        return Ok(());
    };
    if state.history_loaded_for.as_deref() != Some(conversation_id.as_str()) {
        writeln!(
            writer,
            "Open the selected session before loading older Prompt history."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    let Some(before) = state.history_before.clone() else {
        writeln!(writer, "No older Prompt history is available.").map_err(io_error)?;
        return Ok(());
    };
    match client.list_prompts(&conversation_id, Some(&before)).await {
        Ok(prompts) => {
            state.record_prompt_history(&conversation_id, &prompts);
            writeln!(writer, "Older Prompt history loaded.").map_err(io_error)?;
        }
        Err(error) => {
            writeln!(writer, "Older history request failed: {error}").map_err(io_error)?;
        }
    }
    Ok(())
}

async fn create_session<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if state.pending_prompt.is_some() || state.pending_create.is_some() {
        writeln!(
            writer,
            "Resolve the pending write with retry before starting another write."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    let (scope, title) = parse_create_argument(argument)?;
    if title.trim().is_empty() || title.len() > 256 {
        writeln!(writer, "Title must contain 1..256 bytes.").map_err(io_error)?;
        return Ok(());
    }
    state.pending_create = Some(PendingCreate {
        title,
        scope,
        idempotency_key: new_idempotency_key()?,
    });
    retry_pending(client, state, writer).await
}

fn parse_create_argument(argument: &str) -> Result<(RemoteConversationScope, String), RemoteError> {
    if let Some(options) = argument.strip_prefix("--scope ") {
        let (selector, title) = options
            .split_once(' ')
            .ok_or_else(|| RemoteError("create --scope requires a scope and title".into()))?;
        let scope = parse_scope(selector).map_err(RemoteError)?;
        return Ok((scope, title.trim_start().to_owned()));
    }
    Ok((RemoteConversationScope::Global, argument.to_owned()))
}

async fn send_new_prompt<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    content: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if state.pending_prompt.is_some() || state.pending_create.is_some() {
        writeln!(
            writer,
            "Resolve the pending write with retry before starting another write."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    let Some(conversation_id) = state.selected_id.clone() else {
        writeln!(writer, "Open a session before sending a prompt.").map_err(io_error)?;
        return Ok(());
    };
    let Some(entry) = state.selected_conversation() else {
        writeln!(
            writer,
            "Refresh the selected session before sending a prompt."
        )
        .map_err(io_error)?;
        return Ok(());
    };
    if content.trim().is_empty() || content.len() > MAX_TUI_INPUT_BYTES {
        writeln!(writer, "Prompt must contain 1..256 KiB.").map_err(io_error)?;
        return Ok(());
    }
    let expected_version = entry.aggregate_version;
    state.pending_prompt = Some(PendingPrompt {
        conversation_id,
        expected_version,
        content: content.to_owned(),
        idempotency_key: new_idempotency_key()?,
    });
    retry_pending(client, state, writer).await
}

async fn retry_pending<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if state.pending_prompt.is_some() {
        return retry_prompt(client, state, writer).await;
    }
    if state.pending_create.is_some() {
        return retry_create(client, state, writer).await;
    }
    writeln!(writer, "There is no unconfirmed write to retry.").map_err(io_error)?;
    Ok(())
}

async fn retry_prompt<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(pending) = state.pending_prompt.clone() else {
        return Ok(());
    };
    let result = client
        .append_prompt(
            &pending.conversation_id,
            pending.expected_version,
            &pending.content,
            &pending.idempotency_key,
        )
        .await;
    match result {
        Ok(result) => report_prompt_accepted(client, state, &pending, &result, writer).await?,
        Err(error) => {
            handle_prompt_error(client, state, &pending, &error, writer).await?;
        }
    }
    Ok(())
}

async fn report_prompt_accepted<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    pending: &PendingPrompt,
    result: &Value,
    writer: &mut W,
) -> Result<(), RemoteError> {
    state.pending_prompt = None;
    if let Some(version) = result.get("aggregate_version").and_then(Value::as_u64) {
        update_prompt_version(state, &pending.conversation_id, version);
    } else {
        state.selected_id = None;
        state.selected_entry = None;
        state.clear_prompt_history();
    }
    writeln!(writer, "Prompt stored. No Run was started.").map_err(io_error)?;
    if result
        .get("aggregate_version")
        .and_then(Value::as_u64)
        .is_none()
    {
        writeln!(
            writer,
            "The response omitted the new version; refresh before sending another prompt."
        )
        .map_err(io_error)?;
    } else {
        match load_session_history(client, &pending.conversation_id).await {
            Ok(page) => {
                state.record_prompt_history(&pending.conversation_id, &page);
                writeln!(writer, "Prompt history refreshed.").map_err(io_error)?;
            }
            Err(error) => writeln!(
                writer,
                "Prompt was stored, but history refresh failed: {error}. Use sync to refresh it."
            )
            .map_err(io_error)?,
        }
    }
    Ok(())
}

fn update_prompt_version(state: &mut TuiState, conversation_id: &str, version: u64) {
    if let Some(entry) = state
        .conversations
        .iter_mut()
        .chain(state.selected_entry.iter_mut())
        .find(|entry| entry.conversation.get("id").and_then(Value::as_str) == Some(conversation_id))
    {
        entry.aggregate_version = version;
    }
}

async fn handle_prompt_error<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    pending: &PendingPrompt,
    error: &RemoteError,
    writer: &mut W,
) -> Result<(), RemoteError> {
    match response_status(error) {
        Some(409) => {
            state.pending_prompt = None;
            writeln!(writer, "Prompt was rejected: {error}").map_err(io_error)?;
            refresh_after_conflict(client, state, pending, writer).await
        }
        Some(status) if definitive_client_rejection(status) => {
            state.pending_prompt = None;
            writeln!(writer, "Prompt was rejected: {error}").map_err(io_error)
        }
        _ => writeln!(
            writer,
            "{error}. Enter retry to reuse the same version and idempotency key."
        )
        .map_err(io_error),
    }
}

async fn refresh_after_conflict<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    pending: &PendingPrompt,
    writer: &mut W,
) -> Result<(), RemoteError> {
    match refresh_sessions(client, state, false).await {
        Ok(()) => {
            let session_is_loaded = state.conversations.iter().any(|entry| {
                entry.conversation.get("id").and_then(Value::as_str)
                    == Some(&pending.conversation_id)
            });
            if session_is_loaded {
                writeln!(
                    writer,
                    "The session was refreshed; submit the prompt again."
                )
                .map_err(io_error)
            } else {
                state.selected_id = None;
                state.selected_entry = None;
                state.clear_prompt_history();
                writeln!(writer, "The session is outside the refreshed page. Use next until it appears, then open it before submitting again.")
                    .map_err(io_error)
            }
        }
        Err(refresh_error) => writeln!(
            writer,
            "Session refresh failed: {refresh_error}. Refresh before submitting again."
        )
        .map_err(io_error),
    }
}

async fn retry_create<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(pending) = state.pending_create.clone() else {
        return Ok(());
    };
    let result = client
        .create_conversation(&pending.title, &pending.scope, &pending.idempotency_key)
        .await;
    match result {
        Ok(created) => complete_create(client, state, created, writer).await?,
        Err(error) => report_create_error(state, &error, writer)?,
    }
    Ok(())
}

async fn complete_create<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    created: Value,
    writer: &mut W,
) -> Result<(), RemoteError> {
    state.pending_create = None;
    let id = created.get("id").and_then(Value::as_str).map(str::to_owned);
    writeln!(
        writer,
        "Created session {}.",
        id.as_deref()
            .map_or_else(|| "(id unavailable)".into(), json_text)
    )
    .map_err(io_error)?;
    if let Some(id) = id {
        state.selected_entry = Some(OwnedConversationEntry {
            conversation: created,
            aggregate_version: 1,
        });
        state.clear_prompt_history();
        state.selected_id = Some(id);
    }
    if let Err(error) = refresh_sessions(client, state, false).await {
        writeln!(writer, "Session was created, but refresh failed: {error}").map_err(io_error)?;
    }
    Ok(())
}

fn report_create_error<W: Write>(
    state: &mut TuiState,
    error: &RemoteError,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if response_status(error).is_some_and(definitive_client_rejection) {
        state.pending_create = None;
        writeln!(writer, "Create was rejected: {error}").map_err(io_error)
    } else {
        writeln!(
            writer,
            "{error}. Enter retry to reuse the same idempotency key."
        )
        .map_err(io_error)
    }
}

fn response_status(error: &RemoteError) -> Option<u16> {
    error
        .0
        .strip_prefix("Forge API returned HTTP ")
        .and_then(|value| value.get(..3))
        .and_then(|value| value.parse::<u16>().ok())
}

fn definitive_client_rejection(status: u16) -> bool {
    (400..500).contains(&status) && !matches!(status, 408 | 425 | 429)
}

#[cfg(test)]
#[path = "remote_tui_tests.rs"]
mod tests;
