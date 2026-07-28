use std::{ops::Range, sync::Arc};

use thiserror::Error;

use crate::runtime_domain::{HubStore, HubStoreError, Message, PromptRecord};

pub const HISTORY_RECORD_LIMIT: usize = 16;
const HISTORY_FETCH_LIMIT: usize = HISTORY_RECORD_LIMIT + 1;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConversationHistory {
    messages: Vec<Message>,
    content_bytes: usize,
    omitted_messages_lower_bound: usize,
    source_truncated: bool,
}

impl ConversationHistory {
    #[must_use]
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    #[must_use]
    pub fn content_bytes(&self) -> usize {
        self.content_bytes
    }

    #[must_use]
    pub fn omitted_messages_lower_bound(&self) -> usize {
        self.omitted_messages_lower_bound
    }

    #[must_use]
    pub fn source_truncated(&self) -> bool {
        self.source_truncated
    }

    #[must_use]
    pub fn into_messages(self) -> Vec<Message> {
        self.messages
    }
}

pub struct ConversationHistoryBridge {
    store: Arc<dyn HubStore>,
}

impl ConversationHistoryBridge {
    #[must_use]
    pub fn new(store: Arc<dyn HubStore>) -> Self {
        Self { store }
    }

    /// Loads messages strictly before the named current user Prompt.
    ///
    /// This is the history shape consumed by `AgentRuntime::run_with_history`:
    /// the runtime appends the current `RunRequest.prompt` exactly once.
    ///
    /// # Errors
    ///
    /// Fails if the Prompt is not in the Conversation, if that boundary is not
    /// a `user` record, or if any retained record has an unsupported role.
    pub fn load_before(
        &self,
        conversation_id: &str,
        current_prompt_id: &str,
        max_content_bytes: usize,
    ) -> Result<ConversationHistory, HistoryError> {
        if current_prompt_id.trim().is_empty() {
            return Err(HistoryError::EmptyPromptId);
        }
        if conversation_id.trim().is_empty() {
            return Err(HistoryError::EmptyConversationId);
        }
        let mut records = self.store.list_prompts_before(
            conversation_id,
            current_prompt_id,
            HISTORY_FETCH_LIMIT,
        )?;
        validate_conversation_ids(&records, conversation_id)?;
        let source_truncated = records.len() > HISTORY_RECORD_LIMIT;
        records.reverse();
        let records = retain_causal_limit(records, HISTORY_RECORD_LIMIT);
        build_history(records, max_content_bytes, source_truncated)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HistoryError {
    #[error("conversation id must not be empty")]
    EmptyConversationId,
    #[error("current prompt id must not be empty")]
    EmptyPromptId,
    #[error("prompt '{prompt_id}' has unsupported persisted role '{role}'")]
    UnsupportedRole { prompt_id: String, role: String },
    #[error("prompt '{prompt_id}' belongs to conversation '{actual}', expected '{expected}'")]
    RecordConversationMismatch {
        prompt_id: String,
        expected: String,
        actual: String,
    },
    #[error("conversation history store failed: {0}")]
    Store(#[from] HubStoreError),
}

fn build_history(
    records: Vec<PromptRecord>,
    max_content_bytes: usize,
    source_truncated: bool,
) -> Result<ConversationHistory, HistoryError> {
    let messages = records
        .into_iter()
        .map(map_record)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(trim_history(&messages, max_content_bytes, source_truncated))
}

fn retain_causal_limit(records: Vec<PromptRecord>, limit: usize) -> Vec<PromptRecord> {
    if records.len() <= limit {
        return records;
    }
    if limit == 0 {
        return Vec::new();
    }
    let tail_start = records.len() - limit;
    if records[tail_start].role == "user" {
        return records[tail_start..].to_vec();
    }
    let Some(source) = records[..tail_start]
        .iter()
        .rposition(|record| record.role == "user")
    else {
        return records[tail_start..].to_vec();
    };
    let newest_start = records.len() - (limit - 1);
    let mut retained = Vec::with_capacity(limit);
    retained.push(records[source].clone());
    retained.extend_from_slice(&records[newest_start..]);
    retained
}

fn map_record(record: PromptRecord) -> Result<Message, HistoryError> {
    match record.role.as_str() {
        "user" => Ok(Message::User {
            text: record.content,
        }),
        "assistant" => Ok(Message::Assistant {
            text: record.content,
            tool_calls: Vec::new(),
        }),
        _ => Err(HistoryError::UnsupportedRole {
            prompt_id: record.id,
            role: record.role,
        }),
    }
}

fn trim_history(
    messages: &[Message],
    max_content_bytes: usize,
    source_truncated: bool,
) -> ConversationHistory {
    let latest_user = messages
        .iter()
        .rposition(|message| matches!(message, Message::User { .. }));
    let range = discard_orphaned_prefix(
        messages,
        retained_range(messages, latest_user, max_content_bytes),
    );
    let omitted_messages_lower_bound = messages
        .len()
        .saturating_sub(range.len())
        .saturating_add(usize::from(source_truncated));
    let messages = messages[range].to_vec();
    let content_bytes = messages.iter().map(message_bytes).sum();
    ConversationHistory {
        messages,
        content_bytes,
        omitted_messages_lower_bound,
        source_truncated,
    }
}

fn discard_orphaned_prefix(messages: &[Message], mut range: Range<usize>) -> Range<usize> {
    while range.start < range.end && !matches!(messages[range.start], Message::User { .. }) {
        range.start += 1;
    }
    range
}

fn retained_range(
    messages: &[Message],
    latest_user: Option<usize>,
    max_content_bytes: usize,
) -> Range<usize> {
    if max_content_bytes == 0 {
        return 0..0;
    }
    let Some(user) = latest_user else {
        return newest_suffix(messages, max_content_bytes);
    };
    let user_bytes = message_bytes(&messages[user]);
    if user_bytes > max_content_bytes {
        return 0..0;
    }
    let mut range = user..user + 1;
    let mut remaining = max_content_bytes - user_bytes;
    while range.end < messages.len() {
        let bytes = message_bytes(&messages[range.end]);
        if bytes > remaining {
            break;
        }
        remaining -= bytes;
        range.end += 1;
    }
    while range.start > 0 {
        let bytes = message_bytes(&messages[range.start - 1]);
        if bytes > remaining {
            break;
        }
        remaining -= bytes;
        range.start -= 1;
    }
    range
}

fn newest_suffix(messages: &[Message], max_content_bytes: usize) -> Range<usize> {
    let mut start = messages.len();
    let mut remaining = max_content_bytes;
    while start > 0 {
        let bytes = message_bytes(&messages[start - 1]);
        if bytes > remaining {
            break;
        }
        remaining -= bytes;
        start -= 1;
    }
    start..messages.len()
}

fn message_bytes(message: &Message) -> usize {
    match message {
        Message::User { text } | Message::Assistant { text, .. } => text.len(),
        Message::Tool { output, .. } => output.len(),
        Message::ProviderContext { .. } => {
            serde_json::to_vec(message).map_or(usize::MAX, |encoded| encoded.len())
        }
    }
}

fn validate_conversation_ids(
    records: &[PromptRecord],
    conversation_id: &str,
) -> Result<(), HistoryError> {
    if let Some(record) = records
        .iter()
        .find(|record| record.conversation_id != conversation_id)
    {
        return Err(HistoryError::RecordConversationMismatch {
            prompt_id: record.id.clone(),
            expected: conversation_id.into(),
            actual: record.conversation_id.clone(),
        });
    }
    Ok(())
}
