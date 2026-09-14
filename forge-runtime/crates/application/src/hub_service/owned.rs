use crate::{
    HubError, HubField,
    hub_validation::{
        MAX_PROMPT_BYTES, MAX_TITLE_BYTES, conversation_prompt_page, required, required_id, scope,
        validate_idempotency_key,
    },
    runtime_domain::{
        Conversation, ConversationImportPrompt, ConversationOwner, ConversationPromptCursor,
        ConversationPromptPage, ConversationScope, MAX_CONVERSATION_IMPORT_PROMPT_COUNT,
        MAX_OWNED_CONVERSATION_PAGE_LIMIT, MAX_OWNED_RUN_PAGE_LIMIT,
        MAX_OWNED_RUN_TIMELINE_PAGE_LIMIT, MAX_PENDING_RUN_INTENT_PAGE_LIMIT,
        MAX_PENDING_RUN_INTENT_TIMELINE_PAGE_LIMIT, OwnedConversationImportResult,
        OwnedConversationPage, OwnedProjectConversationIdentity, OwnedPromptAppendResult,
        OwnedRunCursor, OwnedRunPage, OwnedRunTimelinePage, PendingRunIntentCursor,
        PendingRunIntentPage, PendingRunIntentSubmissionResult, PendingRunIntentTimelinePage,
        ProjectExecutionConsentGrantResult, ProjectExecutionConsentRevocationResult,
        SubmitPendingRunIntent,
    },
};

use super::{HubService, owner_idempotency_key, validate_owner};

impl HubService {
    /// Creates one account-owned Conversation while retaining an exact copy of
    /// the verified issuer, subject, and tenant principal in the Hub. Its scope
    /// is organizational metadata and grants no Project or execution authority.
    ///
    /// # Errors
    ///
    /// Returns validation or storage errors when ownership, scope, content, or
    /// idempotency checks fail.
    pub fn create_owned_conversation(
        &self,
        owner: &ConversationOwner,
        conversation_scope: &ConversationScope,
        title: &str,
        idempotency_key: &str,
    ) -> Result<Conversation, HubError> {
        validate_owner(owner)?;
        scope(conversation_scope)?;
        required(title, HubField::Title, MAX_TITLE_BYTES)?;
        validate_idempotency_key(idempotency_key)?;
        let storage_key = owner_idempotency_key(owner, "conversation", idempotency_key);
        Ok(self
            .store
            .create_owned_conversation(owner, conversation_scope, title, &storage_key)?)
    }

    /// Imports a bounded user-visible transcript as one new Global Conversation.
    ///
    /// The Hub never sees the source database, its Conversation ID, project paths,
    /// Runs, tool context, or local ownership. The source remains unchanged.
    ///
    /// # Errors
    ///
    /// Returns validation errors for an invalid owner, title, key, role, or
    /// transcript budget, and a structured storage error if the import fails.
    pub fn import_owned_conversation(
        &self,
        owner: &ConversationOwner,
        title: &str,
        prompts: &[ConversationImportPrompt],
        idempotency_key: &str,
    ) -> Result<OwnedConversationImportResult, HubError> {
        validate_owner(owner)?;
        required(title, HubField::Title, MAX_TITLE_BYTES)?;
        validate_idempotency_key(idempotency_key)?;
        if prompts.len() > MAX_CONVERSATION_IMPORT_PROMPT_COUNT {
            return Err(HubError::OutOfRange {
                field: HubField::PromptLimit,
                min: 0,
                max: MAX_CONVERSATION_IMPORT_PROMPT_COUNT,
            });
        }
        let mut total_content_bytes = 0_usize;
        for prompt in prompts {
            if !matches!(prompt.role.as_str(), "user" | "assistant") {
                return Err(HubError::InvalidCharacters {
                    field: HubField::Role,
                });
            }
            required(&prompt.content, HubField::Prompt, MAX_PROMPT_BYTES)?;
            total_content_bytes = total_content_bytes.saturating_add(prompt.content.len());
            if total_content_bytes > MAX_PROMPT_BYTES {
                return Err(HubError::TooLong {
                    field: HubField::Prompt,
                    max_bytes: MAX_PROMPT_BYTES,
                });
            }
        }
        let storage_key = owner_idempotency_key(owner, "conversation_import", idempotency_key);
        Ok(self
            .store
            .import_owned_conversation(owner, title, prompts, &storage_key)?)
    }

    /// Lists only Conversations owned by the verified principal using a stable
    /// bounded cursor that does not reveal the Hub-global change sequence.
    ///
    /// # Errors
    ///
    /// Returns validation or storage errors when the owner page cannot be read.
    pub fn list_owned_conversations(
        &self,
        owner: &ConversationOwner,
        after_id: Option<&str>,
        limit: usize,
    ) -> Result<OwnedConversationPage, HubError> {
        validate_owner(owner)?;
        if let Some(after_id) = after_id {
            required_id(after_id, HubField::ConversationId)?;
        }
        if !(1..=MAX_OWNED_CONVERSATION_PAGE_LIMIT).contains(&limit) {
            return Err(HubError::OutOfRange {
                field: HubField::OwnedConversationPageLimit,
                min: 1,
                max: MAX_OWNED_CONVERSATION_PAGE_LIMIT,
            });
        }
        Ok(self
            .store
            .list_owned_conversations(owner, after_id, limit)?)
    }

    /// Resolves a minimal trusted Project input from an owner-visible
    /// Project-scoped Conversation.
    ///
    /// # Errors
    ///
    /// Returns validation errors for an invalid owner or Conversation ID,
    /// uniform not-found for missing, ownerless, or foreign Conversations,
    /// and conflict when an owner-visible Conversation is not Project-scoped.
    pub fn owned_project_conversation_identity(
        &self,
        owner: &ConversationOwner,
        conversation_id: &str,
    ) -> Result<OwnedProjectConversationIdentity, HubError> {
        validate_owner(owner)?;
        required_id(conversation_id, HubField::ConversationId)?;
        Ok(self
            .store
            .owned_project_conversation_identity(owner, conversation_id)?)
    }

    /// Reads a bounded newest-first Run summary page for one owned Conversation.
    ///
    /// # Errors
    ///
    /// Returns validation or storage errors without exposing Run execution data.
    pub fn owned_run_page(
        &self,
        owner: &ConversationOwner,
        conversation_id: &str,
        before: Option<&OwnedRunCursor>,
        limit: usize,
    ) -> Result<OwnedRunPage, HubError> {
        validate_owner(owner)?;
        required_id(conversation_id, HubField::ConversationId)?;
        if !(1..=MAX_OWNED_RUN_PAGE_LIMIT).contains(&limit) {
            return Err(HubError::OutOfRange {
                field: HubField::OwnedRunPageLimit,
                min: 1,
                max: MAX_OWNED_RUN_PAGE_LIMIT,
            });
        }
        if let Some(cursor) = before {
            required_id(&cursor.run_id, HubField::RunId)?;
            if cursor.created_at_ms > i64::MAX as u64 {
                return Err(HubError::OutOfRange {
                    field: HubField::OwnedRunCursor,
                    min: 0,
                    max: usize::try_from(i64::MAX).unwrap_or(usize::MAX),
                });
            }
        }
        Ok(self
            .store
            .owned_run_page(owner, conversation_id, before, limit)?)
    }

    /// Reads a bounded payload-free event-type timeline for one owned Run.
    ///
    /// # Errors
    ///
    /// Returns validation or storage errors when ownership or the page cannot
    /// be checked.
    pub fn owned_run_timeline_page(
        &self,
        owner: &ConversationOwner,
        conversation_id: &str,
        run_id: &str,
        after_sequence: u64,
        limit: usize,
    ) -> Result<OwnedRunTimelinePage, HubError> {
        validate_owner(owner)?;
        required_id(conversation_id, HubField::ConversationId)?;
        required_id(run_id, HubField::RunId)?;
        if !(1..=MAX_OWNED_RUN_TIMELINE_PAGE_LIMIT).contains(&limit) {
            return Err(HubError::OutOfRange {
                field: HubField::OwnedRunTimelinePageLimit,
                min: 1,
                max: MAX_OWNED_RUN_TIMELINE_PAGE_LIMIT,
            });
        }
        if after_sequence > i64::MAX as u64 {
            return Err(HubError::OutOfRange {
                field: HubField::OwnedRunTimelineSequence,
                min: 0,
                max: usize::try_from(i64::MAX).unwrap_or(usize::MAX),
            });
        }
        Ok(self.store.owned_run_timeline_page(
            owner,
            conversation_id,
            run_id,
            after_sequence,
            limit,
        )?)
    }

    /// Reads a bounded Prompt page only after the Hub confirms exact ownership.
    ///
    /// # Errors
    ///
    /// Returns validation or storage errors when ownership or paging fails.
    pub fn owned_conversation_prompt_page(
        &self,
        owner: &ConversationOwner,
        conversation_id: &str,
        before: Option<&ConversationPromptCursor>,
        limit: usize,
    ) -> Result<ConversationPromptPage, HubError> {
        validate_owner(owner)?;
        required_id(conversation_id, HubField::ConversationId)?;
        conversation_prompt_page(before, limit)?;
        Ok(self
            .store
            .owned_conversation_prompt_page(owner, conversation_id, before, limit)?)
    }

    /// Persists a user Prompt with owner validation and aggregate-version CAS.
    /// The idempotency key is namespaced to this exact identity before storage.
    ///
    /// # Errors
    ///
    /// Returns validation or storage errors when ownership, version, or
    /// idempotency checks fail.
    pub fn append_owned_prompt(
        &self,
        owner: &ConversationOwner,
        conversation_id: &str,
        content: &str,
        idempotency_key: &str,
        expected_version: u64,
    ) -> Result<OwnedPromptAppendResult, HubError> {
        validate_owner(owner)?;
        required_id(conversation_id, HubField::ConversationId)?;
        required(content, HubField::Prompt, MAX_PROMPT_BYTES)?;
        validate_idempotency_key(idempotency_key)?;
        if expected_version > i64::MAX as u64 {
            return Err(HubError::OutOfRange {
                field: HubField::ExpectedAggregateVersion,
                min: 0,
                max: usize::try_from(i64::MAX).unwrap_or(usize::MAX),
            });
        }
        let storage_key = owner_idempotency_key(owner, "prompt", idempotency_key);
        Ok(self.store.append_owned_prompt(
            owner,
            conversation_id,
            content,
            &storage_key,
            expected_version,
        )?)
    }

    /// Atomically persists a user Prompt with a separate inert pending Run
    /// intent. The caller supplies only the profile ID/digest selected by the
    /// trusted Runtime control plane; Project scope is derived by the Hub.
    ///
    /// Replays return the original receipt before checking current CAS or grant
    /// state. A replay never re-evaluates or extends consent.
    ///
    /// # Errors
    ///
    /// Returns validation errors for malformed identifiers, content, or
    /// version; not-found for a Conversation the owner cannot access; conflict
    /// for scope, compare-and-swap, profile, or consent failures; and storage
    /// errors when the Hub transaction cannot complete.
    pub fn submit_owned_prompt_run_intent(
        &self,
        owner: &ConversationOwner,
        submission: SubmitPendingRunIntent<'_>,
    ) -> Result<PendingRunIntentSubmissionResult, HubError> {
        validate_owner(owner)?;
        required_id(submission.conversation_id, HubField::ConversationId)?;
        required(submission.content, HubField::Prompt, MAX_PROMPT_BYTES)?;
        required_id(submission.profile_id, HubField::ProjectExecutionProfileId)?;
        validate_idempotency_key(submission.idempotency_key)?;
        if submission.expected_version > i64::MAX as u64 {
            return Err(HubError::OutOfRange {
                field: HubField::ExpectedAggregateVersion,
                min: 0,
                max: usize::try_from(i64::MAX).unwrap_or(usize::MAX),
            });
        }
        let storage_key =
            owner_idempotency_key(owner, "pending_run_intent", submission.idempotency_key);
        let submission = SubmitPendingRunIntent {
            idempotency_key: &storage_key,
            ..submission
        };
        Ok(self
            .store
            .submit_owned_prompt_run_intent(owner, &submission)?)
    }

    /// Lists a bounded newest-first page of pending intents attached to one
    /// Conversation owned by this exact principal.
    ///
    /// # Errors
    ///
    /// Returns validation errors for malformed IDs or page limits, not-found
    /// for a missing or foreign Conversation, and storage errors on read
    /// failure.
    pub fn owned_pending_run_intent_page(
        &self,
        owner: &ConversationOwner,
        conversation_id: &str,
        before: Option<&PendingRunIntentCursor>,
        limit: usize,
    ) -> Result<PendingRunIntentPage, HubError> {
        validate_owner(owner)?;
        required_id(conversation_id, HubField::ConversationId)?;
        if !(1..=MAX_PENDING_RUN_INTENT_PAGE_LIMIT).contains(&limit) {
            return Err(HubError::OutOfRange {
                field: HubField::PendingRunIntentPageLimit,
                min: 1,
                max: MAX_PENDING_RUN_INTENT_PAGE_LIMIT,
            });
        }
        if let Some(cursor) = before {
            required_id(&cursor.intent_id, HubField::PendingRunIntentId)?;
            if cursor.submitted_at_ms > i64::MAX as u64 {
                return Err(HubError::OutOfRange {
                    field: HubField::PendingRunIntentId,
                    min: 0,
                    max: usize::try_from(i64::MAX).unwrap_or(usize::MAX),
                });
            }
        }
        Ok(self
            .store
            .owned_pending_run_intent_page(owner, conversation_id, before, limit)?)
    }

    /// Reads a bounded, payload-free timeline for one pending intent belonging
    /// to an owner-visible Conversation.
    ///
    /// # Errors
    ///
    /// Returns validation errors for malformed IDs, sequence numbers, or page
    /// limits; uniform not-found for a missing/foreign Conversation or intent;
    /// and storage errors on read failure.
    pub fn owned_pending_run_intent_timeline_page(
        &self,
        owner: &ConversationOwner,
        conversation_id: &str,
        intent_id: &str,
        after_sequence: u64,
        limit: usize,
    ) -> Result<PendingRunIntentTimelinePage, HubError> {
        validate_owner(owner)?;
        required_id(conversation_id, HubField::ConversationId)?;
        required_id(intent_id, HubField::PendingRunIntentId)?;
        if !(1..=MAX_PENDING_RUN_INTENT_TIMELINE_PAGE_LIMIT).contains(&limit) {
            return Err(HubError::OutOfRange {
                field: HubField::PendingRunIntentTimelinePageLimit,
                min: 1,
                max: MAX_PENDING_RUN_INTENT_TIMELINE_PAGE_LIMIT,
            });
        }
        if after_sequence > i64::MAX as u64 {
            return Err(HubError::OutOfRange {
                field: HubField::PendingRunIntentTimelineSequence,
                min: 0,
                max: usize::try_from(i64::MAX).unwrap_or(usize::MAX),
            });
        }
        Ok(self.store.owned_pending_run_intent_timeline_page(
            owner,
            conversation_id,
            intent_id,
            after_sequence,
            limit,
        )?)
    }

    /// Persists one immutable owner-to-Project execution consent record.
    ///
    /// The trusted control plane chooses the opaque profile ID and digest. This
    /// operation only records consent metadata; it creates no Run intent and
    /// invokes no provider, worker, device, or network operation.
    ///
    /// # Errors
    ///
    /// Returns validation errors for malformed identifiers or timestamps and
    /// structured storage errors for a missing Project, replay conflict, or
    /// unavailable Hub.
    pub fn grant_project_execution_consent(
        &self,
        owner: &ConversationOwner,
        project_id: &str,
        profile_id: &str,
        profile_sha256: &[u8; 32],
        expires_at_ms: u64,
        idempotency_key: &str,
    ) -> Result<ProjectExecutionConsentGrantResult, HubError> {
        validate_owner(owner)?;
        required_id(project_id, HubField::ProjectId)?;
        required_id(profile_id, HubField::ProjectExecutionProfileId)?;
        if expires_at_ms > i64::MAX as u64 {
            return Err(HubError::OutOfRange {
                field: HubField::ProjectExecutionGrantExpiry,
                min: 0,
                max: usize::try_from(i64::MAX).unwrap_or(usize::MAX),
            });
        }
        validate_idempotency_key(idempotency_key)?;
        let storage_key =
            owner_idempotency_key(owner, "project_execution_consent_grant", idempotency_key);
        Ok(self.store.grant_project_execution_consent(
            owner,
            project_id,
            profile_id,
            profile_sha256,
            expires_at_ms,
            &storage_key,
        )?)
    }

    /// Appends an immutable revocation event for one consent grant belonging
    /// to this exact verified owner.
    ///
    /// # Errors
    ///
    /// Returns validation errors for malformed IDs/keys and structured storage
    /// errors for a foreign/missing grant, replay conflict, or unavailable Hub.
    pub fn revoke_project_execution_consent(
        &self,
        owner: &ConversationOwner,
        grant_id: &str,
        idempotency_key: &str,
    ) -> Result<ProjectExecutionConsentRevocationResult, HubError> {
        validate_owner(owner)?;
        required_id(grant_id, HubField::ProjectExecutionGrantId)?;
        validate_idempotency_key(idempotency_key)?;
        let storage_key =
            owner_idempotency_key(owner, "project_execution_consent_revoke", idempotency_key);
        Ok(self
            .store
            .revoke_project_execution_consent(owner, grant_id, &storage_key)?)
    }
}
