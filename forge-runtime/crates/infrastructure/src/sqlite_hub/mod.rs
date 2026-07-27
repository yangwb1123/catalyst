mod atomic_link;
mod read;
mod rows;
mod schema;
mod write;

use std::path::{Path, PathBuf};

use forge_runtime_domain::{
    Conversation, ConversationScope, GroupProjectMember, HubEntity, HubSnapshot, HubStore,
    HubStoreError, Project, PromptRecord, SessionGroup,
};
use rusqlite::{Connection, Error as SqliteError, ErrorCode};

#[derive(Clone, Debug)]
pub struct SqliteHubStore {
    database_path: PathBuf,
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
        Ok(Self { database_path })
    }

    fn connect(&self) -> Result<Connection, HubStoreError> {
        schema::open_database(&self.database_path)
    }
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
        _ => unavailable(error),
    }
}

fn read_error(error: SqliteError) -> HubStoreError {
    match error {
        SqliteError::FromSqlConversionFailure(..)
        | SqliteError::InvalidColumnType(..)
        | SqliteError::IntegralValueOutOfRange(..) => HubStoreError::Corrupt {
            message: error.to_string(),
        },
        _ => unavailable(error),
    }
}

fn unavailable(error: impl std::fmt::Display) -> HubStoreError {
    HubStoreError::Unavailable {
        message: error.to_string(),
    }
}
