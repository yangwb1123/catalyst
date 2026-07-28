use crate::{PromptRecord, RunInspection, RunLimits, RuntimeEvent};

pub const RUN_STORE_VERSION: u16 = 1;
pub const MAX_RUN_LIST_LIMIT: usize = 1_000;
pub const MAX_RUN_EVENTS: usize = 8_192;
pub const MAX_RUN_EVENT_JSON_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_RUN_JOURNAL_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_RUN_EXECUTION_JSON_BYTES: usize = 64 * 1024;
pub const MAX_RUN_CURSOR_JSON_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunProvider {
    DeterministicRead { path: String },
    OpenAiResponses { endpoint: String, model: String },
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct RunExecution {
    pub provider: RunProvider,
    pub system_prompt: String,
    pub allowed_read_paths: Vec<String>,
    pub limits: RunLimits,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BeginRun {
    pub v: u16,
    pub run_id: String,
    pub conversation_id: String,
    pub prompt_id: String,
    pub project_id: String,
    pub execution: RunExecution,
    pub idempotency_key: String,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct RunRecord {
    pub v: u16,
    pub run_id: String,
    pub conversation_id: String,
    pub prompt_id: String,
    pub project_id: String,
    pub execution: RunExecution,
    pub protocol_version: u16,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct BoundRunPrompt {
    pub v: u16,
    pub prompt_id: String,
    pub conversation_id: String,
    pub content: String,
    pub created_at_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BeginRunDisposition {
    Created,
    Replayed,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct BeginRunResult {
    pub v: u16,
    pub disposition: BeginRunDisposition,
    pub run: RunRecord,
    pub prompt: BoundRunPrompt,
}

pub trait RunStore: Send + Sync {
    /// Creates a durable Run intent bound to an existing Project Conversation
    /// and one of that Conversation's user Prompts.
    ///
    /// An identical idempotency-key replay returns the original record.
    ///
    /// # Errors
    ///
    /// Returns a structured error for a missing or mismatched entity, an
    /// idempotency conflict, an unsupported version, or unavailable storage.
    fn begin_run(&self, request: &BeginRun) -> Result<BeginRunResult, RunStoreError>;

    /// Finds one Run by the retry key without creating execution intent.
    ///
    /// # Errors
    ///
    /// Returns a structured error when a matching record is corrupt or storage
    /// cannot be read.
    fn find_run_by_idempotency_key(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<RunRecord>, RunStoreError>;

    /// Appends one event to the Run journal.
    ///
    /// Events must be contiguous and match the Run envelope. An exact replay
    /// of a committed sequence is accepted; every divergent replay fails.
    ///
    /// # Errors
    ///
    /// Returns a structured error when the append violates the journal
    /// contract or cannot be committed.
    fn append_event(&self, event: &RuntimeEvent) -> Result<(), RunStoreError>;

    /// Loads and validates the complete durable prefix for one Run.
    ///
    /// This operation only inspects recovery state. It never invokes a tool or
    /// resumes model execution.
    ///
    /// # Errors
    ///
    /// Returns a structured error when the Run is missing, corrupt, or cannot
    /// be read.
    fn inspect_run(&self, run_id: &str) -> Result<RunInspection, RunStoreError>;

    /// Lists Runs newest first, optionally restricted to one Conversation.
    ///
    /// # Errors
    ///
    /// Returns a structured error for a missing Conversation, an invalid
    /// limit, corrupt records, or unavailable storage.
    fn list_runs(
        &self,
        conversation_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<RunRecord>, RunStoreError>;

    /// Atomically derives and writes the assistant Prompt from a completed Run.
    ///
    /// # Errors
    ///
    /// Returns an error unless the validated journal is terminal-completed and
    /// its answer is bound to this Run and Conversation.
    fn reconcile_completed_assistant(&self, run_id: &str) -> Result<PromptRecord, RunStoreError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunEntity {
    Project,
    Conversation,
    Prompt,
    Run,
    Event,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RunStoreError {
    NotFound { entity: RunEntity, id: String },
    Conflict { entity: RunEntity, message: String },
    Unavailable { message: String },
    Corrupt { message: String },
}

impl std::fmt::Display for RunStoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { entity, id } => write!(formatter, "{entity:?} '{id}' was not found"),
            Self::Conflict { entity, message } => write!(formatter, "{entity:?}: {message}"),
            Self::Unavailable { message } => write!(formatter, "run store unavailable: {message}"),
            Self::Corrupt { message } => write!(formatter, "run store is corrupt: {message}"),
        }
    }
}

impl std::error::Error for RunStoreError {}
