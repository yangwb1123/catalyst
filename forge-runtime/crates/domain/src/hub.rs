#[path = "hub_owned.rs"]
mod owned;
pub use owned::*;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkspaceIdentity {
    Unix {
        device: u64,
        inode: u64,
    },
    Windows {
        volume_serial_number: u32,
        file_index: u64,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum ConversationScope {
    Global,
    Project(String),
    Group(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Conversation {
    pub id: String,
    pub scope: ConversationScope,
    pub title: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PromptRecord {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub idempotency_key: String,
    pub created_at_ms: u64,
}

/// Maximum UTF-8 Prompt body size admitted by the Hub and bridge page budget.
pub const MAX_PROMPT_CONTENT_BYTES: usize = 256 * 1024;
/// Maximum UTF-8 length for Hub entity identifiers.
pub const MAX_HUB_ENTITY_ID_BYTES: usize = 128;
/// Maximum UTF-8 length for a stored Prompt role label.
pub const MAX_HUB_ROLE_BYTES: usize = 64;
/// Maximum number of Prompt rows returned by one replicated history page.
pub const MAX_CONVERSATION_PROMPT_PAGE_LIMIT: usize = 128;
/// Maximum aggregate UTF-8 Prompt body bytes preloaded by one history page.
pub const MAX_CONVERSATION_PROMPT_PAGE_CONTENT_BYTES: usize = MAX_PROMPT_CONTENT_BYTES;
/// Maximum number of transcript rows copied by one explicit Conversation import.
pub const MAX_CONVERSATION_IMPORT_PROMPT_COUNT: usize = 128;

/// Maximum number of Conversation metadata records returned per bootstrap page.
pub const MAX_CONVERSATION_BOOTSTRAP_PAGE_LIMIT: usize = 128;
/// Maximum number of owner-visible Conversations returned by one page.
pub const MAX_OWNED_CONVERSATION_PAGE_LIMIT: usize = 128;
/// Maximum Run summaries returned in one owner-scoped page.
pub const MAX_OWNED_RUN_PAGE_LIMIT: usize = 25;
/// Maximum payload-free timeline markers returned in one owner-scoped page.
pub const MAX_OWNED_RUN_TIMELINE_PAGE_LIMIT: usize = 128;
/// Maximum pending-intent summaries returned for one Conversation.
pub const MAX_PENDING_RUN_INTENT_PAGE_LIMIT: usize = 25;
/// Maximum payload-free pending-intent timeline markers returned per read.
pub const MAX_PENDING_RUN_INTENT_TIMELINE_PAGE_LIMIT: usize = 128;
pub const MAX_CONVERSATION_OWNER_ISSUER_BYTES: usize = 2048;
pub const MAX_CONVERSATION_OWNER_SUBJECT_BYTES: usize = 255;
pub const MAX_CONVERSATION_OWNER_TENANT_BYTES: usize = 256;
/// Project execution consent cannot remain live for more than thirty days.
pub const MAX_PROJECT_EXECUTION_CONSENT_TTL_MS: u64 = 30 * 24 * 60 * 60 * 1_000;

/// Prompt projection for local session-history consumers; idempotency keys are excluded.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConversationPrompt {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub created_at_ms: u64,
}

/// Exclusive keyset position in newest-first Conversation Prompt history.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationPromptCursor {
    pub created_at_ms: u64,
    pub prompt_id: String,
}

/// Bounded newest-first page of Prompt history for exactly one Conversation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConversationPromptPage {
    pub conversation_id: String,
    pub prompts: Vec<ConversationPrompt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<ConversationPromptCursor>,
    pub has_more: bool,
}

/// Stable continuation phase for a frozen-head Conversation bootstrap.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationBootstrapPhase {
    LegacyBaseline,
    ChangeLog,
}

/// Continuation position bound to the journal head captured on page one.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationBootstrapCursor {
    pub snapshot_cursor: u64,
    pub phase: ConversationBootstrapPhase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_conversation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_change_cursor: Option<u64>,
}

/// Sanitized Conversation metadata plus its creation and observed aggregate versions.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationBootstrapEntry {
    pub conversation: Conversation,
    pub creation_cursor: u64,
    pub aggregate_version: u64,
}

/// Bounded Conversation metadata bootstrap through one fixed Hub change head.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationBootstrapPage {
    pub snapshot_cursor: u64,
    pub conversations: Vec<ConversationBootstrapEntry>,
    /// Last journal cursor scanned; zero for baseline pages.
    pub scanned_through_cursor: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<ConversationBootstrapCursor>,
    pub has_more: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SessionGroup {
    pub id: String,
    pub name: String,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupProjectMember {
    pub group_id: String,
    pub project_id: String,
    pub role: String,
    pub added_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HubSnapshot {
    pub scope: ConversationScope,
    pub projects: Vec<Project>,
    pub conversations: Vec<Conversation>,
    pub groups: Vec<SessionGroup>,
    pub group_project_members: Vec<GroupProjectMember>,
}

/// Canonical Global Hub snapshot paired with the store-local change cursor
/// observed in the same storage read snapshot.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HubSnapshotAtCursor {
    pub snapshot: HubSnapshot,
    pub cursor: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationChangeKind {
    ConversationCreated,
    PromptAppended,
}

/// An ID-only reference to one committed Conversation or Prompt write.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConversationChange {
    pub cursor: u64,
    pub schema_version: u16,
    pub conversation_id: String,
    pub entity_id: String,
    /// Sequence among this Conversation's journaled changes; pre-v30 history
    /// is not represented in this version.
    pub aggregate_version: u64,
    pub kind: ConversationChangeKind,
    pub created_at_ms: u64,
}

/// A bounded page from the Hub-local, append-only Conversation change feed.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConversationChangePage {
    pub after_cursor: u64,
    pub next_cursor: u64,
    pub head_cursor: u64,
    pub has_more: bool,
    pub changes: Vec<ConversationChange>,
}

/// Owner-filtered page from the Conversation change journal. Only exact-owner
/// changes are returned, using a dense cursor local to that principal. The
/// cursor remains at `after_cursor` when the page is empty and never exposes
/// the Hub-global head or journal position.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OwnedConversationChangePage {
    pub after_cursor: u64,
    /// Last returned owner-local cursor, or `after_cursor` if the page is empty.
    pub scanned_through_cursor: u64,
    pub has_more: bool,
    pub changes: Vec<ConversationChange>,
}

pub const MAX_CONVERSATION_CHANGE_PAGE_LIMIT: usize = 128;
use std::{path::Path, sync::Arc};

use crate::{
    Cancellation,
    tool::{ToolError, ToolOutput},
};

pub trait WorkspaceReader: Send + Sync {
    /// Reads one file relative to the anchored workspace.
    ///
    /// # Errors
    ///
    /// Returns a tool error when the path is denied, the file cannot be read,
    /// or the output exceeds `max_bytes`.
    fn read_file(
        &self,
        relative: &Path,
        max_bytes: usize,
        cancellation: &Cancellation,
    ) -> Result<ToolOutput, ToolError>;
}

#[derive(Clone)]
pub struct WorkspaceReadCapability {
    reader: Arc<dyn WorkspaceReader>,
    identity: Option<WorkspaceIdentity>,
}

impl WorkspaceReadCapability {
    #[must_use]
    pub fn new(reader: Arc<dyn WorkspaceReader>, identity: Option<WorkspaceIdentity>) -> Self {
        Self { reader, identity }
    }

    #[must_use]
    pub const fn workspace_identity(&self) -> Option<&WorkspaceIdentity> {
        self.identity.as_ref()
    }

    /// Reads one workspace-relative file through the anchored capability.
    ///
    /// # Errors
    ///
    /// Returns a tool error when the path is denied, the file cannot be read,
    /// or the output exceeds the requested bound.
    pub fn read_file(
        &self,
        relative: &Path,
        max_bytes: usize,
        cancellation: &Cancellation,
    ) -> Result<ToolOutput, ToolError> {
        self.reader.read_file(relative, max_bytes, cancellation)
    }
}

pub trait WorkspaceReadFactory: Send + Sync {
    /// Opens one anchored read capability for a workspace path.
    ///
    /// # Errors
    ///
    /// Returns an error when the workspace cannot be opened.
    fn open(&self, workspace: &Path) -> Result<WorkspaceReadCapability, WorkspaceOpenError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceOpenError {
    message: String,
}

impl WorkspaceOpenError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for WorkspaceOpenError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for WorkspaceOpenError {}
