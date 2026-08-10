mod atomic_link;
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
mod read;
mod rows;
mod run_integrity;
mod run_read;
#[cfg(test)]
#[path = "tests/run_read_snapshot.rs"]
mod run_read_snapshot_tests;
mod run_write;
mod run_writeback;
mod schema;
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
mod schema_v9_sql;
mod write;

use std::path::{Path, PathBuf};

use forge_runtime_domain::GroupAgentNodeLifecycleInspection;
use forge_runtime_domain::{
    BeginGroupExecution, BeginGroupExecutionResult, BeginRun, BeginRunResult, Conversation,
    ConversationScope, GroupContextPolicy, GroupContextSlice, GroupExecutionEvent,
    GroupExecutionInspection, GroupExecutionRecord, GroupExecutionStore, GroupProjectMember,
    GroupRunRecord, GroupRunSnapshot, GroupRunStore, HubEntity, HubSnapshot, HubStore,
    HubStoreError, PrepareGroupRun, PrepareGroupRunResult, Project, PromptRecord, RunInspection,
    RunRecord, RunStore, RunStoreError, RuntimeEvent, SessionGroup,
};
use rusqlite::{Connection, Error as SqliteError, ErrorCode};

#[derive(Clone, Debug)]
pub struct SqliteHubStore {
    database_path: PathBuf,
    open_mode: SqliteHubStoreOpenMode,
}

#[derive(Clone, Copy, Debug)]
enum SqliteHubStoreOpenMode {
    ReadWrite,
    ExistingCurrentReadOnly,
    ExistingDispatchPreflightReadOnly,
    ExistingDispatchReentryReadOnly,
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

impl SqliteHubStore {
    /// Opens or creates a versioned local Hub database.
    ///
    /// # Errors
    ///
    /// Returns an error when the path is unsafe, `SQLite` cannot open the file,
    /// or the schema version is unsupported.
    pub fn open(database_path: impl AsRef<Path>) -> Result<Self, HubStoreError> {
        let database_path = database_path.as_ref().to_path_buf();
        schema::open_database(&database_path)?;
        Ok(Self {
            database_path,
            open_mode: SqliteHubStoreOpenMode::ReadWrite,
        })
    }

    /// Opens an existing current-schema Hub database without changing any state files.
    ///
    /// This mode never creates a database, migrates a schema, changes file permissions,
    /// or opens a database with live `SQLite` WAL/SHM/rollback-journal sidecars.
    ///
    /// # Errors
    ///
    /// Returns an error when the database is missing, unsafe, non-current, corrupt,
    /// concurrently changing, lacks a persistent WAL header, or has `SQLite` sidecars.
    pub fn open_existing_current_read_only(
        database_path: impl AsRef<Path>,
    ) -> Result<Self, HubStoreError> {
        let database_path = database_path.as_ref().to_path_buf();
        schema::open_existing_current_read_only_database(&database_path)?;
        Ok(Self {
            database_path,
            open_mode: SqliteHubStoreOpenMode::ExistingCurrentReadOnly,
        })
    }

    /// Opens an exact existing v11 through v26 Hub for dispatch topology preflight only.
    ///
    /// This mode is immutable and cannot create, migrate, chmod, or write Hub state.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe, missing, corrupt, unsupported, or active state.
    pub fn open_existing_dispatch_preflight_read_only(
        database_path: impl AsRef<Path>,
    ) -> Result<Self, HubStoreError> {
        let database_path = database_path.as_ref().to_path_buf();
        schema::open_existing_dispatch_preflight_read_only_database(&database_path)?;
        Ok(Self {
            database_path,
            open_mode: SqliteHubStoreOpenMode::ExistingDispatchPreflightReadOnly,
        })
    }

    /// Opens existing dispatch state for a no-send re-entry diagnosis.
    ///
    /// A clean exact v11 through v26 database keeps the immutable preflight path. When
    /// v12 through v26 has a hot WAL, the fallback reads the existing WAL/SHM pair without
    /// changing logical Hub content; `SQLite` may update transient SHM read locks.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe, missing, corrupt, unsupported, or changing state.
    pub fn open_existing_dispatch_inspection_read_only(
        database_path: impl AsRef<Path>,
    ) -> Result<Self, HubStoreError> {
        let database_path = database_path.as_ref().to_path_buf();
        let open_mode =
            match schema::open_existing_dispatch_preflight_read_only_database(&database_path) {
                Ok(_) => SqliteHubStoreOpenMode::ExistingDispatchPreflightReadOnly,
                Err(immutable_error) => {
                    schema::open_existing_dispatch_reentry_read_only_database(&database_path)
                        .map_err(|reentry_error| {
                            prefer_corruption(immutable_error, reentry_error)
                        })?;
                    SqliteHubStoreOpenMode::ExistingDispatchReentryReadOnly
                }
            };
        Ok(Self {
            database_path,
            open_mode,
        })
    }

    /// Inspects a dispatch lifecycle, when one exists, in one deferred snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error for missing Runs, corrupt evidence, or unavailable storage.
    pub fn inspect_existing_group_agent_node_lifecycle(
        &self,
        graph_run_id: &str,
    ) -> Result<Option<GroupAgentNodeLifecycleInspection>, HubStoreError> {
        group_agent_node_lifecycle::inspect_if_present(&mut self.connect()?, graph_run_id)
    }

    fn connect(&self) -> Result<Connection, HubStoreError> {
        match self.open_mode {
            SqliteHubStoreOpenMode::ReadWrite => schema::open_database(&self.database_path),
            SqliteHubStoreOpenMode::ExistingCurrentReadOnly => {
                schema::open_existing_current_read_only_database(&self.database_path)
            }
            SqliteHubStoreOpenMode::ExistingDispatchPreflightReadOnly => {
                schema::open_existing_dispatch_preflight_read_only_database(&self.database_path)
            }
            SqliteHubStoreOpenMode::ExistingDispatchReentryReadOnly => {
                schema::open_existing_dispatch_reentry_read_only_database(&self.database_path)
            }
        }
    }
}

fn prefer_corruption(
    immutable_error: HubStoreError,
    reentry_error: HubStoreError,
) -> HubStoreError {
    if matches!(immutable_error, HubStoreError::Corrupt { .. }) {
        return immutable_error;
    }
    if matches!(reentry_error, HubStoreError::Corrupt { .. }) {
        return reentry_error;
    }
    immutable_error
}

impl HubStore for SqliteHubStore {
    fn open_project(&self, absolute_path: &Path) -> Result<Project, HubStoreError> {
        let mut connection = self.connect()?;
        write::open_project(&mut connection, absolute_path)
    }

    fn snapshot(&self, scope: &ConversationScope) -> Result<HubSnapshot, HubStoreError> {
        let mut connection = self.connect()?;
        read::snapshot(&mut connection, scope)
    }

    fn create_conversation(
        &self,
        scope: &ConversationScope,
        title: &str,
        idempotency_key: &str,
    ) -> Result<Conversation, HubStoreError> {
        let mut connection = self.connect()?;
        write::create_conversation(&mut connection, scope, title, idempotency_key)
    }

    fn list_conversations(
        &self,
        scope: &ConversationScope,
    ) -> Result<Vec<Conversation>, HubStoreError> {
        read::list_conversations(&self.connect()?, scope)
    }

    fn append_prompt(
        &self,
        conversation_id: &str,
        role: &str,
        content: &str,
        idempotency_key: &str,
    ) -> Result<PromptRecord, HubStoreError> {
        let mut connection = self.connect()?;
        write::append_prompt(
            &mut connection,
            conversation_id,
            role,
            content,
            idempotency_key,
        )
    }

    fn list_prompts(
        &self,
        conversation_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<PromptRecord>, HubStoreError> {
        read::list_prompts(&self.connect()?, conversation_id, limit)
    }

    fn list_prompts_before(
        &self,
        conversation_id: &str,
        boundary_prompt_id: &str,
        limit: usize,
    ) -> Result<Vec<PromptRecord>, HubStoreError> {
        let mut connection = self.connect()?;
        read::list_prompts_before(&mut connection, conversation_id, boundary_prompt_id, limit)
    }

    fn load_group_context(
        &self,
        group_id: &str,
        policy: &GroupContextPolicy,
    ) -> Result<GroupContextSlice, HubStoreError> {
        let mut connection = self.connect()?;
        group_context_read::load(&mut connection, group_id, policy)
    }

    fn create_group(
        &self,
        name: &str,
        idempotency_key: &str,
    ) -> Result<SessionGroup, HubStoreError> {
        let mut connection = self.connect()?;
        write::create_group(&mut connection, name, idempotency_key)
    }

    fn list_groups(&self) -> Result<Vec<SessionGroup>, HubStoreError> {
        read::list_groups(&self.connect()?)
    }

    fn add_project_to_group(
        &self,
        group_id: &str,
        project_id: &str,
        role: &str,
        idempotency_key: &str,
    ) -> Result<GroupProjectMember, HubStoreError> {
        let mut connection = self.connect()?;
        write::add_project_to_group(&mut connection, group_id, project_id, role, idempotency_key)
    }

    fn add_project_path_to_group(
        &self,
        group_id: &str,
        absolute_path: &Path,
        role: &str,
        idempotency_key: &str,
    ) -> Result<GroupProjectMember, HubStoreError> {
        let mut connection = self.connect()?;
        atomic_link::add_project_path_to_group(
            &mut connection,
            group_id,
            absolute_path,
            role,
            idempotency_key,
        )
    }
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
    fn begin_run(&self, request: &BeginRun) -> Result<BeginRunResult, RunStoreError> {
        let mut connection = self.connect_run()?;
        run_write::begin_run(&mut connection, request)
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
