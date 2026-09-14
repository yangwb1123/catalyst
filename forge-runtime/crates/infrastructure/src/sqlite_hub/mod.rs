mod atomic_link;
mod change_read;
mod change_write;
mod conversation_bootstrap_read;
mod conversation_import_read;
mod conversation_owner_changes_read;
mod conversation_owner_read;
mod conversation_project_read;
#[cfg(test)]
#[path = "tests/error_classification.rs"]
mod error_classification_tests;
mod governance_record_journal;
mod group_agent_graph;
mod group_agent_graph_run;
mod group_agent_node_execution_contract;
mod group_agent_node_lifecycle;
mod group_agent_scheduled_node_contract;
mod group_agent_scheduled_node_lifecycle;
mod group_agent_scheduled_node_provider_request;
mod group_agent_scheduled_node_successor;
mod group_analysis_panel;
mod group_context_build;
mod group_context_read;
#[cfg(test)]
#[path = "tests/group_context_snapshot.rs"]
mod group_context_snapshot_tests;
mod group_context_validate;
mod group_execution_codec;
mod group_execution_read;
mod group_execution_write;
mod group_model_analysis;
mod group_panel_synthesis;
mod group_run_codec;
mod group_run_read;
mod group_run_write;
mod open;
mod owned_run_read;
mod pending_run_intent;
mod project_execution_consent;
mod prompt_read;
mod read;
mod rows;
mod run_execution_lock;
mod run_integrity;
mod run_lineage_read;
mod run_lineage_write;
#[cfg(test)]
#[path = "tests/run_prompt_atomicity.rs"]
mod run_prompt_atomicity_tests;
mod run_read;
#[cfg(test)]
#[path = "tests/run_read_snapshot.rs"]
mod run_read_snapshot_tests;
mod run_seed;
mod run_write;
mod run_writeback;
mod scheduled_graph_controller;
mod scheduled_graph_progress;
mod schema;
mod schema_hard_link;
#[cfg(test)]
mod schema_migration_tests;
mod schema_sql;
mod schema_v10_sql;
mod schema_v11_sql;
#[path = "schema_contract/v12_sql.rs"]
mod schema_v12_sql;
#[path = "schema_contract/v13_sql.rs"]
mod schema_v13_sql;
#[path = "schema_contract/v14_sql.rs"]
mod schema_v14_sql;
#[path = "schema_contract/v15_sql.rs"]
mod schema_v15_sql;
#[path = "schema_contract/v16_sql.rs"]
mod schema_v16_sql;
#[path = "schema_contract/v17_sql.rs"]
mod schema_v17_sql;
#[path = "schema_contract/v18_sql.rs"]
mod schema_v18_sql;
#[path = "schema_contract/v19_sql.rs"]
mod schema_v19_sql;
#[path = "schema_contract/v20_sql.rs"]
mod schema_v20_sql;
#[path = "schema_contract/v21_sql.rs"]
mod schema_v21_sql;
#[path = "schema_contract/v22_sql.rs"]
mod schema_v22_sql;
#[path = "schema_contract/v23_sql.rs"]
mod schema_v23_sql;
#[path = "schema_contract/v24_sql.rs"]
mod schema_v24_sql;
#[path = "schema_contract/v25_sql.rs"]
mod schema_v25_sql;
#[path = "schema_contract/v26_sql.rs"]
mod schema_v26_sql;
#[path = "schema_contract/v27_sql.rs"]
mod schema_v27_sql;
#[path = "schema_contract/v28_sql.rs"]
mod schema_v28_sql;
#[path = "schema_contract/v29_sql.rs"]
mod schema_v29_sql;
#[path = "schema_contract/v30_sql.rs"]
mod schema_v30_sql;
#[path = "schema_contract/v31_sql.rs"]
mod schema_v31_sql;
#[path = "schema_contract/v32_sql.rs"]
mod schema_v32_sql;
#[path = "schema_contract/v33_sql.rs"]
mod schema_v33_sql;
#[path = "schema_contract/v34_sql.rs"]
mod schema_v34_sql;
mod schema_v9_sql;
mod store_impl;
mod write;

use std::path::{Path, PathBuf};

pub(super) use crate::runtime_domain::{
    BeginGroupExecution, BeginGroupExecutionResult, BeginRun, BeginRunBranch, BeginRunBranchResult,
    BeginRunResult, BeginRunWithPrompt, Conversation, ConversationBootstrapCursor,
    ConversationBootstrapEntry, ConversationBootstrapPage, ConversationBootstrapPhase,
    ConversationChange, ConversationChangeKind, ConversationChangePage, ConversationImportPrompt,
    ConversationOwner, ConversationPrompt, ConversationPromptCursor, ConversationPromptPage,
    ConversationScope, GroupContextPolicy, GroupContextSlice, GroupExecutionEvent,
    GroupExecutionInspection, GroupExecutionRecord, GroupExecutionStore, GroupProjectMember,
    GroupRunRecord, GroupRunSnapshot, GroupRunStore, HubEntity, HubSnapshot, HubSnapshotAtCursor,
    HubStore, HubStoreError, LocalConversationImportSource, MAX_CONVERSATION_BOOTSTRAP_PAGE_LIMIT,
    MAX_CONVERSATION_CHANGE_PAGE_LIMIT, MAX_CONVERSATION_IMPORT_PROMPT_COUNT,
    MAX_CONVERSATION_PROMPT_PAGE_CONTENT_BYTES, MAX_CONVERSATION_PROMPT_PAGE_LIMIT,
    MAX_HUB_ENTITY_ID_BYTES, MAX_HUB_ROLE_BYTES, MAX_OWNED_CONVERSATION_PAGE_LIMIT,
    MAX_OWNED_RUN_PAGE_LIMIT, MAX_OWNED_RUN_TIMELINE_PAGE_LIMIT, MAX_PROMPT_CONTENT_BYTES,
    OwnedConversationChangePage, OwnedConversationEntry, OwnedConversationImportResult,
    OwnedConversationPage, OwnedProjectConversationIdentity, OwnedPromptAppendResult,
    OwnedRunCursor, OwnedRunPage, OwnedRunStatus, OwnedRunSummary, OwnedRunTimelineEvent,
    OwnedRunTimelineEventType, OwnedRunTimelinePage, PendingRunIntentCursor, PendingRunIntentPage,
    PendingRunIntentSubmissionResult, PendingRunIntentTimelinePage, PrepareGroupRun,
    PrepareGroupRunResult, Project, ProjectExecutionConsentGrantResult,
    ProjectExecutionConsentRevocationResult, PromptRecord, RunInspection, RunLineageRecord,
    RunRecord, RunStore, RunStoreError, RuntimeEvent, SessionGroup, SubmitPendingRunIntent,
};
use rusqlite::{Connection, Error as SqliteError, ErrorCode};

use open::SqliteHubStoreOpenMode;
pub use run_execution_lock::RunExecutionGuard;

#[derive(Clone, Debug)]
pub struct SqliteHubStore {
    database_path: PathBuf,
    open_mode: SqliteHubStoreOpenMode,
}

/// Current hub schema version (the version `open` migrates to).
pub const CURRENT_SCHEMA_VERSION: i64 = schema::SCHEMA_VERSION;

/// Reads the stored schema version of an existing hub WITHOUT migrating or
/// creating it — the readiness probe primitive (Stage-06 High follow-up).
///
/// # Errors
///
/// Returns a store error when the database cannot be opened or the
/// pragma cannot be read.
pub fn hub_schema_version(path: &Path) -> Result<i64, HubStoreError> {
    if !path.exists() {
        return Ok(0);
    }
    let connection = Connection::open(path).map_err(schema::sqlite_error)?;
    let version = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(schema::sqlite_error)?;
    Ok(version)
}

impl GroupRunStore for SqliteHubStore {
    fn prepare_group_run(
        &self,
        request: &PrepareGroupRun,
    ) -> Result<PrepareGroupRunResult, HubStoreError> {
        group_run_write::prepare(&mut self.connect()?, request)
    }

    fn inspect_group_run(&self, run_id: &str) -> Result<GroupRunSnapshot, HubStoreError> {
        group_run_read::inspect(&self.connect()?, run_id)
    }

    fn list_group_runs(
        &self,
        group_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupRunRecord>, HubStoreError> {
        group_run_read::list(&self.connect()?, group_id, limit)
    }
}

impl GroupExecutionStore for SqliteHubStore {
    fn begin_group_execution(
        &self,
        request: &BeginGroupExecution,
    ) -> Result<BeginGroupExecutionResult, HubStoreError> {
        group_execution_write::begin(&mut self.connect()?, request)
    }

    fn append_group_execution_event(
        &self,
        event: &GroupExecutionEvent,
    ) -> Result<(), HubStoreError> {
        group_execution_write::append(&mut self.connect()?, event)
    }

    fn inspect_group_execution(
        &self,
        execution_id: &str,
    ) -> Result<GroupExecutionInspection, HubStoreError> {
        group_execution_read::inspect(&mut self.connect()?, execution_id)
    }

    fn list_group_executions(
        &self,
        group_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupExecutionRecord>, HubStoreError> {
        group_execution_read::list(&self.connect()?, group_run_id, limit)
    }
}

impl RunStore for SqliteHubStore {
    fn begin_run_with_prompt(
        &self,
        request: &BeginRunWithPrompt,
    ) -> Result<BeginRunResult, RunStoreError> {
        let mut connection = self.connect_run()?;
        run_seed::begin(&mut connection, request)
    }

    fn begin_run(&self, request: &BeginRun) -> Result<BeginRunResult, RunStoreError> {
        let mut connection = self.connect_run()?;
        run_write::begin_run(&mut connection, request)
    }

    fn begin_run_branch(
        &self,
        request: &BeginRunBranch,
    ) -> Result<BeginRunBranchResult, RunStoreError> {
        run_lineage_write::begin(&mut self.connect_run()?, request)
    }

    fn find_run_lineage(&self, run_id: &str) -> Result<Option<RunLineageRecord>, RunStoreError> {
        run_lineage_read::find_validated(&mut self.connect_run()?, run_id)
    }

    fn append_event(&self, event: &RuntimeEvent) -> Result<(), RunStoreError> {
        let mut connection = self.connect_run()?;
        run_write::append_event(&mut connection, event)
    }

    fn find_run_by_idempotency_key(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<RunRecord>, RunStoreError> {
        run_read::record_by_key(&self.connect_run()?, idempotency_key)
    }

    fn inspect_run(&self, run_id: &str) -> Result<RunInspection, RunStoreError> {
        run_read::inspect_transaction(&mut self.connect_run()?, run_id)
    }

    fn list_runs(
        &self,
        conversation_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<RunRecord>, RunStoreError> {
        run_read::list_runs(&self.connect_run()?, conversation_id, limit)
    }

    fn reconcile_completed_assistant(&self, run_id: &str) -> Result<PromptRecord, RunStoreError> {
        let mut connection = self.connect_run()?;
        run_writeback::reconcile_completed_assistant(&mut connection, run_id)
    }
}

impl SqliteHubStore {
    fn connect_run(&self) -> Result<Connection, RunStoreError> {
        self.connect().map_err(run_error_from_hub)
    }
}

fn write_error(entity: HubEntity, error: SqliteError) -> HubStoreError {
    match &error {
        SqliteError::SqliteFailure(problem, message)
            if problem.code == ErrorCode::ConstraintViolation =>
        {
            HubStoreError::Conflict {
                entity,
                message: message.clone().unwrap_or_else(|| problem.to_string()),
            }
        }
        _ => schema::sqlite_error(error),
    }
}

fn read_error(error: SqliteError) -> HubStoreError {
    match error {
        SqliteError::FromSqlConversionFailure(..)
        | SqliteError::InvalidColumnType(..)
        | SqliteError::IntegralValueOutOfRange(..) => HubStoreError::Corrupt {
            message: error.to_string(),
        },
        _ => schema::sqlite_error(error),
    }
}

fn unavailable(error: impl std::fmt::Display) -> HubStoreError {
    HubStoreError::Unavailable {
        message: error.to_string(),
    }
}

fn run_error_from_hub(error: HubStoreError) -> RunStoreError {
    match error {
        HubStoreError::Unavailable { message } => RunStoreError::Unavailable { message },
        HubStoreError::Corrupt { message } => RunStoreError::Corrupt { message },
        HubStoreError::NotFound { .. } | HubStoreError::Conflict { .. } => {
            RunStoreError::Unavailable {
                message: error.to_string(),
            }
        }
    }
}
