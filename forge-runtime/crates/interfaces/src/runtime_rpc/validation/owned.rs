use crate::runtime_domain::{
    ConversationOwner, ConversationPromptCursor, ConversationScope,
    MAX_CONVERSATION_CHANGE_PAGE_LIMIT, MAX_CONVERSATION_OWNER_ISSUER_BYTES,
    MAX_CONVERSATION_OWNER_SUBJECT_BYTES, MAX_CONVERSATION_OWNER_TENANT_BYTES,
    MAX_CONVERSATION_PROMPT_PAGE_LIMIT, MAX_HUB_ENTITY_ID_BYTES, MAX_OWNED_CONVERSATION_PAGE_LIMIT,
    MAX_OWNED_RUN_PAGE_LIMIT, MAX_OWNED_RUN_TIMELINE_PAGE_LIMIT, MAX_PENDING_RUN_INTENT_PAGE_LIMIT,
    MAX_PENDING_RUN_INTENT_TIMELINE_PAGE_LIMIT, OwnedRunCursor, PendingRunIntentCursor,
};

use super::{Operation, RpcRequest, validate_write_header};

#[path = "owned_write.rs"]
mod write;
use write::validate_owned_write_request;

pub(super) fn default_owned_change_limit() -> usize {
    50
}

pub(super) fn validate_owned_operation(
    request: RpcRequest,
) -> Result<(String, Operation), (String, &'static str)> {
    if matches!(
        &request,
        RpcRequest::CreateOwnedConversation { .. }
            | RpcRequest::AppendOwnedPrompt { .. }
            | RpcRequest::SubmitOwnedPromptRunIntent { .. }
            | RpcRequest::ImportOwnedConversation { .. }
            | RpcRequest::GrantProjectExecutionConsent { .. }
            | RpcRequest::RevokeProjectExecutionConsent { .. }
    ) {
        validate_owned_write_request(request)
    } else {
        validate_owned_read_request(request)
    }
}

fn validate_owned_read_request(
    request: RpcRequest,
) -> Result<(String, Operation), (String, &'static str)> {
    match request {
        request @ (RpcRequest::OwnedRunPage { .. } | RpcRequest::OwnedRunTimelinePage { .. }) => {
            validate_owned_run_read_request(request)
        }
        request @ (RpcRequest::OwnedPendingRunIntentPage { .. }
        | RpcRequest::OwnedPendingRunIntentTimelinePage { .. }) => {
            validate_owned_pending_intent_read_request(request)
        }
        conversation_request => validate_owned_conversation_read_request(conversation_request),
    }
}

fn validate_owned_run_read_request(
    request: RpcRequest,
) -> Result<(String, Operation), (String, &'static str)> {
    match request {
        RpcRequest::OwnedRunPage {
            api_version,
            request_id,
            owner,
            conversation_id,
            before_created_at_ms,
            before_run_id,
            limit,
        } => validate_owned_run_page_request(
            &api_version,
            request_id,
            owner,
            conversation_id,
            before_created_at_ms,
            before_run_id,
            limit,
        ),
        RpcRequest::OwnedRunTimelinePage {
            api_version,
            request_id,
            owner,
            conversation_id,
            run_id,
            after_sequence,
            limit,
        } => validate_owned_run_timeline_page_request(
            &api_version,
            request_id,
            owner,
            conversation_id,
            run_id,
            after_sequence,
            limit,
        ),
        _ => Err(("invalid".into(), "invalid_request")),
    }
}

fn validate_owned_pending_intent_read_request(
    request: RpcRequest,
) -> Result<(String, Operation), (String, &'static str)> {
    match request {
        RpcRequest::OwnedPendingRunIntentPage {
            api_version,
            request_id,
            owner,
            conversation_id,
            before,
            limit,
        } => validate_owned_pending_run_intent_page_request(
            &api_version,
            request_id,
            owner,
            conversation_id,
            before,
            limit,
        ),
        RpcRequest::OwnedPendingRunIntentTimelinePage {
            api_version,
            request_id,
            owner,
            conversation_id,
            intent_id,
            after_sequence,
            limit,
        } => validate_owned_pending_run_intent_timeline_request(
            &api_version,
            request_id,
            owner,
            conversation_id,
            intent_id,
            after_sequence,
            limit,
        ),
        _ => Err(("invalid".into(), "invalid_request")),
    }
}

fn validate_owned_conversation_read_request(
    request: RpcRequest,
) -> Result<(String, Operation), (String, &'static str)> {
    match request {
        RpcRequest::ListOwnedConversations {
            api_version,
            request_id,
            owner,
            after_id,
            limit,
        } => validate_list_owned_request(&api_version, request_id, owner, after_id, limit),
        RpcRequest::OwnedConversationPromptPage {
            api_version,
            request_id,
            owner,
            conversation_id,
            before,
            limit,
        } => validate_owned_prompt_page_request(
            &api_version,
            request_id,
            owner,
            conversation_id,
            before,
            limit,
        ),
        RpcRequest::OwnedProjectConversationIdentity {
            api_version,
            request_id,
            owner,
            conversation_id,
        } => validate_owned_project_identity_request(
            &api_version,
            request_id,
            owner,
            conversation_id,
        ),
        RpcRequest::OwnedConversationChangesAfter {
            api_version,
            request_id,
            owner,
            after_cursor,
            limit,
        } => validate_owned_changes_request(&api_version, request_id, owner, after_cursor, limit),
        _ => Err(("invalid".into(), "invalid_request")),
    }
}

fn validate_list_owned_request(
    api_version: &str,
    request_id: String,
    owner: ConversationOwner,
    after_id: Option<String>,
    limit: usize,
) -> Result<(String, Operation), (String, &'static str)> {
    validate_write_header(api_version, &request_id)?;
    if !(1..=MAX_OWNED_CONVERSATION_PAGE_LIMIT).contains(&limit)
        || after_id
            .as_ref()
            .is_some_and(|id| id.trim().is_empty() || id.len() > MAX_HUB_ENTITY_ID_BYTES)
        || !valid_conversation_owner(&owner)
    {
        return Err((request_id, "invalid_owned_conversation_request"));
    }
    Ok((
        request_id,
        Operation::ListOwnedConversations {
            owner,
            after_id,
            limit,
        },
    ))
}

fn validate_owned_prompt_page_request(
    api_version: &str,
    request_id: String,
    owner: ConversationOwner,
    conversation_id: String,
    before: Option<ConversationPromptCursor>,
    limit: usize,
) -> Result<(String, Operation), (String, &'static str)> {
    validate_write_header(api_version, &request_id)?;
    if !(1..=MAX_CONVERSATION_PROMPT_PAGE_LIMIT).contains(&limit)
        || conversation_id.trim().is_empty()
        || conversation_id.len() > MAX_HUB_ENTITY_ID_BYTES
        || !valid_conversation_owner(&owner)
        || before.as_ref().is_some_and(|cursor| {
            cursor.created_at_ms > i64::MAX as u64
                || cursor.prompt_id.trim().is_empty()
                || cursor.prompt_id.len() > MAX_HUB_ENTITY_ID_BYTES
        })
    {
        return Err((request_id, "invalid_prompt_request"));
    }
    Ok((
        request_id,
        Operation::OwnedPromptPage {
            owner,
            conversation_id,
            before,
            limit,
        },
    ))
}

fn validate_owned_project_identity_request(
    api_version: &str,
    request_id: String,
    owner: ConversationOwner,
    conversation_id: String,
) -> Result<(String, Operation), (String, &'static str)> {
    validate_write_header(api_version, &request_id)?;
    if !valid_entity_id(&conversation_id) || !valid_conversation_owner(&owner) {
        return Err((request_id, "invalid_owned_conversation_request"));
    }
    Ok((
        request_id,
        Operation::OwnedProjectConversationIdentity {
            owner,
            conversation_id,
        },
    ))
}

fn validate_owned_run_page_request(
    api_version: &str,
    request_id: String,
    owner: ConversationOwner,
    conversation_id: String,
    before_created_at_ms: Option<u64>,
    before_run_id: Option<String>,
    limit: usize,
) -> Result<(String, Operation), (String, &'static str)> {
    validate_write_header(api_version, &request_id)?;
    let before = match (before_created_at_ms, before_run_id) {
        (None, None) => None,
        (Some(created_at_ms), Some(run_id))
            if i64::try_from(created_at_ms).is_ok() && valid_entity_id(&run_id) =>
        {
            Some(OwnedRunCursor {
                created_at_ms,
                run_id,
            })
        }
        _ => return Err((request_id, "invalid_owned_run_request")),
    };
    if !(1..=MAX_OWNED_RUN_PAGE_LIMIT).contains(&limit)
        || !valid_entity_id(&conversation_id)
        || !valid_conversation_owner(&owner)
    {
        return Err((request_id, "invalid_owned_run_request"));
    }
    Ok((
        request_id,
        Operation::OwnedRunPage {
            owner,
            conversation_id,
            before,
            limit,
        },
    ))
}

fn validate_owned_run_timeline_page_request(
    api_version: &str,
    request_id: String,
    owner: ConversationOwner,
    conversation_id: String,
    run_id: String,
    after_sequence: u64,
    limit: usize,
) -> Result<(String, Operation), (String, &'static str)> {
    validate_write_header(api_version, &request_id)?;
    if !(1..=MAX_OWNED_RUN_TIMELINE_PAGE_LIMIT).contains(&limit)
        || i64::try_from(after_sequence).is_err()
        || !valid_entity_id(&conversation_id)
        || !valid_entity_id(&run_id)
        || !valid_conversation_owner(&owner)
    {
        return Err((request_id, "invalid_owned_run_request"));
    }
    Ok((
        request_id,
        Operation::OwnedRunTimelinePage {
            owner,
            conversation_id,
            run_id,
            after_sequence,
            limit,
        },
    ))
}

fn validate_owned_pending_run_intent_page_request(
    api_version: &str,
    request_id: String,
    owner: ConversationOwner,
    conversation_id: String,
    before: Option<PendingRunIntentCursor>,
    limit: usize,
) -> Result<(String, Operation), (String, &'static str)> {
    validate_write_header(api_version, &request_id)?;
    if !(1..=MAX_PENDING_RUN_INTENT_PAGE_LIMIT).contains(&limit)
        || !valid_entity_id(&conversation_id)
        || !valid_conversation_owner(&owner)
        || before.as_ref().is_some_and(|cursor| {
            i64::try_from(cursor.submitted_at_ms).is_err() || !valid_entity_id(&cursor.intent_id)
        })
    {
        return Err((request_id, "invalid_pending_run_intent_request"));
    }
    Ok((
        request_id,
        Operation::OwnedPendingRunIntentPage {
            owner,
            conversation_id,
            before,
            limit,
        },
    ))
}

fn validate_owned_pending_run_intent_timeline_request(
    api_version: &str,
    request_id: String,
    owner: ConversationOwner,
    conversation_id: String,
    intent_id: String,
    after_sequence: u64,
    limit: usize,
) -> Result<(String, Operation), (String, &'static str)> {
    validate_write_header(api_version, &request_id)?;
    if !(1..=MAX_PENDING_RUN_INTENT_TIMELINE_PAGE_LIMIT).contains(&limit)
        || i64::try_from(after_sequence).is_err()
        || !valid_entity_id(&conversation_id)
        || !valid_entity_id(&intent_id)
        || !valid_conversation_owner(&owner)
    {
        return Err((request_id, "invalid_pending_run_intent_request"));
    }
    Ok((
        request_id,
        Operation::OwnedPendingRunIntentTimelinePage {
            owner,
            conversation_id,
            intent_id,
            after_sequence,
            limit,
        },
    ))
}

fn validate_owned_changes_request(
    api_version: &str,
    request_id: String,
    owner: ConversationOwner,
    after_cursor: u64,
    limit: usize,
) -> Result<(String, Operation), (String, &'static str)> {
    validate_write_header(api_version, &request_id)?;
    if !(1..=MAX_CONVERSATION_CHANGE_PAGE_LIMIT).contains(&limit)
        || i64::try_from(after_cursor).is_err()
        || !valid_conversation_owner(&owner)
    {
        return Err((request_id, "invalid_owned_conversation_request"));
    }
    Ok((
        request_id,
        Operation::OwnedChangesAfter {
            owner,
            after_cursor,
            limit,
        },
    ))
}

fn valid_conversation_owner(owner: &ConversationOwner) -> bool {
    valid_owner_component(&owner.issuer, MAX_CONVERSATION_OWNER_ISSUER_BYTES)
        && valid_owner_component(&owner.subject, MAX_CONVERSATION_OWNER_SUBJECT_BYTES)
        && valid_owner_component(&owner.tenant_id, MAX_CONVERSATION_OWNER_TENANT_BYTES)
}

fn valid_owner_component(value: &str, maximum_bytes: usize) -> bool {
    !value.trim().is_empty() && value.len() <= maximum_bytes && !value.chars().any(char::is_control)
}

fn valid_entity_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_HUB_ENTITY_ID_BYTES
        && !value.chars().any(char::is_control)
}

fn valid_conversation_scope(scope: &ConversationScope) -> bool {
    match scope {
        ConversationScope::Global => true,
        ConversationScope::Project(id) | ConversationScope::Group(id) => {
            !id.trim().is_empty() && id.len() <= MAX_HUB_ENTITY_ID_BYTES
        }
    }
}
