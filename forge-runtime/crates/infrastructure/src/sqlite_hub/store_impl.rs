use std::path::Path;

use super::{
    Conversation, ConversationBootstrapCursor, ConversationBootstrapPage, ConversationChangePage,
    ConversationImportPrompt, ConversationOwner, ConversationPromptCursor, ConversationPromptPage,
    ConversationScope, GroupContextPolicy, GroupContextSlice, GroupProjectMember, HubSnapshot,
    HubSnapshotAtCursor, HubStore, HubStoreError, LocalConversationImportSource,
    OwnedConversationChangePage, OwnedConversationImportResult, OwnedConversationPage,
    OwnedProjectConversationIdentity, OwnedPromptAppendResult, OwnedRunCursor, OwnedRunPage,
    OwnedRunTimelinePage, PendingRunIntentCursor, PendingRunIntentPage,
    PendingRunIntentSubmissionResult, PendingRunIntentTimelinePage, Project,
    ProjectExecutionConsentGrantResult, ProjectExecutionConsentRevocationResult, PromptRecord,
    SessionGroup, SqliteHubStore, SubmitPendingRunIntent, atomic_link, change_read,
    conversation_bootstrap_read, conversation_import_read, conversation_owner_changes_read,
    conversation_owner_read, conversation_project_read, group_context_read, owned_run_read,
    pending_run_intent, project_execution_consent, prompt_read, read, write,
};

impl HubStore for SqliteHubStore {
    fn create_owned_conversation(
        &self,
        owner: &ConversationOwner,
        scope: &ConversationScope,
        title: &str,
        idempotency_key: &str,
    ) -> Result<Conversation, HubStoreError> {
        write::create_owned_conversation(&mut self.connect()?, owner, scope, title, idempotency_key)
    }

    fn import_owned_conversation(
        &self,
        owner: &ConversationOwner,
        title: &str,
        prompts: &[ConversationImportPrompt],
        idempotency_key: &str,
    ) -> Result<OwnedConversationImportResult, HubStoreError> {
        write::import_owned_conversation(
            &mut self.connect()?,
            owner,
            title,
            prompts,
            idempotency_key,
        )
    }

    fn local_conversation_import_source(
        &self,
        conversation_id: &str,
    ) -> Result<LocalConversationImportSource, HubStoreError> {
        conversation_import_read::local_conversation_import_source(
            &mut self.connect()?,
            conversation_id,
        )
    }

    fn list_owned_conversations(
        &self,
        owner: &ConversationOwner,
        after_id: Option<&str>,
        limit: usize,
    ) -> Result<OwnedConversationPage, HubStoreError> {
        conversation_owner_read::list_owned_conversations(
            &mut self.connect()?,
            owner,
            after_id,
            limit,
        )
    }

    fn owned_project_conversation_identity(
        &self,
        owner: &ConversationOwner,
        conversation_id: &str,
    ) -> Result<OwnedProjectConversationIdentity, HubStoreError> {
        conversation_project_read::owned_project_conversation_identity(
            &mut self.connect()?,
            owner,
            conversation_id,
        )
    }

    fn owned_conversation_changes_after(
        &self,
        owner: &ConversationOwner,
        after_cursor: u64,
        limit: usize,
    ) -> Result<OwnedConversationChangePage, HubStoreError> {
        conversation_owner_changes_read::owned_conversation_changes_after(
            &mut self.connect()?,
            owner,
            after_cursor,
            limit,
        )
    }

    fn owned_run_page(
        &self,
        owner: &ConversationOwner,
        conversation_id: &str,
        before: Option<&OwnedRunCursor>,
        limit: usize,
    ) -> Result<OwnedRunPage, HubStoreError> {
        owned_run_read::owned_run_page(&mut self.connect()?, owner, conversation_id, before, limit)
    }

    fn owned_run_timeline_page(
        &self,
        owner: &ConversationOwner,
        conversation_id: &str,
        run_id: &str,
        after_sequence: u64,
        limit: usize,
    ) -> Result<OwnedRunTimelinePage, HubStoreError> {
        owned_run_read::owned_run_timeline_page(
            &mut self.connect()?,
            owner,
            conversation_id,
            run_id,
            after_sequence,
            limit,
        )
    }

    fn owned_conversation_prompt_page(
        &self,
        owner: &ConversationOwner,
        conversation_id: &str,
        before: Option<&ConversationPromptCursor>,
        limit: usize,
    ) -> Result<ConversationPromptPage, HubStoreError> {
        prompt_read::owned_conversation_prompt_page(
            &mut self.connect()?,
            owner,
            conversation_id,
            before,
            limit,
        )
    }

    fn append_owned_prompt(
        &self,
        owner: &ConversationOwner,
        conversation_id: &str,
        content: &str,
        idempotency_key: &str,
        expected_version: u64,
    ) -> Result<OwnedPromptAppendResult, HubStoreError> {
        write::append_owned_prompt(
            &mut self.connect()?,
            owner,
            conversation_id,
            content,
            idempotency_key,
            expected_version,
        )
    }

    fn grant_project_execution_consent(
        &self,
        owner: &ConversationOwner,
        project_id: &str,
        profile_id: &str,
        profile_sha256: &[u8; 32],
        expires_at_ms: u64,
        idempotency_key: &str,
    ) -> Result<ProjectExecutionConsentGrantResult, HubStoreError> {
        project_execution_consent::grant(
            &mut self.connect()?,
            project_execution_consent::GrantInput {
                owner,
                project_id,
                profile_id,
                profile_sha256,
                expires_at_ms,
                idempotency_key,
            },
        )
    }

    fn revoke_project_execution_consent(
        &self,
        owner: &ConversationOwner,
        grant_id: &str,
        idempotency_key: &str,
    ) -> Result<ProjectExecutionConsentRevocationResult, HubStoreError> {
        project_execution_consent::revoke(&mut self.connect()?, owner, grant_id, idempotency_key)
    }

    fn submit_owned_prompt_run_intent(
        &self,
        owner: &ConversationOwner,
        submission: &SubmitPendingRunIntent<'_>,
    ) -> Result<PendingRunIntentSubmissionResult, HubStoreError> {
        pending_run_intent::submit(
            &mut self.connect()?,
            pending_run_intent::SubmitInput {
                owner,
                conversation_id: submission.conversation_id,
                content: submission.content,
                idempotency_key: submission.idempotency_key,
                expected_version: submission.expected_version,
                profile_id: submission.profile_id,
                profile_sha256: submission.profile_sha256,
            },
        )
    }

    fn owned_pending_run_intent_page(
        &self,
        owner: &ConversationOwner,
        conversation_id: &str,
        before: Option<&PendingRunIntentCursor>,
        limit: usize,
    ) -> Result<PendingRunIntentPage, HubStoreError> {
        pending_run_intent::owned_page(&mut self.connect()?, owner, conversation_id, before, limit)
    }

    fn owned_pending_run_intent_timeline_page(
        &self,
        owner: &ConversationOwner,
        conversation_id: &str,
        intent_id: &str,
        after_sequence: u64,
        limit: usize,
    ) -> Result<PendingRunIntentTimelinePage, HubStoreError> {
        pending_run_intent::owned_timeline_page(
            &mut self.connect()?,
            owner,
            conversation_id,
            intent_id,
            after_sequence,
            limit,
        )
    }

    fn open_project(&self, absolute_path: &Path) -> Result<Project, HubStoreError> {
        let mut connection = self.connect()?;
        write::open_project(&mut connection, absolute_path)
    }

    fn snapshot(&self, scope: &ConversationScope) -> Result<HubSnapshot, HubStoreError> {
        let mut connection = self.connect()?;
        read::snapshot(&mut connection, scope)
    }

    fn snapshot_at_cursor(&self) -> Result<HubSnapshotAtCursor, HubStoreError> {
        let mut connection = self.connect()?;
        change_read::snapshot_at_cursor(&mut connection)
    }

    fn conversation_changes_after(
        &self,
        after_cursor: u64,
        limit: usize,
    ) -> Result<ConversationChangePage, HubStoreError> {
        let mut connection = self.connect()?;
        change_read::conversation_changes_after(&mut connection, after_cursor, limit)
    }

    fn conversation_bootstrap_page(
        &self,
        cursor: Option<&ConversationBootstrapCursor>,
        limit: usize,
    ) -> Result<ConversationBootstrapPage, HubStoreError> {
        conversation_bootstrap_read::conversation_bootstrap_page(
            &mut self.connect()?,
            cursor,
            limit,
        )
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
        prompt_read::list_prompts(&self.connect()?, conversation_id, limit)
    }

    fn conversation_prompt_page(
        &self,
        conversation_id: &str,
        before: Option<&ConversationPromptCursor>,
        limit: usize,
    ) -> Result<ConversationPromptPage, HubStoreError> {
        prompt_read::conversation_prompt_page(&mut self.connect()?, conversation_id, before, limit)
    }

    fn list_prompts_before(
        &self,
        conversation_id: &str,
        boundary_prompt_id: &str,
        limit: usize,
    ) -> Result<Vec<PromptRecord>, HubStoreError> {
        let mut connection = self.connect()?;
        prompt_read::list_prompts_before(
            &mut connection,
            conversation_id,
            boundary_prompt_id,
            limit,
        )
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
