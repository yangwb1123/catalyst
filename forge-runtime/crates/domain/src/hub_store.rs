use std::path::Path;

use crate::{
    Conversation, ConversationScope, GroupProjectMember, HubSnapshot, Project, PromptRecord,
    SessionGroup,
};

pub trait HubStore: Send + Sync {
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
    /// `None` spans every conversation visible to the store.
    ///
    /// # Errors
    ///
    /// Returns a structured storage error when prompt history cannot be loaded.
    fn list_prompts(
        &self,
        conversation_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<PromptRecord>, HubStoreError>;

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
    Conversation,
    Prompt,
    Group,
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
