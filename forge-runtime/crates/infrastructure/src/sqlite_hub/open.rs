use std::path::Path;

use rusqlite::Connection;

use crate::runtime_domain::{GroupAgentNodeLifecycleInspection, HubStoreError};

use super::{SqliteHubStore, group_agent_node_lifecycle, schema};

#[derive(Clone, Copy, Debug)]
pub(super) enum SqliteHubStoreOpenMode {
    ReadWrite,
    ExistingCurrentReadOnly,
    ExistingCurrentLiveReadOnly,
    ExistingCurrentWritable,
    ExistingDispatchPreflightReadOnly,
    ExistingDispatchReentryReadOnly,
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

    /// Opens an immutable, existing current-schema Hub without changing state files.
    ///
    /// This mode rejects live `SQLite` WAL/SHM/rollback-journal sidecars. It remains
    /// the effect-free contract for ordinary journal reads and dispatch preflight.
    ///
    /// # Errors
    ///
    /// Returns an error for missing, unsafe, non-current, corrupt, or active state.
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

    /// Opens an exact current-schema Hub through a live read-only `SQLite` snapshot.
    ///
    /// This non-immutable mode can include an existing complete WAL/SHM pair. It uses
    /// `mode=ro` plus `query_only`, never migrates or performs logical Hub writes, and
    /// may let `SQLite` create, remove, or coordinate transient empty WAL/SHM artifacts
    /// and SHM reader locks. Those physical coordination effects are not Hub mutations.
    ///
    /// # Errors
    ///
    /// Returns an error for missing, unsafe, non-v34, corrupt, incomplete-sidecar,
    /// rollback-journal, or concurrently changing state.
    pub fn open_existing_current_live_read_only(
        database_path: impl AsRef<Path>,
    ) -> Result<Self, HubStoreError> {
        let database_path = database_path.as_ref().to_path_buf();
        schema::open_existing_current_live_read_only_database(&database_path)?;
        Ok(Self {
            database_path,
            open_mode: SqliteHubStoreOpenMode::ExistingCurrentLiveReadOnly,
        })
    }

    /// Opens an exact existing v34 Hub for controlled application writes.
    ///
    /// This path refuses to create a database or migrate its schema. It enables
    /// foreign-key enforcement and verifies the existing database/WAL identity
    /// around open; `SQLite` may still coordinate its WAL/SHM files for this
    /// read-write connection.
    ///
    /// # Errors
    ///
    /// Returns an error for missing, unsafe, non-current, corrupt, incomplete-sidecar,
    /// rollback-journal, or concurrently changing state.
    pub fn open_existing_current_writable(
        database_path: impl AsRef<Path>,
    ) -> Result<Self, HubStoreError> {
        let database_path = database_path.as_ref().to_path_buf();
        schema::open_existing_current_writable_database(&database_path)?;
        Ok(Self {
            database_path,
            open_mode: SqliteHubStoreOpenMode::ExistingCurrentWritable,
        })
    }

    /// Opens an exact existing v11 through v32 Hub for dispatch topology preflight.
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
    /// The immutable dispatch preflight accepts exact v11 through v32. Re-entry
    /// accepts v12 through v34, using a live read-only snapshot for hot WAL state
    /// and current v34; `SQLite` may coordinate transient SHM reader locks.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe, missing, corrupt, unsupported, or changing state.
    pub fn open_existing_dispatch_inspection_read_only(
        database_path: impl AsRef<Path>,
    ) -> Result<Self, HubStoreError> {
        let database_path = database_path.as_ref().to_path_buf();
        let open_mode = dispatch_inspection_mode(&database_path)?;
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

    /// Returns the schema version observed through this store's existing read-only snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when the snapshot cannot be reopened or queried.
    pub fn inspected_schema_version(&self) -> Result<i64, HubStoreError> {
        schema::schema_version(&self.connect()?).map_err(schema::sqlite_error)
    }

    pub(super) fn connect(&self) -> Result<Connection, HubStoreError> {
        match self.open_mode {
            SqliteHubStoreOpenMode::ReadWrite => schema::open_database(&self.database_path),
            SqliteHubStoreOpenMode::ExistingCurrentReadOnly => {
                schema::open_existing_current_read_only_database(&self.database_path)
            }
            SqliteHubStoreOpenMode::ExistingCurrentLiveReadOnly => {
                schema::open_existing_current_live_read_only_database(&self.database_path)
            }
            SqliteHubStoreOpenMode::ExistingCurrentWritable => {
                schema::open_existing_current_writable_database(&self.database_path)
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

fn dispatch_inspection_mode(path: &Path) -> Result<SqliteHubStoreOpenMode, HubStoreError> {
    match schema::open_existing_dispatch_preflight_read_only_database(path) {
        Ok(_) => Ok(SqliteHubStoreOpenMode::ExistingDispatchPreflightReadOnly),
        Err(immutable_error) => match schema::open_existing_current_read_only_database(path) {
            Ok(_) => Ok(SqliteHubStoreOpenMode::ExistingCurrentReadOnly),
            Err(current_error) => {
                let first_error = prefer_corruption(immutable_error, current_error);
                schema::open_existing_dispatch_reentry_read_only_database(path)
                    .map(|_| SqliteHubStoreOpenMode::ExistingDispatchReentryReadOnly)
                    .map_err(|reentry_error| prefer_corruption(first_error, reentry_error))
            }
        },
    }
}

fn prefer_corruption(
    immutable_error: HubStoreError,
    reentry_error: HubStoreError,
) -> HubStoreError {
    if matches!(immutable_error, HubStoreError::Corrupt { .. }) {
        immutable_error
    } else if matches!(reentry_error, HubStoreError::Corrupt { .. }) {
        reentry_error
    } else {
        immutable_error
    }
}
