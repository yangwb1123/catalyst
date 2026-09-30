use super::{
    ConversationBootstrapCursor, ConversationImportPrompt, ConversationOwner,
    ConversationPromptCursor, ConversationScope, OwnedRunCursor, PendingRunIntentCursor,
};

pub(in crate::runtime_rpc) enum Operation {
    SnapshotAtCursor,
    ChangesAfter {
        after_cursor: u64,
        limit: usize,
    },
    OwnedChangesAfter {
        owner: ConversationOwner,
        after_cursor: u64,
        limit: usize,
    },
    PromptPage {
        conversation_id: String,
        before: Option<ConversationPromptCursor>,
        limit: usize,
    },
    BootstrapPage {
        cursor: Option<ConversationBootstrapCursor>,
        limit: usize,
    },
    CreateOwnedConversation {
        owner: ConversationOwner,
        scope: ConversationScope,
        title: String,
        idempotency_key: String,
    },
    ListOwnedConversations {
        owner: ConversationOwner,
        after_id: Option<String>,
        limit: usize,
    },
    GetOwnedConversation {
        owner: ConversationOwner,
        conversation_id: String,
    },
    OwnedPromptPage {
        owner: ConversationOwner,
        conversation_id: String,
        before: Option<ConversationPromptCursor>,
        limit: usize,
    },
    OwnedProjectConversationIdentity {
        owner: ConversationOwner,
        conversation_id: String,
    },
    OwnedRunPage {
        owner: ConversationOwner,
        conversation_id: String,
        before: Option<OwnedRunCursor>,
        limit: usize,
    },
    OwnedRunObservation {
        owner: ConversationOwner,
        conversation_id: String,
        run_id: String,
    },
    OwnedRunTimelinePage {
        owner: ConversationOwner,
        conversation_id: String,
        run_id: String,
        after_sequence: u64,
        limit: usize,
    },
    OwnedPendingRunIntentPage {
        owner: ConversationOwner,
        conversation_id: String,
        before: Option<PendingRunIntentCursor>,
        limit: usize,
    },
    OwnedPendingRunIntentTimelinePage {
        owner: ConversationOwner,
        conversation_id: String,
        intent_id: String,
        after_sequence: u64,
        limit: usize,
    },
    AppendOwnedPrompt {
        owner: ConversationOwner,
        conversation_id: String,
        content: String,
        idempotency_key: String,
        expected_version: u64,
    },
    SubmitOwnedPromptRunIntent {
        owner: ConversationOwner,
        conversation_id: String,
        content: String,
        idempotency_key: String,
        expected_version: u64,
        profile_id: String,
        profile_sha256: [u8; 32],
    },
    ImportOwnedConversation {
        owner: ConversationOwner,
        title: String,
        prompts: Vec<ConversationImportPrompt>,
        idempotency_key: String,
    },
    GrantProjectExecutionConsent {
        owner: ConversationOwner,
        project_id: String,
        profile_id: String,
        profile_sha256: [u8; 32],
        expires_at_ms: u64,
        idempotency_key: String,
    },
    RevokeProjectExecutionConsent {
        owner: ConversationOwner,
        grant_id: String,
        idempotency_key: String,
    },
}

impl Operation {
    pub(in crate::runtime_rpc) fn requires_write(&self) -> bool {
        matches!(
            self,
            Self::CreateOwnedConversation { .. }
                | Self::AppendOwnedPrompt { .. }
                | Self::SubmitOwnedPromptRunIntent { .. }
                | Self::ImportOwnedConversation { .. }
                | Self::GrantProjectExecutionConsent { .. }
                | Self::RevokeProjectExecutionConsent { .. }
        )
    }

    pub(in crate::runtime_rpc) fn uses_owned_errors(&self) -> bool {
        matches!(
            self,
            Self::CreateOwnedConversation { .. }
                | Self::ListOwnedConversations { .. }
                | Self::GetOwnedConversation { .. }
                | Self::OwnedPromptPage { .. }
                | Self::OwnedProjectConversationIdentity { .. }
                | Self::OwnedRunPage { .. }
                | Self::OwnedRunObservation { .. }
                | Self::OwnedRunTimelinePage { .. }
                | Self::OwnedPendingRunIntentPage { .. }
                | Self::OwnedPendingRunIntentTimelinePage { .. }
                | Self::OwnedChangesAfter { .. }
                | Self::AppendOwnedPrompt { .. }
                | Self::SubmitOwnedPromptRunIntent { .. }
                | Self::ImportOwnedConversation { .. }
                | Self::GrantProjectExecutionConsent { .. }
                | Self::RevokeProjectExecutionConsent { .. }
        )
    }
}
