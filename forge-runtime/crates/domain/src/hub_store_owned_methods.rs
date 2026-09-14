macro_rules! owned_conversation_methods {
    () => {
    /// Creates a Conversation and binds its verified owner in the same Hub
    /// transaction. Existing ownerless Conversations are never auto-claimed.
    ///
    /// # Errors
    ///
    /// Returns a conflict for a mismatched idempotent replay or an existing
    /// Conversation owned by another principal.
    fn create_owned_conversation(
        &self,
        owner: &ConversationOwner,
        scope: &ConversationScope,
        title: &str,
        idempotency_key: &str,
    ) -> Result<Conversation, HubStoreError> {
        let _ = (owner, scope, title, idempotency_key);
        Err(HubStoreError::Unavailable {
            message: "owner-bound Conversation writes are not supported by this Hub store".into(),
        })
    }

    /// Atomically copies a bounded user-visible transcript into a new owner-bound
    /// Global Conversation. The source remains local and is never modified.
    ///
    /// # Errors
    ///
    /// Returns `Conflict` for a mismatched idempotent replay and `Unavailable`
    /// when this Hub store does not support owner-bound imports.
    fn import_owned_conversation(
        &self,
        owner: &ConversationOwner,
        title: &str,
        prompts: &[ConversationImportPrompt],
        idempotency_key: &str,
    ) -> Result<OwnedConversationImportResult, HubStoreError> {
        let _ = (owner, title, prompts, idempotency_key);
        Err(HubStoreError::Unavailable {
            message: "owner-bound Conversation import is not supported by this Hub store".into(),
        })
    }

    /// Lists only the verified principal's Conversations in stable binary-ID
    /// order. The cursor is owner-local and carries no global journal offset.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the owner-filtered page cannot be read.
    fn list_owned_conversations(
        &self,
        owner: &ConversationOwner,
        after_id: Option<&str>,
        limit: usize,
    ) -> Result<OwnedConversationPage, HubStoreError> {
        let _ = (owner, after_id, limit);
        Err(HubStoreError::Unavailable {
            message: "owner-bound Conversation reads are not supported by this Hub store".into(),
        })
    }

    /// Resolves the Project scope of a Conversation owned by this exact
    /// principal. The result contains only opaque Hub IDs, never path, title,
    /// or Prompt content.
    ///
    /// # Errors
    ///
    /// Returns uniform not-found for missing, ownerless, or foreign
    /// Conversations; conflict for an owner-visible non-Project scope;
    /// unavailable when unsupported.
    fn owned_project_conversation_identity(
        &self,
        owner: &ConversationOwner,
        conversation_id: &str,
    ) -> Result<OwnedProjectConversationIdentity, HubStoreError> {
        let _ = (owner, conversation_id);
        Err(HubStoreError::Unavailable {
            message: "owner-bound Project Conversation identity reads are not supported by this Hub store".into(),
        })
    }

    /// Reads a newest-first bounded Run summary page for one Conversation
    /// owned by this exact principal. Both owner and Conversation are checked
    /// in the same read snapshot; only scalar status metadata is projected.
    ///
    /// # Errors
    ///
    /// Returns a storage error when ownership, paging, or stored Run metadata
    /// cannot be validated.
    fn owned_run_page(
        &self,
        owner: &ConversationOwner,
        conversation_id: &str,
        before: Option<&OwnedRunCursor>,
        limit: usize,
    ) -> Result<OwnedRunPage, HubStoreError> {
        let _ = (owner, conversation_id, before, limit);
        Err(HubStoreError::Unavailable {
            message: "owner-bound Run reads are not supported by this Hub store".into(),
        })
    }

    /// Reads a bounded payload-free `RuntimeEvent` type page for one Run in an
    /// owner-visible Conversation using one deferred read snapshot.
    ///
    /// # Errors
    ///
    /// Returns a storage error when ownership, Run membership, paging, or a
    /// stored event envelope cannot be validated.
    fn owned_run_timeline_page(
        &self,
        owner: &ConversationOwner,
        conversation_id: &str,
        run_id: &str,
        after_sequence: u64,
        limit: usize,
    ) -> Result<OwnedRunTimelinePage, HubStoreError> {
        let _ = (owner, conversation_id, run_id, after_sequence, limit);
        Err(HubStoreError::Unavailable {
            message: "owner-bound Run timeline reads are not supported by this Hub store".into(),
        })
    }

    /// Reads a bounded Prompt page only when this exact principal owns the
    /// Conversation. Ownership and history are checked in one read snapshot.
    ///
    /// # Errors
    ///
    /// Returns a storage error when ownership or the requested page cannot be read.
    fn owned_conversation_prompt_page(
        &self,
        owner: &ConversationOwner,
        conversation_id: &str,
        before: Option<&ConversationPromptCursor>,
        limit: usize,
    ) -> Result<ConversationPromptPage, HubStoreError> {
        let _ = (owner, conversation_id, before, limit);
        Err(HubStoreError::Unavailable {
            message: "owner-bound Prompt reads are not supported by this Hub store".into(),
        })
    }

    /// Appends one fixed-role user Prompt after an exact owner and aggregate
    /// version check. Identical idempotent retries replay even after the head
    /// advances; new writes must match `expected_version`.
    ///
    /// # Errors
    ///
    /// Returns a storage error when ownership, version, or idempotency validation fails.
    fn append_owned_prompt(
        &self,
        owner: &ConversationOwner,
        conversation_id: &str,
        content: &str,
        idempotency_key: &str,
        expected_version: u64,
    ) -> Result<OwnedPromptAppendResult, HubStoreError> {
        let _ = (
            owner,
            conversation_id,
            content,
            idempotency_key,
            expected_version,
        );
        Err(HubStoreError::Unavailable {
            message: "owner-bound Prompt writes are not supported by this Hub store".into(),
        })
    }

    /// Creates or exactly replays an immutable owner-to-Project execution
    /// consent grant. Project existence, owner binding, and idempotency are
    /// checked in one write transaction. The Hub treats the profile ID and
    /// SHA-256 as opaque values and does not grant execution by itself.
    ///
    /// # Errors
    ///
    /// Returns a structured error for a missing Project, mismatched replay,
    /// invalid bounded expiry, or unavailable storage.
    fn grant_project_execution_consent(
        &self,
        owner: &ConversationOwner,
        project_id: &str,
        profile_id: &str,
        profile_sha256: &[u8; 32],
        expires_at_ms: u64,
        idempotency_key: &str,
    ) -> Result<ProjectExecutionConsentGrantResult, HubStoreError> {
        let _ = (
            owner,
            project_id,
            profile_id,
            profile_sha256,
            expires_at_ms,
            idempotency_key,
        );
        Err(HubStoreError::Unavailable {
            message: "Project execution consent is not supported by this Hub store".into(),
        })
    }

    /// Appends one immutable revocation event for a grant belonging to the
    /// exact owner. Replays are keyed by the owner-scoped idempotency key.
    ///
    /// # Errors
    ///
    /// Returns not found for a foreign/missing grant, conflict for a divergent
    /// replay or second revocation, or unavailable storage.
    fn revoke_project_execution_consent(
        &self,
        owner: &ConversationOwner,
        grant_id: &str,
        idempotency_key: &str,
    ) -> Result<ProjectExecutionConsentRevocationResult, HubStoreError> {
        let _ = (owner, grant_id, idempotency_key);
        Err(HubStoreError::Unavailable {
            message: "Project execution consent revocation is not supported by this Hub store"
                .into(),
        })
    }

    /// Atomically inserts an owner-bound Prompt and a distinct inert pending
    /// Run intent after Project-scope, aggregate-version, profile, and exact
    /// unexpired consent validation. Implementations must not create Run rows.
    ///
    /// # Errors
    ///
    /// Returns uniform not-found for an unowned Conversation, conflict for
    /// scope/CAS/consent/profile failures, and unavailable for unsupported stores.
    fn submit_owned_prompt_run_intent(
        &self,
        owner: &ConversationOwner,
        submission: &SubmitPendingRunIntent<'_>,
    ) -> Result<PendingRunIntentSubmissionResult, HubStoreError> {
        let _ = (owner, submission);
        Err(HubStoreError::Unavailable {
            message: "pending Run intent submission is not supported by this Hub store".into(),
        })
    }

    /// Lists pending-intent summaries in one owner-visible Conversation using
    /// a stable bounded newest-first cursor.
    ///
    /// # Errors
    ///
    /// Returns uniform not-found for an unowned Conversation or unavailable
    /// when the store cannot read the projection.
    fn owned_pending_run_intent_page(
        &self,
        owner: &ConversationOwner,
        conversation_id: &str,
        before: Option<&PendingRunIntentCursor>,
        limit: usize,
    ) -> Result<PendingRunIntentPage, HubStoreError> {
        let _ = (owner, conversation_id, before, limit);
        Err(HubStoreError::Unavailable {
            message: "pending Run intent reads are not supported by this Hub store".into(),
        })
    }

    /// Reads a bounded payload-free timeline for one intent in an owner-visible
    /// Conversation. Pending-intent timelines remain separate from Run events.
    ///
    /// # Errors
    ///
    /// Returns uniform not-found for a missing/foreign Conversation or intent,
    /// or unavailable when the store cannot read the projection.
    fn owned_pending_run_intent_timeline_page(
        &self,
        owner: &ConversationOwner,
        conversation_id: &str,
        intent_id: &str,
        after_sequence: u64,
        limit: usize,
    ) -> Result<PendingRunIntentTimelinePage, HubStoreError> {
        let _ = (owner, conversation_id, intent_id, after_sequence, limit);
        Err(HubStoreError::Unavailable {
            message: "pending Run intent timelines are not supported by this Hub store".into(),
        })
    }

    /// Reads only changes belonging to Conversations owned by this exact
    /// principal. Its dense cursor is local to this owner and reveals no
    /// Hub-global change position or foreign activity.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the owner-filtered page cannot be read.
    fn owned_conversation_changes_after(
        &self,
        owner: &ConversationOwner,
        after_cursor: u64,
        limit: usize,
    ) -> Result<OwnedConversationChangePage, HubStoreError> {
        let _ = (owner, after_cursor, limit);
        Err(HubStoreError::Unavailable {
            message: "owner-bound Conversation change reads are not supported by this Hub store"
                .into(),
        })
    }

    };
}
