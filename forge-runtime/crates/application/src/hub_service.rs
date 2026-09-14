mod owned;

use sha2::{Digest, Sha256};
use std::{path::Path, sync::Arc};

use crate::hub_validation::{
    conversation_bootstrap_page, conversation_change_page_limit, conversation_prompt_page,
};
use crate::{
    HubError, HubField,
    hub_validation::{
        MAX_GROUP_NAME_BYTES, MAX_PROMPT_BYTES, MAX_ROLE_BYTES, MAX_TITLE_BYTES,
        normalized_absolute_path, prompt_limit, required, required_id, scope,
        validate_idempotency_key,
    },
    runtime_domain::{
        Conversation, ConversationBootstrapCursor, ConversationBootstrapPage,
        ConversationChangePage, ConversationOwner, ConversationPromptCursor,
        ConversationPromptPage, ConversationScope, GroupContextPolicy, GroupContextSlice,
        GroupProjectMember, HubSnapshot, HubSnapshotAtCursor, HubStore,
        LocalConversationImportSource, MAX_CONVERSATION_OWNER_ISSUER_BYTES,
        MAX_CONVERSATION_OWNER_SUBJECT_BYTES, MAX_CONVERSATION_OWNER_TENANT_BYTES,
        MAX_GROUP_CONTEXT_CONTENT_BYTES, OwnedConversationChangePage, Project, PromptRecord,
        SessionGroup,
    },
};

const LOWERCASE_HEX: &[u8; 16] = b"0123456789abcdef";

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

    /// Reads this exact principal's changes after its last owner-local cursor.
    /// The returned cursor reveals no Hub-global journal position.
    ///
    /// # Errors
    ///
    /// Returns validation or storage errors when owner-scoped paging fails.
    pub fn owned_conversation_changes_after(
        &self,
        owner: &ConversationOwner,
        after_cursor: u64,
        limit: usize,
    ) -> Result<OwnedConversationChangePage, HubError> {
        validate_owner(owner)?;
        conversation_change_page_limit(limit)?;
        if i64::try_from(after_cursor).is_err() {
            return Err(HubError::OutOfRange {
                field: HubField::OwnedConversationChangeCursor,
                min: 0,
                max: usize::try_from(i64::MAX).unwrap_or(usize::MAX),
            });
        }
        Ok(self
            .store
            .owned_conversation_changes_after(owner, after_cursor, limit)?)
    }

    /// Loads the Global Hub snapshot and store-global change cursor atomically.
    ///
    /// This is the bootstrap point for a later replicated client: it can load
    /// the snapshot at cursor N and then request changes strictly after N.
    ///
    /// # Errors
    ///
    /// Returns a structured storage error when the snapshot cannot be loaded.
    pub fn snapshot_at_cursor(&self) -> Result<HubSnapshotAtCursor, HubError> {
        Ok(self.store.snapshot_at_cursor()?)
    }

    /// Reads one bounded page from the Hub-local Conversation change feed.
    ///
    /// # Errors
    ///
    /// Returns an out-of-range error for unsupported page sizes or a structured
    /// storage error for invalid cursors and corrupt change rows.
    pub fn conversation_changes_after(
        &self,
        after_cursor: u64,
        limit: usize,
    ) -> Result<ConversationChangePage, HubError> {
        conversation_change_page_limit(limit)?;
        Ok(self.store.conversation_changes_after(after_cursor, limit)?)
    }

    /// Reads one bounded Conversation metadata page through a fixed journal head.
    ///
    /// # Errors
    ///
    /// Returns validation errors for malformed cursors or limits and structured
    /// storage errors for cursors beyond the current head or corrupt journal rows.
    pub fn conversation_bootstrap_page(
        &self,
        cursor: Option<&ConversationBootstrapCursor>,
        limit: usize,
    ) -> Result<ConversationBootstrapPage, HubError> {
        conversation_bootstrap_page(cursor, limit)?;
        Ok(self.store.conversation_bootstrap_page(cursor, limit)?)
    }

    /// Reads one bounded newest-first page of Prompt history for one Conversation.
    ///
    /// The cursor is exclusive and ordered by `(created_at_ms DESC, prompt_id DESC)`.
    ///
    /// # Errors
    ///
    /// Returns validation errors for invalid IDs, cursors, or limits and a
    /// structured storage error for a missing Conversation or corrupt Prompt row.
    pub fn conversation_prompt_page(
        &self,
        conversation_id: &str,
        before: Option<&ConversationPromptCursor>,
        limit: usize,
    ) -> Result<ConversationPromptPage, HubError> {
        required_id(conversation_id, HubField::ConversationId)?;
        conversation_prompt_page(before, limit)?;
        Ok(self
            .store
            .conversation_prompt_page(conversation_id, before, limit)?)
    }

    /// Previews a bounded ownerless local Conversation for explicit import.
    ///
    /// # Errors
    ///
    /// Returns a validation error for an invalid Conversation ID and a
    /// structured storage error if the source is missing or not importable.
    pub fn local_conversation_import_source(
        &self,
        conversation_id: &str,
    ) -> Result<LocalConversationImportSource, HubError> {
        required_id(conversation_id, HubField::ConversationId)?;
        Ok(self
            .store
            .local_conversation_import_source(conversation_id)?)
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

fn validate_owner(owner: &ConversationOwner) -> Result<(), HubError> {
    validate_owner_component(
        &owner.issuer,
        HubField::ConversationOwnerIssuer,
        MAX_CONVERSATION_OWNER_ISSUER_BYTES,
    )?;
    validate_owner_component(
        &owner.subject,
        HubField::ConversationOwnerSubject,
        MAX_CONVERSATION_OWNER_SUBJECT_BYTES,
    )?;
    validate_owner_component(
        &owner.tenant_id,
        HubField::ConversationOwnerTenant,
        MAX_CONVERSATION_OWNER_TENANT_BYTES,
    )
}

fn validate_owner_component(
    value: &str,
    field: HubField,
    max_bytes: usize,
) -> Result<(), HubError> {
    required(value, field, max_bytes)?;
    if value.chars().any(char::is_control) {
        return Err(HubError::InvalidCharacters { field });
    }
    Ok(())
}

fn owner_idempotency_key(owner: &ConversationOwner, operation: &str, supplied: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"forgeos.conversation-owner-idempotency.v1\0");
    for value in [
        owner.issuer.as_str(),
        owner.subject.as_str(),
        owner.tenant_id.as_str(),
        operation,
        supplied,
    ] {
        digest.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
        digest.update(value.as_bytes());
    }
    let mut encoded = String::with_capacity(64);
    for byte in digest.finalize() {
        encoded.push(char::from(LOWERCASE_HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(LOWERCASE_HEX[usize::from(byte & 0x0f)]));
    }
    format!("owner-v1-{encoded}")
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
