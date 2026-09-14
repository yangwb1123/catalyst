use crate::runtime_domain::{
    ConversationImportPrompt, ConversationOwner, ConversationScope,
    MAX_CONVERSATION_IMPORT_PROMPT_COUNT, MAX_HUB_ENTITY_ID_BYTES, MAX_PROMPT_CONTENT_BYTES,
};

use super::{
    Operation, RpcRequest, valid_conversation_owner, valid_conversation_scope, valid_entity_id,
    validate_write_header,
};

pub(super) fn validate_owned_write_request(
    request: RpcRequest,
) -> Result<(String, Operation), (String, &'static str)> {
    match request {
        request @ (RpcRequest::CreateOwnedConversation { .. }
        | RpcRequest::AppendOwnedPrompt { .. }
        | RpcRequest::SubmitOwnedPromptRunIntent { .. }
        | RpcRequest::ImportOwnedConversation { .. }) => {
            validate_owned_conversation_write_request(request)
        }
        request @ (RpcRequest::GrantProjectExecutionConsent { .. }
        | RpcRequest::RevokeProjectExecutionConsent { .. }) => {
            validate_owned_consent_write_request(request)
        }
        _ => Err(("invalid".into(), "invalid_request")),
    }
}

fn validate_owned_conversation_write_request(
    request: RpcRequest,
) -> Result<(String, Operation), (String, &'static str)> {
    match request {
        RpcRequest::CreateOwnedConversation {
            api_version,
            request_id,
            owner,
            scope,
            title,
            idempotency_key,
        } => validate_create_owned_request(
            &api_version,
            request_id,
            owner,
            scope,
            title,
            idempotency_key,
        ),
        request @ (RpcRequest::AppendOwnedPrompt { .. }
        | RpcRequest::SubmitOwnedPromptRunIntent { .. }) => {
            validate_owned_prompt_write_request(request)
        }
        RpcRequest::ImportOwnedConversation {
            api_version,
            request_id,
            owner,
            title,
            prompts,
            idempotency_key,
        } => validate_import_owned_conversation_request(
            &api_version,
            request_id,
            owner,
            title,
            prompts,
            idempotency_key,
        ),
        _ => Err(("invalid".into(), "invalid_request")),
    }
}

fn validate_owned_prompt_write_request(
    request: RpcRequest,
) -> Result<(String, Operation), (String, &'static str)> {
    match request {
        RpcRequest::AppendOwnedPrompt {
            api_version,
            request_id,
            owner,
            conversation_id,
            content,
            idempotency_key,
            expected_version,
        } => validate_append_owned_prompt_request(
            &api_version,
            request_id,
            owner,
            conversation_id,
            content,
            idempotency_key,
            expected_version,
        ),
        RpcRequest::SubmitOwnedPromptRunIntent {
            api_version,
            request_id,
            owner,
            conversation_id,
            content,
            idempotency_key,
            expected_version,
            profile_id,
            profile_sha256,
        } => validate_submit_owned_prompt_run_intent_request(
            &api_version,
            request_id,
            owner,
            conversation_id,
            content,
            idempotency_key,
            expected_version,
            profile_id,
            profile_sha256,
        ),
        _ => Err(("invalid".into(), "invalid_request")),
    }
}

fn validate_owned_consent_write_request(
    request: RpcRequest,
) -> Result<(String, Operation), (String, &'static str)> {
    match request {
        RpcRequest::GrantProjectExecutionConsent {
            api_version,
            request_id,
            owner,
            project_id,
            profile_id,
            profile_sha256,
            expires_at_ms,
            idempotency_key,
        } => validate_project_consent_grant_request(ProjectConsentGrantRequest {
            api_version,
            request_id,
            owner,
            project_id,
            profile_id,
            profile_sha256,
            expires_at_ms,
            idempotency_key,
        }),
        RpcRequest::RevokeProjectExecutionConsent {
            api_version,
            request_id,
            owner,
            grant_id,
            idempotency_key,
        } => validate_project_consent_revoke_request(
            &api_version,
            request_id,
            owner,
            grant_id,
            idempotency_key,
        ),
        _ => Err(("invalid".into(), "invalid_request")),
    }
}

fn validate_create_owned_request(
    api_version: &str,
    request_id: String,
    owner: ConversationOwner,
    scope: ConversationScope,
    title: String,
    idempotency_key: String,
) -> Result<(String, Operation), (String, &'static str)> {
    validate_write_header(api_version, &request_id)?;
    if title.trim().is_empty()
        || title.len() > 256
        || idempotency_key.trim().is_empty()
        || idempotency_key.len() > 256
        || !valid_conversation_owner(&owner)
        || !valid_conversation_scope(&scope)
    {
        return Err((request_id, "invalid_owned_conversation_request"));
    }
    Ok((
        request_id,
        Operation::CreateOwnedConversation {
            owner,
            scope,
            title,
            idempotency_key,
        },
    ))
}

fn validate_append_owned_prompt_request(
    api_version: &str,
    request_id: String,
    owner: ConversationOwner,
    conversation_id: String,
    content: String,
    idempotency_key: String,
    expected_version: u64,
) -> Result<(String, Operation), (String, &'static str)> {
    validate_write_header(api_version, &request_id)?;
    if conversation_id.trim().is_empty()
        || conversation_id.len() > MAX_HUB_ENTITY_ID_BYTES
        || !valid_conversation_owner(&owner)
        || content.trim().is_empty()
        || content.len() > crate::runtime_domain::MAX_PROMPT_CONTENT_BYTES
        || idempotency_key.trim().is_empty()
        || idempotency_key.len() > 256
        || expected_version > i64::MAX as u64
    {
        return Err((request_id, "invalid_owned_prompt_request"));
    }
    Ok((
        request_id,
        Operation::AppendOwnedPrompt {
            owner,
            conversation_id,
            content,
            idempotency_key,
            expected_version,
        },
    ))
}

#[allow(clippy::too_many_arguments)]
fn validate_submit_owned_prompt_run_intent_request(
    api_version: &str,
    request_id: String,
    owner: ConversationOwner,
    conversation_id: String,
    content: String,
    idempotency_key: String,
    expected_version: u64,
    profile_id: String,
    profile_sha256: [u8; 32],
) -> Result<(String, Operation), (String, &'static str)> {
    validate_write_header(api_version, &request_id)?;
    if !valid_conversation_owner(&owner)
        || !valid_entity_id(&conversation_id)
        || !valid_entity_id(&profile_id)
        || content.trim().is_empty()
        || content.len() > MAX_PROMPT_CONTENT_BYTES
        || idempotency_key.trim().is_empty()
        || idempotency_key.len() > 256
        || idempotency_key.chars().any(char::is_control)
        || expected_version > i64::MAX as u64
    {
        return Err((request_id, "invalid_owned_prompt_request"));
    }
    Ok((
        request_id,
        Operation::SubmitOwnedPromptRunIntent {
            owner,
            conversation_id,
            content,
            idempotency_key,
            expected_version,
            profile_id,
            profile_sha256,
        },
    ))
}

fn validate_import_owned_conversation_request(
    api_version: &str,
    request_id: String,
    owner: ConversationOwner,
    title: String,
    prompts: Vec<ConversationImportPrompt>,
    idempotency_key: String,
) -> Result<(String, Operation), (String, &'static str)> {
    validate_write_header(api_version, &request_id)?;
    if title.trim().is_empty()
        || title.len() > 256
        || idempotency_key.trim().is_empty()
        || idempotency_key.len() > 256
        || !valid_conversation_owner(&owner)
        || prompts.len() > MAX_CONVERSATION_IMPORT_PROMPT_COUNT
    {
        return Err((request_id, "invalid_owned_conversation_import"));
    }
    let mut total_content_bytes = 0_usize;
    for prompt in &prompts {
        if !matches!(prompt.role.as_str(), "user" | "assistant")
            || prompt.content.trim().is_empty()
            || prompt.content.len() > MAX_PROMPT_CONTENT_BYTES
        {
            return Err((request_id, "invalid_owned_conversation_import"));
        }
        total_content_bytes = total_content_bytes.saturating_add(prompt.content.len());
        if total_content_bytes > MAX_PROMPT_CONTENT_BYTES {
            return Err((request_id, "invalid_owned_conversation_import"));
        }
    }
    Ok((
        request_id,
        Operation::ImportOwnedConversation {
            owner,
            title,
            prompts,
            idempotency_key,
        },
    ))
}

struct ProjectConsentGrantRequest {
    api_version: String,
    request_id: String,
    owner: ConversationOwner,
    project_id: String,
    profile_id: String,
    profile_sha256: [u8; 32],
    expires_at_ms: u64,
    idempotency_key: String,
}

fn validate_project_consent_grant_request(
    request: ProjectConsentGrantRequest,
) -> Result<(String, Operation), (String, &'static str)> {
    validate_write_header(&request.api_version, &request.request_id)?;
    if !valid_conversation_owner(&request.owner)
        || !valid_entity_id(&request.project_id)
        || !valid_entity_id(&request.profile_id)
        || i64::try_from(request.expires_at_ms).is_err()
        || !valid_consent_idempotency_key(&request.idempotency_key)
    {
        return Err((
            request.request_id,
            "invalid_project_execution_consent_request",
        ));
    }
    Ok((
        request.request_id,
        Operation::GrantProjectExecutionConsent {
            owner: request.owner,
            project_id: request.project_id,
            profile_id: request.profile_id,
            profile_sha256: request.profile_sha256,
            expires_at_ms: request.expires_at_ms,
            idempotency_key: request.idempotency_key,
        },
    ))
}

fn validate_project_consent_revoke_request(
    api_version: &str,
    request_id: String,
    owner: ConversationOwner,
    grant_id: String,
    idempotency_key: String,
) -> Result<(String, Operation), (String, &'static str)> {
    validate_write_header(api_version, &request_id)?;
    if !valid_conversation_owner(&owner)
        || !valid_entity_id(&grant_id)
        || !valid_consent_idempotency_key(&idempotency_key)
    {
        return Err((request_id, "invalid_project_execution_consent_request"));
    }
    Ok((
        request_id,
        Operation::RevokeProjectExecutionConsent {
            owner,
            grant_id,
            idempotency_key,
        },
    ))
}

fn valid_consent_idempotency_key(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}
