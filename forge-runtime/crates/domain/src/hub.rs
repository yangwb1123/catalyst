use std::path::PathBuf;

use serde::{Deserialize, Serialize};

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
use std::{path::Path, sync::Arc};

use crate::tool::{ToolError, ToolOutput};

pub trait WorkspaceReader: Send + Sync {
    /// Reads one file relative to the anchored workspace.
    ///
    /// # Errors
    ///
    /// Returns a tool error when the path is denied, the file cannot be read,
    /// or the output exceeds `max_bytes`.
    fn read_file(&self, relative: &Path, max_bytes: usize) -> Result<ToolOutput, ToolError>;
}

#[derive(Clone)]
pub struct WorkspaceReadCapability {
    reader: Arc<dyn WorkspaceReader>,
}

impl WorkspaceReadCapability {
    #[must_use]
    pub fn new(reader: Arc<dyn WorkspaceReader>) -> Self {
        Self { reader }
    }

    /// Reads one workspace-relative file through the anchored capability.
    ///
    /// # Errors
    ///
    /// Returns a tool error when the path is denied, the file cannot be read,
    /// or the output exceeds the requested bound.
    pub fn read_file(&self, relative: &Path, max_bytes: usize) -> Result<ToolOutput, ToolError> {
        self.reader.read_file(relative, max_bytes)
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
