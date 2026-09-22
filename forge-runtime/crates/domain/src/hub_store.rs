use std::path::Path;

use crate::{
    Conversation, ConversationBootstrapCursor, ConversationBootstrapPage, ConversationChangePage,
    ConversationImportPrompt, ConversationOwner, ConversationPromptCursor, ConversationPromptPage,
    ConversationScope, GroupContextPolicy, GroupContextSlice, GroupProjectMember, HubSnapshot,
    HubSnapshotAtCursor, LocalConversationImportSource, OwnedConversationChangePage,
    OwnedConversationEntry, OwnedConversationImportResult, OwnedConversationPage,
    OwnedProjectConversationIdentity, OwnedPromptAppendResult, OwnedRunCursor, OwnedRunPage,
    OwnedRunSummary, OwnedRunTimelinePage, PendingRunIntentCursor, PendingRunIntentPage,
    PendingRunIntentSubmissionResult, PendingRunIntentTimelinePage, Project,
    ProjectExecutionConsentGrantResult, ProjectExecutionConsentRevocationResult, PromptRecord,
    SessionGroup, SubmitPendingRunIntent,
};

#[macro_use]
#[path = "hub_store_owned_methods.rs"]
mod owned_methods;

pub trait HubStore: Send + Sync {
    owned_conversation_methods!();

    /// Finds or registers the project anchored at an absolute path.
    ///
    /// # Errors
    ///
    /// Returns a structured storage error when the operation cannot complete.
    fn open_project(&self, absolute_path: &Path) -> Result<Project, HubStoreError>;

    /// Loads the visible hub state for one scope.
    ///
    /// # Errors
    ///
    /// Returns a structured storage error when the snapshot cannot be loaded.
    fn snapshot(&self, scope: &ConversationScope) -> Result<HubSnapshot, HubStoreError>;

    /// Loads the canonical Global snapshot and store-global feed head observed
    /// in one storage read snapshot. Project and Group snapshots have no scoped
    /// change feed in this version.
    ///
    /// # Errors
    ///
    /// Returns a structured storage error when either read cannot complete.
    fn snapshot_at_cursor(&self) -> Result<HubSnapshotAtCursor, HubStoreError>;

    /// Reads a bounded, ordered page of changes strictly after a local cursor.
    ///
    /// Implementations must reject cursors beyond the observed head and detect
    /// gaps because this v1 journal has no retention or deletion operation.
    ///
    /// # Errors
    ///
    /// Returns a structured storage error for invalid cursors, gaps, or corrupt
    /// journal rows.
    fn conversation_changes_after(
        &self,
        after_cursor: u64,
        limit: usize,
    ) -> Result<ConversationChangePage, HubStoreError>;

    /// Reads one bounded page of Conversation metadata through a frozen change
    /// head. Implementations page legacy baselines by binary ID, then page
    /// journal rows by cursor and hydrate only Conversation-created events.
    /// Prompt-only pages may be empty while advancing the journal cursor.
    ///
    /// # Errors
    ///
    /// Returns a structured storage error for invalid cursors, gaps, or corrupt
    /// baseline/change rows.
    fn conversation_bootstrap_page(
        &self,
        cursor: Option<&ConversationBootstrapCursor>,
        limit: usize,
    ) -> Result<ConversationBootstrapPage, HubStoreError>;

    /// Creates a conversation, or returns an identical prior result for the key.
    ///
    /// # Errors
    ///
    /// Returns a conflict if the key is reused with different request data.
    fn create_conversation(
        &self,
        scope: &ConversationScope,
        title: &str,
        idempotency_key: &str,
    ) -> Result<Conversation, HubStoreError>;

    /// Lists conversations visible in a scope; global scope spans all conversations.
    ///
    /// # Errors
    ///
    /// Returns a structured storage error when the list cannot be loaded.
    fn list_conversations(
        &self,
        scope: &ConversationScope,
    ) -> Result<Vec<Conversation>, HubStoreError>;

    /// Appends a durable prompt, idempotently for identical request data.
    ///
    /// # Errors
    ///
    /// Returns a conflict if the key is reused with different request data.
    fn append_prompt(
        &self,
        conversation_id: &str,
        role: &str,
        content: &str,
        idempotency_key: &str,
    ) -> Result<PromptRecord, HubStoreError>;

    /// Lists prompts newest first, breaking timestamp ties by descending record id.
    ///
    /// `None` spans every conversation visible to the store. A conversation-specific
    /// query returns `NotFound` when that Conversation does not exist.
    ///
    /// # Errors
    ///
    /// Returns a structured storage error when prompt history cannot be loaded.
    fn list_prompts(
        &self,
        conversation_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<PromptRecord>, HubStoreError>;

    /// Reads one newest-first keyset page of sanitized Prompt history.
    ///
    /// The page is scoped to one Conversation, excludes the supplied `(time, id)`
    /// cursor, and must enforce both row-count and aggregate content-byte bounds
    /// before loading Prompt bodies. Idempotency keys are not part of the result.
    ///
    /// # Errors
    ///
    /// Returns `NotFound` for an absent Conversation and `Corrupt` for rows that
    /// cannot fit the frozen page contract.
    fn conversation_prompt_page(
        &self,
        conversation_id: &str,
        before: Option<&ConversationPromptCursor>,
        limit: usize,
    ) -> Result<ConversationPromptPage, HubStoreError>;

    /// Reads one bounded local ownerless Conversation transcript for explicit import.
    /// Only user/assistant text is returned; project paths and execution records are excluded.
    ///
    /// # Errors
    ///
    /// Returns `NotFound` for an absent source, `Conflict` when the source is
    /// owner-bound or exceeds import limits, and `Unavailable` when previews
    /// are unsupported by this Hub store.
    fn local_conversation_import_source(
        &self,
        conversation_id: &str,
    ) -> Result<LocalConversationImportSource, HubStoreError> {
        let _ = conversation_id;
        Err(HubStoreError::Unavailable {
            message: "local Conversation import previews are not supported by this Hub store"
                .into(),
        })
    }

    /// Lists records strictly before one user Prompt in the same Conversation.
    ///
    /// Ordering is newest causal record first. An assistant Prompt associated
    /// with a Run is placed immediately after that Run's bound user Prompt,
    /// independent of delayed recovery/writeback time. Boundary validation and
    /// the read share one storage snapshot. The global record budget keeps
    /// newer causal groups first and reserves the cutoff group's source before
    /// admitting the newest answers that still fit.
    ///
    /// # Errors
    ///
    /// Returns `NotFound` when the Conversation or boundary Prompt is absent
    /// or mismatched, and `Conflict` when the boundary is not a user Prompt.
    fn list_prompts_before(
        &self,
        conversation_id: &str,
        boundary_prompt_id: &str,
        limit: usize,
    ) -> Result<Vec<PromptRecord>, HubStoreError>;

    /// Loads a deterministic, bounded dossier for one collaboration group.
    ///
    /// The result contains only committed Prompt history from the Group's own
    /// Conversations and its current member Projects. Implementations must
    /// resolve membership, Conversations, and Prompts from one read snapshot.
    ///
    /// # Errors
    ///
    /// Returns a structured error for a missing Group, invalid policy,
    /// corrupt provenance, unsupported Prompt roles, or unavailable storage.
    fn load_group_context(
        &self,
        group_id: &str,
        policy: &GroupContextPolicy,
    ) -> Result<GroupContextSlice, HubStoreError>;

    /// Creates a collaboration group, idempotently for identical request data.
    ///
    /// # Errors
    ///
    /// Returns a conflict if the key is reused with a different group name.
    fn create_group(
        &self,
        name: &str,
        idempotency_key: &str,
    ) -> Result<SessionGroup, HubStoreError>;

    /// Lists every collaboration group visible to the store.
    ///
    /// # Errors
    ///
    /// Returns a structured storage error when the list cannot be loaded.
    fn list_groups(&self) -> Result<Vec<SessionGroup>, HubStoreError>;

    /// Adds a project to a group with a descriptive collaboration role.
    ///
    /// # Errors
    ///
    /// Returns a conflict if a key or project link has different request data.
    fn add_project_to_group(
        &self,
        group_id: &str,
        project_id: &str,
        role: &str,
        idempotency_key: &str,
    ) -> Result<GroupProjectMember, HubStoreError>;

    /// Registers a project path and links it to a group in one atomic operation.
    ///
    /// # Errors
    ///
    /// Returns without registering the project if validation or linking fails.
    fn add_project_path_to_group(
        &self,
        group_id: &str,
        absolute_path: &Path,
        role: &str,
        idempotency_key: &str,
    ) -> Result<GroupProjectMember, HubStoreError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HubEntity {
    Project,
    ProjectExecutionConsent,
    PendingRunIntent,
    Conversation,
    Prompt,
    Group,
    GroupRun,
    GroupExecution,
    GroupModelAnalysis,
    GroupAnalysisPanel,
    GroupPanelSynthesis,
    GroupAgentGraph,
    GroupAgentGraphRun,
    GroupAgentGraphExecutionSchedule,
    GroupAgentScheduledNodeContract,
    GroupAgentScheduledNodeProviderRequest,
    GroupAgentScheduledNodeLifecycle,
    ScheduledGraphController,
    GroupAgentNodeExecutionContract,
    GroupAgentNodeDispatchRequest,
    GroupAgentNodeLifecycle,
    GovernanceRecord,
    GroupProjectMember,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HubStoreError {
    NotFound { entity: HubEntity, id: String },
    Conflict { entity: HubEntity, message: String },
    Unavailable { message: String },
    Corrupt { message: String },
}

impl std::fmt::Display for HubStoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { entity, id } => write!(formatter, "{entity:?} '{id}' was not found"),
            Self::Conflict { entity, message } => write!(formatter, "{entity:?}: {message}"),
            Self::Unavailable { message } => write!(formatter, "store unavailable: {message}"),
            Self::Corrupt { message } => write!(formatter, "store data is corrupt: {message}"),
        }
    }
}

impl std::error::Error for HubStoreError {}
