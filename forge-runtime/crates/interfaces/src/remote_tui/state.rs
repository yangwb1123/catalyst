use std::io::Write;

use rand::TryRngCore;
use serde_json::Value;

use super::super::{OwnedConversationEntry, RemoteClient, RemoteError};
use crate::args::{PromptPageCursor, RemoteConversationScope};

#[derive(Default)]
pub(super) struct TuiState {
    pub(super) conversations: Vec<OwnedConversationEntry>,
    pub(super) selected_id: Option<String>,
    pub(super) selected_entry: Option<OwnedConversationEntry>,
    pub(super) next_after_id: Option<String>,
    pub(super) has_more: bool,
    pub(super) change_cursor: u64,
    pub(super) history_loaded_for: Option<String>,
    pub(super) prompt_history: Vec<Value>,
    pub(super) history_before: Option<PromptPageCursor>,
    pub(super) pending_prompt: Option<PendingPrompt>,
    pub(super) pending_create: Option<PendingCreate>,
}

#[derive(Clone)]
pub(super) struct PendingPrompt {
    pub(super) conversation_id: String,
    pub(super) expected_version: u64,
    pub(super) content: String,
    pub(super) idempotency_key: String,
}

#[derive(Clone)]
pub(super) struct PendingCreate {
    pub(super) title: String,
    pub(super) scope: RemoteConversationScope,
    pub(super) idempotency_key: String,
}

impl TuiState {
    pub(super) fn has_pending_write(&self) -> bool {
        self.pending_prompt.is_some() || self.pending_create.is_some()
    }

    pub(super) fn selected_conversation(&self) -> Option<&OwnedConversationEntry> {
        let selected_id = self.selected_id.as_deref()?;
        self.conversations
            .iter()
            .chain(self.selected_entry.iter())
            .find(|entry| entry.conversation.get("id").and_then(Value::as_str) == Some(selected_id))
    }

    pub(super) fn record_prompt_history(&mut self, conversation_id: &str, page: &Value) {
        if self.history_loaded_for.as_deref() != Some(conversation_id) {
            self.clear_prompt_history();
        }
        self.history_loaded_for = Some(conversation_id.to_owned());
        if let Some(prompts) = page.get("prompts").and_then(Value::as_array) {
            for prompt in prompts {
                let Some(id) = prompt.get("id").and_then(Value::as_str) else {
                    continue;
                };
                if let Some(existing) = self
                    .prompt_history
                    .iter_mut()
                    .find(|existing| existing.get("id").and_then(Value::as_str) == Some(id))
                {
                    *existing = prompt.clone();
                } else {
                    self.prompt_history.push(prompt.clone());
                }
            }
            self.prompt_history.sort_by(|left, right| {
                let left_key = (
                    left.get("created_at_ms")
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                    left.get("id").and_then(Value::as_str).unwrap_or_default(),
                );
                let right_key = (
                    right
                        .get("created_at_ms")
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                    right.get("id").and_then(Value::as_str).unwrap_or_default(),
                );
                left_key.cmp(&right_key)
            });
        }
        let page_cursor = page.get("next_cursor").and_then(|cursor| {
            Some(PromptPageCursor {
                created_at_ms: cursor.get("created_at_ms")?.as_u64()?,
                prompt_id: cursor.get("prompt_id")?.as_str()?.to_owned(),
            })
        });
        self.history_before = match (&self.history_before, page_cursor) {
            (Some(current), Some(incoming)) if cursor_precedes(current, &incoming) => {
                Some(current.clone())
            }
            (_, incoming) => incoming,
        };
    }

    pub(super) fn clear_prompt_history(&mut self) {
        self.history_loaded_for = None;
        self.prompt_history.clear();
        self.history_before = None;
    }
}

fn cursor_precedes(left: &PromptPageCursor, right: &PromptPageCursor) -> bool {
    left.created_at_ms < right.created_at_ms
        || (left.created_at_ms == right.created_at_ms && left.prompt_id < right.prompt_id)
}

pub(super) fn render<W: Write>(state: &TuiState, writer: &mut W) -> Result<(), RemoteError> {
    writeln!(writer, "\nForge shared sessions").map_err(io_error)?;
    render_conversations(state, writer)?;
    render_selected_entry(state, writer)?;
    render_selected_history(state, writer)?;
    render_pagination(state, writer)?;
    render_pending_writes(state, writer)
}

fn render_selected_history<W: Write>(state: &TuiState, writer: &mut W) -> Result<(), RemoteError> {
    let Some(selected_id) = state.selected_id.as_deref() else {
        return Ok(());
    };
    writeln!(writer, "  Prompt history for {}:", json_text(selected_id)).map_err(io_error)?;
    if state.history_loaded_for.as_deref() != Some(selected_id) {
        writeln!(writer, "    Not loaded; enter open {selected_id} or sync.").map_err(io_error)?;
        return Ok(());
    }
    if state.prompt_history.is_empty() {
        writeln!(writer, "    No prompts.").map_err(io_error)?;
    }
    for prompt in &state.prompt_history {
        let id = prompt
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let role = prompt
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let content = prompt
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let created_at_ms = prompt
            .get("created_at_ms")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        writeln!(
            writer,
            "    {}  {}  {}  {}",
            created_at_ms,
            json_text(id),
            json_text(role),
            json_text(content)
        )
        .map_err(io_error)?;
    }
    if state.history_before.is_some() {
        writeln!(writer, "    More history is available: enter older.").map_err(io_error)?;
    }
    Ok(())
}

fn render_conversations<W: Write>(state: &TuiState, writer: &mut W) -> Result<(), RemoteError> {
    if state.conversations.is_empty() {
        writeln!(writer, "  No sessions on this page.").map_err(io_error)?;
    }
    for entry in &state.conversations {
        let Some(id) = entry.conversation.get("id").and_then(Value::as_str) else {
            continue;
        };
        let title = entry
            .conversation
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("Untitled");
        let marker = if state.selected_id.as_deref() == Some(id) {
            "*"
        } else {
            " "
        };
        writeln!(
            writer,
            " {marker} {}  {}  [{}]  (version {})",
            json_text(id),
            json_text(title),
            conversation_scope_label(&entry.conversation),
            entry.aggregate_version
        )
        .map_err(io_error)?;
    }
    Ok(())
}

fn render_selected_entry<W: Write>(state: &TuiState, writer: &mut W) -> Result<(), RemoteError> {
    if let (Some(selected_id), Some(entry)) = (&state.selected_id, &state.selected_entry)
        && entry.conversation.get("id").and_then(Value::as_str) == Some(selected_id)
    {
        let title = entry
            .conversation
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("Untitled");
        writeln!(
            writer,
            " * {}  {}  [{}]  (version {}, outside loaded pages)",
            json_text(selected_id),
            json_text(title),
            conversation_scope_label(&entry.conversation),
            entry.aggregate_version
        )
        .map_err(io_error)?;
    }
    Ok(())
}

fn render_pagination<W: Write>(state: &TuiState, writer: &mut W) -> Result<(), RemoteError> {
    if state.has_more {
        writeln!(writer, "  More sessions are available: enter next.").map_err(io_error)?;
    }
    Ok(())
}

fn render_pending_writes<W: Write>(state: &TuiState, writer: &mut W) -> Result<(), RemoteError> {
    if let Some(pending) = &state.pending_prompt {
        writeln!(
            writer,
            "  Prompt outcome is not confirmed for {}; enter retry to resend the identical request.\n",
            json_text(&pending.conversation_id)
        )
        .map_err(io_error)?;
    }
    if state.pending_create.is_some() {
        writeln!(
            writer,
            "  Session creation outcome is not confirmed; enter retry to resend the identical request.\n"
        )
        .map_err(io_error)?;
    }
    Ok(())
}

pub(super) fn write_help<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Commands: list | next | sync | open ID | older | runs [--before TIME RUN_ID] | timeline RUN_ID [AFTER_SEQUENCE] | create [--scope global|project:ID|group:ID] TITLE | prompt TEXT | retry | quit | quit --discard-pending"
    )
    .map_err(io_error)
}

fn conversation_scope_label(conversation: &Value) -> String {
    let Some(scope) = conversation.get("scope") else {
        return "scope unknown".to_owned();
    };
    let kind = scope
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    match (kind, scope.get("id").and_then(Value::as_str)) {
        ("global", _) => "global".to_owned(),
        ("project" | "group", Some(id)) => format!("{kind}:{}", json_text(id)),
        _ => "scope unknown".to_owned(),
    }
}

/// Loads a page and commits it to the local session view only after validation.
///
/// # Errors
///
/// Returns an error when the API request fails or the page violates its cursor contract.
pub(super) async fn refresh_sessions(
    client: &RemoteClient,
    state: &mut TuiState,
    next_page: bool,
) -> Result<(), RemoteError> {
    let after = if next_page {
        if !state.has_more {
            return Ok(());
        }
        Some(
            state
                .next_after_id
                .as_deref()
                .ok_or_else(|| RemoteError("Forge API pagination cursor is unavailable".into()))?,
        )
    } else {
        None
    };
    let page = client.list_conversations(after).await?;
    if next_page {
        append_page(state, page);
    } else {
        replace_page(state, page);
    }
    Ok(())
}

fn append_page(state: &mut TuiState, page: super::super::OwnedConversationPage) {
    let super::super::OwnedConversationPage {
        conversations,
        next_after_id,
        has_more,
    } = page;
    for entry in conversations {
        let incoming_id = entry.conversation.get("id").and_then(Value::as_str);
        if let Some(incoming_id) = incoming_id
            && let Some(existing) = state.conversations.iter_mut().find(|existing| {
                existing.conversation.get("id").and_then(Value::as_str) == Some(incoming_id)
            })
        {
            *existing = entry;
        } else {
            state.conversations.push(entry);
        }
    }
    if selected_is_loaded(state) {
        state.selected_entry = None;
    }
    set_page_cursor(state, next_after_id, has_more);
}

fn replace_page(state: &mut TuiState, page: super::super::OwnedConversationPage) {
    let super::super::OwnedConversationPage {
        conversations,
        next_after_id,
        has_more,
    } = page;
    let preserved_selection = preserve_selected_entry(state);
    let selected_id = state.selected_id.clone();
    state.conversations = conversations;
    state.selected_entry = selected_id.as_deref().and_then(|selected_id| {
        if page_contains_id(&state.conversations, selected_id) {
            None
        } else {
            preserved_selection.filter(|entry| {
                entry.conversation.get("id").and_then(Value::as_str) == Some(selected_id)
            })
        }
    });
    if selected_is_available(state) {
        state.selected_id.clone_from(&selected_id);
    } else {
        state.selected_id = state
            .conversations
            .first()
            .and_then(|entry| entry.conversation.get("id"))
            .and_then(Value::as_str)
            .map(str::to_owned);
    }
    if state.selected_id != selected_id {
        state.clear_prompt_history();
    }
    set_page_cursor(state, next_after_id, has_more);
}

fn preserve_selected_entry(state: &TuiState) -> Option<OwnedConversationEntry> {
    let selected_id = state.selected_id.as_deref()?;
    state
        .conversations
        .iter()
        .chain(state.selected_entry.iter())
        .find(|entry| entry.conversation.get("id").and_then(Value::as_str) == Some(selected_id))
        .cloned()
}

fn selected_is_loaded(state: &TuiState) -> bool {
    state
        .selected_id
        .as_deref()
        .is_some_and(|selected_id| page_contains_id(&state.conversations, selected_id))
}

fn selected_is_available(state: &TuiState) -> bool {
    state.selected_id.as_deref().is_some_and(|selected_id| {
        page_contains_id(&state.conversations, selected_id)
            || state.selected_entry.as_ref().is_some_and(|entry| {
                entry.conversation.get("id").and_then(Value::as_str) == Some(selected_id)
            })
    })
}

fn page_contains_id(page: &[OwnedConversationEntry], id: &str) -> bool {
    page.iter()
        .any(|entry| entry.conversation.get("id").and_then(Value::as_str) == Some(id))
}

fn set_page_cursor(state: &mut TuiState, next_after_id: Option<String>, has_more: bool) {
    state.next_after_id = next_after_id;
    state.has_more = has_more;
}

/// Prints the metadata needed to replay a pending write without exposing prompt text.
///
/// # Errors
///
/// Returns an error when the terminal writer fails.
pub(super) fn write_pending_recovery<W: Write>(
    state: &TuiState,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let recovery = if let Some(pending) = &state.pending_prompt {
        serde_json::json!({
            "operation": "append_prompt",
            "conversation_id": pending.conversation_id,
            "expected_version": pending.expected_version,
            "idempotency_key": pending.idempotency_key,
            "content_included": false,
            "replay_note": "Use the original prompt text with remote prompts add and the same version/key."
        })
    } else if let Some(pending) = &state.pending_create {
        serde_json::json!({
            "operation": "create_conversation",
            "title": pending.title,
            "scope": super::super::scope_json(&pending.scope),
            "idempotency_key": pending.idempotency_key,
            "replay_note": "Use remote sessions create and the same key."
        })
    } else {
        return Ok(());
    };
    writeln!(writer, "Pending write recovery: {recovery}").map_err(io_error)
}

/// Creates a fresh, process-local idempotency key for a new TUI write.
///
/// # Errors
///
/// Returns an error when the operating system random source fails.
pub(super) fn new_idempotency_key() -> Result<String, RemoteError> {
    let mut bytes = [0_u8; 16];
    rand::rngs::OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| RemoteError("could not create an idempotency key".into()))?;
    let mut output = String::from("forge-tui-");
    for byte in bytes {
        use std::fmt::Write as _;
        write!(output, "{byte:02x}").map_err(io_error)?;
    }
    Ok(output)
}

pub(super) fn json_text(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

/// Wraps terminal writer failures without leaking content or host details.
pub(super) fn io_error(_: impl std::fmt::Display) -> RemoteError {
    RemoteError("could not write the terminal session".into())
}
