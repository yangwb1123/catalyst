use serde::Deserialize;

use crate::runtime_domain::{
    ConversationBootstrapCursor, ConversationImportPrompt, ConversationOwner,
    ConversationPromptCursor, ConversationScope, MAX_CONVERSATION_BOOTSTRAP_PAGE_LIMIT,
    MAX_CONVERSATION_PROMPT_PAGE_LIMIT, MAX_HUB_ENTITY_ID_BYTES, OwnedRunCursor,
    PendingRunIntentCursor,
};

use super::{
    API_VERSION, MAX_CHANGE_LIMIT, MAX_READ_REQUEST_BYTES, MAX_REQUEST_BYTES, MAX_REQUEST_ID_BYTES,
    WRITE_API_VERSION,
};

#[path = "validation_headers.rs"]
mod headers;
mod operation;
mod owned;
pub(super) use operation::Operation;

const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

pub(super) use headers::{validate_header, validate_write_header};

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum RpcRequest {
    SnapshotAtCursor {
        api_version: String,
        request_id: String,
    },
    ConversationChangesAfter {
        api_version: String,
        request_id: String,
        after_cursor: u64,
        limit: usize,
    },
    OwnedConversationChangesAfter {
        api_version: String,
        request_id: String,
        owner: ConversationOwner,
        after_cursor: u64,
        #[serde(default = "owned::default_owned_change_limit")]
        limit: usize,
    },
    ConversationPromptPage {
        api_version: String,
        request_id: String,
        conversation_id: String,
        #[serde(default)]
        before: Option<ConversationPromptCursor>,
        limit: usize,
    },
    ConversationBootstrapPage {
        api_version: String,
        request_id: String,
        #[serde(default)]
        cursor: Option<ConversationBootstrapCursor>,
        limit: usize,
    },
    CreateOwnedConversation {
        api_version: String,
        request_id: String,
        owner: ConversationOwner,
        scope: ConversationScope,
        title: String,
        idempotency_key: String,
    },
    ListOwnedConversations {
        api_version: String,
        request_id: String,
        owner: ConversationOwner,
        #[serde(default)]
        after_id: Option<String>,
        limit: usize,
    },
    GetOwnedConversation {
        api_version: String,
        request_id: String,
        owner: ConversationOwner,
        conversation_id: String,
    },
    OwnedConversationPromptPage {
        api_version: String,
        request_id: String,
        owner: ConversationOwner,
        conversation_id: String,
        #[serde(default)]
        before: Option<ConversationPromptCursor>,
        limit: usize,
    },
    OwnedProjectConversationIdentity {
        api_version: String,
        request_id: String,
        owner: ConversationOwner,
        conversation_id: String,
    },
    OwnedRunPage {
        api_version: String,
        request_id: String,
        owner: ConversationOwner,
        conversation_id: String,
        #[serde(default)]
        before_created_at_ms: Option<u64>,
        #[serde(default)]
        before_run_id: Option<String>,
        limit: usize,
    },
    OwnedRunObservation {
        api_version: String,
        request_id: String,
        owner: ConversationOwner,
        conversation_id: String,
        run_id: String,
    },
    OwnedRunTimelinePage {
        api_version: String,
        request_id: String,
        owner: ConversationOwner,
        conversation_id: String,
        run_id: String,
        after_sequence: u64,
        limit: usize,
    },
    OwnedPendingRunIntentPage {
        api_version: String,
        request_id: String,
        owner: ConversationOwner,
        conversation_id: String,
        #[serde(default)]
        before: Option<PendingRunIntentCursor>,
        limit: usize,
    },
    OwnedPendingRunIntentTimelinePage {
        api_version: String,
        request_id: String,
        owner: ConversationOwner,
        conversation_id: String,
        intent_id: String,
        after_sequence: u64,
        limit: usize,
    },
    AppendOwnedPrompt {
        api_version: String,
        request_id: String,
        owner: ConversationOwner,
        conversation_id: String,
        content: String,
        idempotency_key: String,
        expected_version: u64,
    },
    SubmitOwnedPromptRunIntent {
        api_version: String,
        request_id: String,
        owner: ConversationOwner,
        conversation_id: String,
        content: String,
        idempotency_key: String,
        expected_version: u64,
        profile_id: String,
        profile_sha256: [u8; 32],
    },
    ImportOwnedConversation {
        api_version: String,
        request_id: String,
        owner: ConversationOwner,
        title: String,
        prompts: Vec<ConversationImportPrompt>,
        idempotency_key: String,
    },
    GrantProjectExecutionConsent {
        api_version: String,
        request_id: String,
        owner: ConversationOwner,
        project_id: String,
        profile_id: String,
        profile_sha256: [u8; 32],
        expires_at_ms: u64,
        idempotency_key: String,
    },
    RevokeProjectExecutionConsent {
        api_version: String,
        request_id: String,
        owner: ConversationOwner,
        grant_id: String,
        idempotency_key: String,
    },
}

impl RpcRequest {
    pub(super) fn response_api_version(&self) -> &'static str {
        match self {
            Self::CreateOwnedConversation { .. }
            | Self::ListOwnedConversations { .. }
            | Self::GetOwnedConversation { .. }
            | Self::OwnedConversationPromptPage { .. }
            | Self::OwnedProjectConversationIdentity { .. }
            | Self::OwnedRunPage { .. }
            | Self::OwnedRunObservation { .. }
            | Self::OwnedRunTimelinePage { .. }
            | Self::OwnedPendingRunIntentPage { .. }
            | Self::OwnedPendingRunIntentTimelinePage { .. }
            | Self::OwnedConversationChangesAfter { .. }
            | Self::AppendOwnedPrompt { .. }
            | Self::SubmitOwnedPromptRunIntent { .. }
            | Self::ImportOwnedConversation { .. }
            | Self::GrantProjectExecutionConsent { .. }
            | Self::RevokeProjectExecutionConsent { .. } => WRITE_API_VERSION,
            Self::SnapshotAtCursor { .. }
            | Self::ConversationChangesAfter { .. }
            | Self::ConversationPromptPage { .. }
            | Self::ConversationBootstrapPage { .. } => API_VERSION,
        }
    }
}

pub(super) fn validate_request(
    request: RpcRequest,
    request_bytes: usize,
) -> Result<(String, Operation), (String, &'static str)> {
    let (request_id, operation) = validate_operation(request)?;
    let maximum = if operation.requires_write() {
        MAX_REQUEST_BYTES
    } else {
        MAX_READ_REQUEST_BYTES
    };
    if request_bytes + 1 > maximum {
        return Err((request_id, "request_too_large"));
    }
    Ok((request_id, operation))
}

fn validate_operation(request: RpcRequest) -> Result<(String, Operation), (String, &'static str)> {
    match request {
        RpcRequest::SnapshotAtCursor {
            api_version,
            request_id,
        } => validate_snapshot_request(&api_version, request_id),
        RpcRequest::ConversationChangesAfter {
            api_version,
            request_id,
            after_cursor,
            limit,
        } => validate_change_request(&api_version, request_id, after_cursor, limit),
        RpcRequest::ConversationPromptPage {
            api_version,
            request_id,
            conversation_id,
            before,
            limit,
        } => validate_prompt_page_request(&api_version, request_id, conversation_id, before, limit),
        RpcRequest::ConversationBootstrapPage {
            api_version,
            request_id,
            cursor,
            limit,
        } => validate_bootstrap_page_request(&api_version, request_id, cursor, limit),
        owned_request => owned::validate_owned_operation(owned_request),
    }
}

fn validate_bootstrap_page_request(
    api_version: &str,
    request_id: String,
    cursor: Option<ConversationBootstrapCursor>,
    limit: usize,
) -> Result<(String, Operation), (String, &'static str)> {
    validate_header(api_version, &request_id)?;
    if !(1..=MAX_CONVERSATION_BOOTSTRAP_PAGE_LIMIT).contains(&limit) {
        return Err((request_id, "invalid_limit"));
    }
    let valid_cursor = cursor.as_ref().is_none_or(|cursor| {
        if cursor.snapshot_cursor > i64::MAX as u64 {
            return false;
        }
        match cursor.phase {
            crate::runtime_domain::ConversationBootstrapPhase::LegacyBaseline => {
                cursor.after_change_cursor.is_none()
                    && cursor.after_conversation_id.as_ref().is_some_and(|id| {
                        !id.trim().is_empty() && id.len() <= MAX_HUB_ENTITY_ID_BYTES
                    })
            }
            crate::runtime_domain::ConversationBootstrapPhase::ChangeLog => {
                cursor.after_conversation_id.is_none()
                    && cursor
                        .after_change_cursor
                        .is_some_and(|position| position <= cursor.snapshot_cursor)
            }
        }
    });
    if !valid_cursor {
        return Err((request_id, "invalid_cursor"));
    }
    Ok((request_id, Operation::BootstrapPage { cursor, limit }))
}

fn validate_snapshot_request(
    api_version: &str,
    request_id: String,
) -> Result<(String, Operation), (String, &'static str)> {
    validate_header(api_version, &request_id)?;
    Ok((request_id, Operation::SnapshotAtCursor))
}

fn validate_change_request(
    api_version: &str,
    request_id: String,
    after_cursor: u64,
    limit: usize,
) -> Result<(String, Operation), (String, &'static str)> {
    validate_header(api_version, &request_id)?;
    if after_cursor > MAX_SAFE_JSON_INTEGER {
        return Err((request_id, "invalid_cursor"));
    }
    if !(1..=MAX_CHANGE_LIMIT).contains(&limit) {
        return Err((request_id, "invalid_limit"));
    }
    Ok((
        request_id,
        Operation::ChangesAfter {
            after_cursor,
            limit,
        },
    ))
}

fn validate_prompt_page_request(
    api_version: &str,
    request_id: String,
    conversation_id: String,
    before: Option<ConversationPromptCursor>,
    limit: usize,
) -> Result<(String, Operation), (String, &'static str)> {
    validate_header(api_version, &request_id)?;
    if !(1..=MAX_CONVERSATION_PROMPT_PAGE_LIMIT).contains(&limit) {
        return Err((request_id, "invalid_limit"));
    }
    if conversation_id.trim().is_empty()
        || conversation_id.len() > MAX_HUB_ENTITY_ID_BYTES
        || before.as_ref().is_some_and(|cursor| {
            cursor.created_at_ms > MAX_SAFE_JSON_INTEGER
                || cursor.prompt_id.trim().is_empty()
                || cursor.prompt_id.len() > MAX_HUB_ENTITY_ID_BYTES
        })
    {
        return Err((request_id, "invalid_prompt_request"));
    }
    Ok((
        request_id,
        Operation::PromptPage {
            conversation_id,
            before,
            limit,
        },
    ))
}
