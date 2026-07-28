use std::{path::Path, sync::Arc};

use crate::{
    HubError, HubField,
    hub_validation::{
        MAX_GROUP_NAME_BYTES, MAX_IDEMPOTENCY_KEY_BYTES, MAX_PROMPT_BYTES, MAX_ROLE_BYTES,
        MAX_TITLE_BYTES, normalized_absolute_path, prompt_limit, required, required_id, scope,
    },
    runtime_domain::{
        Conversation, ConversationScope, GroupContextPolicy, GroupContextSlice, GroupProjectMember,
        HubSnapshot, HubStore, MAX_GROUP_CONTEXT_CONTENT_BYTES, Project, PromptRecord,
        SessionGroup,
    },
};

pub struct HubService {
    store: Arc<dyn HubStore>,
}

impl HubService {
    #[must_use]
    pub fn new(store: Arc<dyn HubStore>) -> Self {
        Self { store }
    }

    /// Opens a previously normalized absolute project path.
    ///
    /// # Errors
    ///
    /// Returns validation or structured storage errors.
    pub fn open_project(&self, absolute_path: &Path) -> Result<Project, HubError> {
        normalized_absolute_path(absolute_path)?;
        Ok(self.store.open_project(absolute_path)?)
    }

    /// Loads the global hub overview.
    ///
    /// # Errors
    ///
    /// Returns a structured storage error when the snapshot cannot be loaded.
    pub fn global_snapshot(&self) -> Result<HubSnapshot, HubError> {
        Ok(self.store.snapshot(&ConversationScope::Global)?)
    }

    /// Loads one project's scoped hub overview.
    ///
    /// # Errors
    ///
    /// Returns validation or structured storage errors.
    pub fn project_snapshot(&self, project_id: &str) -> Result<HubSnapshot, HubError> {
        required_id(project_id, HubField::ProjectId)?;
        let scope = ConversationScope::Project(project_id.to_owned());
        Ok(self.store.snapshot(&scope)?)
    }

    /// Loads one collaboration group's scoped overview.
    ///
    /// # Errors
    ///
    /// Returns validation or structured storage errors.
    pub fn group_snapshot(&self, group_id: &str) -> Result<HubSnapshot, HubError> {
        required_id(group_id, HubField::GroupId)?;
        let scope = ConversationScope::Group(group_id.to_owned());
        Ok(self.store.snapshot(&scope)?)
    }

    /// Creates the conversation represented as a session by the CLI.
    ///
    /// # Errors
    ///
    /// Returns validation or structured storage errors.
    pub fn create_session(
        &self,
        scope_value: &ConversationScope,
        title: &str,
        idempotency_key: &str,
    ) -> Result<Conversation, HubError> {
        scope(scope_value)?;
        required(title, HubField::Title, MAX_TITLE_BYTES)?;
        validate_idempotency_key(idempotency_key)?;
        Ok(self
            .store
            .create_conversation(scope_value, title, idempotency_key)?)
    }

    /// Lists sessions visible in a scope; the global scope spans every session.
    ///
    /// # Errors
    ///
    /// Returns validation or structured storage errors.
    pub fn list_sessions(
        &self,
        scope_value: &ConversationScope,
    ) -> Result<Vec<Conversation>, HubError> {
        scope(scope_value)?;
        Ok(self.store.list_conversations(scope_value)?)
    }

    /// Appends one durable prompt record to a conversation.
    ///
    /// # Errors
    ///
    /// Returns validation or structured storage errors.
    pub fn append_prompt(
        &self,
        conversation_id: &str,
        role: &str,
        content: &str,
        idempotency_key: &str,
    ) -> Result<PromptRecord, HubError> {
        required_id(conversation_id, HubField::ConversationId)?;
        required(role, HubField::Role, MAX_ROLE_BYTES)?;
        required(content, HubField::Prompt, MAX_PROMPT_BYTES)?;
        validate_idempotency_key(idempotency_key)?;
        Ok(self
            .store
            .append_prompt(conversation_id, role, content, idempotency_key)?)
    }

    /// Lists newest prompt records globally or for one conversation.
    ///
    /// # Errors
    ///
    /// Returns validation or structured storage errors.
    pub fn list_prompts(
        &self,
        conversation_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<PromptRecord>, HubError> {
        if let Some(id) = conversation_id {
            required_id(id, HubField::ConversationId)?;
        }
        prompt_limit(limit)?;
        Ok(self.store.list_prompts(conversation_id, limit)?)
    }

    /// Builds an atomic, bounded Prompt dossier for one collaboration group.
    ///
    /// Project paths, files, Run journals, and tool/provider context are never
    /// included. Group roles remain descriptive provenance only.
    ///
    /// # Errors
    ///
    /// Returns validation or structured storage errors.
    pub fn group_context(
        &self,
        group_id: &str,
        max_content_bytes: usize,
    ) -> Result<GroupContextSlice, HubError> {
        required_id(group_id, HubField::GroupId)?;
        context_bytes(max_content_bytes)?;
        let policy = GroupContextPolicy {
            max_total_content_bytes: max_content_bytes,
            ..GroupContextPolicy::default()
        };
        Ok(self.store.load_group_context(group_id, &policy)?)
    }

    /// Creates a collaboration group.
    ///
    /// # Errors
    ///
    /// Returns validation or structured storage errors.
    pub fn create_group(
        &self,
        name: &str,
        idempotency_key: &str,
    ) -> Result<SessionGroup, HubError> {
        required(name, HubField::GroupName, MAX_GROUP_NAME_BYTES)?;
        validate_idempotency_key(idempotency_key)?;
        Ok(self.store.create_group(name, idempotency_key)?)
    }

    /// Lists collaboration groups.
    ///
    /// # Errors
    ///
    /// Returns a structured storage error when the list cannot be loaded.
    pub fn list_groups(&self) -> Result<Vec<SessionGroup>, HubError> {
        Ok(self.store.list_groups()?)
    }

    /// Associates a project and descriptive role with a collaboration group.
    ///
    /// # Errors
    ///
    /// Returns validation or structured storage errors.
    pub fn add_project_to_group(
        &self,
        group_id: &str,
        project_id: &str,
        role: &str,
        idempotency_key: &str,
    ) -> Result<GroupProjectMember, HubError> {
        required_id(group_id, HubField::GroupId)?;
        required_id(project_id, HubField::ProjectId)?;
        required(role, HubField::Role, MAX_ROLE_BYTES)?;
        validate_idempotency_key(idempotency_key)?;
        Ok(self
            .store
            .add_project_to_group(group_id, project_id, role, idempotency_key)?)
    }

    /// Atomically registers a normalized path and links it to a group.
    ///
    /// # Errors
    ///
    /// Returns validation or structured storage errors without partial registration.
    pub fn add_project_path_to_group(
        &self,
        group_id: &str,
        absolute_path: &Path,
        role: &str,
        idempotency_key: &str,
    ) -> Result<GroupProjectMember, HubError> {
        required_id(group_id, HubField::GroupId)?;
        normalized_absolute_path(absolute_path)?;
        required(role, HubField::Role, MAX_ROLE_BYTES)?;
        validate_idempotency_key(idempotency_key)?;
        Ok(self
            .store
            .add_project_path_to_group(group_id, absolute_path, role, idempotency_key)?)
    }
}

fn validate_idempotency_key(value: &str) -> Result<(), HubError> {
    required(value, HubField::IdempotencyKey, MAX_IDEMPOTENCY_KEY_BYTES)?;
    Ok(())
}

fn context_bytes(value: usize) -> Result<(), HubError> {
    if (1..=MAX_GROUP_CONTEXT_CONTENT_BYTES).contains(&value) {
        return Ok(());
    }
    Err(HubError::OutOfRange {
        field: HubField::GroupContextBytes,
        min: 1,
        max: MAX_GROUP_CONTEXT_CONTENT_BYTES,
    })
}
