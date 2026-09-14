use serde::{Deserialize, Serialize};

use super::{Conversation, ConversationPrompt};

/// Verified identity used to bind a remote Conversation to one exact account.
/// The tuple is supplied only after resource-server token validation and is
/// persisted in Hub metadata; it is never part of the public Conversation DTO.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationOwner {
    pub issuer: String,
    pub subject: String,
    pub tenant_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OwnedConversationEntry {
    pub conversation: Conversation,
    pub aggregate_version: u64,
}

/// Stable ID-ordered owner-only Conversation page. The cursor does not expose
/// any Hub-global journal position or another principal's activity count.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OwnedConversationPage {
    pub conversations: Vec<OwnedConversationEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_after_id: Option<String>,
    pub has_more: bool,
}

/// Minimal trusted Project input derived from a Conversation owned by the
/// exact verified principal. This projection intentionally excludes its title,
/// path, and Prompt history.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedProjectConversationIdentity {
    pub conversation_id: String,
    pub project_id: String,
}

/// Sanitized status for one Run attached to an owner-visible Conversation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnedRunStatus {
    Nonterminal,
    Completed,
    Cancelled,
    LimitExceeded,
    Failed,
}

/// Exclusive keyset position for newest-first owner-scoped Run pages.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedRunCursor {
    pub created_at_ms: u64,
    pub run_id: String,
}

/// Scalar Run metadata only; execution and journal payloads are never exposed.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedRunSummary {
    pub run_id: String,
    pub prompt_id: String,
    pub created_at_ms: u64,
    pub latest_sequence: u64,
    pub status: OwnedRunStatus,
}

/// Newest-first page of Run summaries for one owner-visible Conversation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedRunPage {
    pub conversation_id: String,
    pub runs: Vec<OwnedRunSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<OwnedRunCursor>,
    pub has_more: bool,
}

/// Closed, payload-free `RuntimeEvent` type projection.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnedRunTimelineEventType {
    RunStarted,
    TurnStarted,
    Activity,
    RunFinished,
}

/// Sanitized timeline marker with no prompt, message, tool, or error content.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedRunTimelineEvent {
    pub seq: u64,
    pub emitted_at_ms: u64,
    #[serde(rename = "type")]
    pub event_type: OwnedRunTimelineEventType,
}

/// Bounded page of payload-free event markers for one owner-visible Run.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedRunTimelinePage {
    pub conversation_id: String,
    pub run_id: String,
    pub after_sequence: u64,
    pub scanned_through_sequence: u64,
    pub has_more: bool,
    pub events: Vec<OwnedRunTimelineEvent>,
}

/// Candidate Hub state for a remote Prompt that passed server-side consent
/// checks. This projection is deliberately distinct from a Run: it carries no
/// worker, provider, or execution state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PendingRunIntentStatus {
    Pending,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PendingRunIntent {
    pub intent_id: String,
    pub conversation_id: String,
    pub prompt_id: String,
    pub project_id: String,
    pub profile_id: String,
    pub submitted_at_ms: u64,
    pub aggregate_version: u64,
    pub latest_sequence: u64,
    pub status: PendingRunIntentStatus,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PendingRunIntentTimelineEventType {
    Submitted,
}

/// Payload-free event marker for one pending Run intent.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PendingRunIntentTimelineEvent {
    pub event_id: String,
    pub seq: u64,
    pub emitted_at_ms: u64,
    #[serde(rename = "type")]
    pub event_type: PendingRunIntentTimelineEventType,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PendingRunIntentSubmissionResult {
    pub prompt: ConversationPrompt,
    pub intent: PendingRunIntent,
    pub initial_event: PendingRunIntentTimelineEvent,
    pub replayed: bool,
}

/// Trusted internal command for creating one consent-checked pending intent.
/// It deliberately has no caller-supplied Project, path, provider, tool,
/// Runner, or credential fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubmitPendingRunIntent<'a> {
    pub conversation_id: &'a str,
    pub content: &'a str,
    pub idempotency_key: &'a str,
    pub expected_version: u64,
    pub profile_id: &'a str,
    pub profile_sha256: &'a [u8; 32],
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PendingRunIntentCursor {
    pub submitted_at_ms: u64,
    pub intent_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PendingRunIntentPage {
    pub conversation_id: String,
    pub intents: Vec<PendingRunIntent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<PendingRunIntentCursor>,
    pub has_more: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PendingRunIntentTimelinePage {
    pub conversation_id: String,
    pub intent_id: String,
    pub after_sequence: u64,
    pub scanned_through_sequence: u64,
    pub has_more: bool,
    pub events: Vec<PendingRunIntentTimelineEvent>,
}

/// Result of an owner-checked, version-guarded user Prompt append.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OwnedPromptAppendResult {
    pub prompt: ConversationPrompt,
    pub aggregate_version: u64,
    pub replayed: bool,
}

/// One user-visible Prompt copied from a local conversation during explicit import.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationImportPrompt {
    pub role: String,
    pub content: String,
}

/// Local-only sanitized source used to preview one explicit import.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalConversationImportSource {
    pub conversation: Conversation,
    /// Oldest-first, user-visible Prompt rows only.
    pub prompts: Vec<ConversationImportPrompt>,
    pub content_bytes: usize,
}

/// Result of importing one transcript atomically into an owner-scoped Conversation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OwnedConversationImportResult {
    pub conversation: Conversation,
    pub aggregate_version: u64,
    pub imported_prompt_count: usize,
    pub replayed: bool,
}

/// One immutable owner-authorized Project execution consent record.
///
/// The profile identifier and digest are opaque Hub metadata. The Hub does not
/// interpret them or execute the profile; a trusted control-plane caller owns
/// profile selection and meaning.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectExecutionConsentGrant {
    pub grant_id: String,
    pub project_id: String,
    pub profile_id: String,
    pub profile_sha256: [u8; 32],
    pub granted_at_ms: u64,
    pub expires_at_ms: u64,
}

/// Result of creating or exactly replaying one Project consent grant.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectExecutionConsentGrantResult {
    pub grant: ProjectExecutionConsentGrant,
    pub replayed: bool,
}

/// Immutable owner-scoped revocation event for a Project consent grant.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectExecutionConsentRevocation {
    pub event_id: String,
    pub grant_id: String,
    pub revoked_at_ms: u64,
}

/// Result of creating or exactly replaying one consent revocation event.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectExecutionConsentRevocationResult {
    pub revocation: ProjectExecutionConsentRevocation,
    pub replayed: bool,
}
