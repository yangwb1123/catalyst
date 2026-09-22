use forge_runtime_application::HubService;
use serde::Serialize;

use crate::runtime_domain::{
    ConversationOwner, ConversationPromptCursor, OwnedPromptAppendResult, OwnedRunCursor,
    SubmitPendingRunIntent,
};

use super::{
    HubError,
    conversation_transport::{ConversationChangesJsonSafe, ConversationTimestampsJsonSafe},
    pending_intent_transport::PendingRunIntentJsonSafe,
    validation::Operation,
    wire::{error_response, owned_error_response, success_response},
};

pub(super) fn execute_owned_operation(
    service: &HubService,
    request_id: &str,
    operation: Operation,
) -> Vec<u8> {
    if operation.requires_write() {
        execute_owned_write_operation(service, request_id, operation)
    } else {
        execute_owned_read_operation(service, request_id, operation)
    }
}

fn execute_owned_read_operation(
    service: &HubService,
    request_id: &str,
    operation: Operation,
) -> Vec<u8> {
    match operation {
        conversation_operation @ (Operation::ListOwnedConversations { .. }
        | Operation::GetOwnedConversation { .. }
        | Operation::OwnedPromptPage { .. }
        | Operation::OwnedProjectConversationIdentity { .. }
        | Operation::OwnedChangesAfter { .. }) => {
            execute_owned_conversation_read_operation(service, request_id, conversation_operation)
        }
        run_operation @ (Operation::OwnedRunPage { .. }
        | Operation::OwnedRunObservation { .. }
        | Operation::OwnedRunTimelinePage { .. }) => {
            execute_owned_run_operation(service, request_id, run_operation)
        }
        intent_operation @ (Operation::OwnedPendingRunIntentPage { .. }
        | Operation::OwnedPendingRunIntentTimelinePage { .. }) => {
            execute_owned_intent_read_operation(service, request_id, intent_operation)
        }
        _ => error_response(request_id, "invalid_owned_request"),
    }
}

fn execute_owned_conversation_read_operation(
    service: &HubService,
    request_id: &str,
    operation: Operation,
) -> Vec<u8> {
    match operation {
        Operation::ListOwnedConversations {
            owner,
            after_id,
            limit,
        } => {
            execute_owned_conversation_list(service, request_id, &owner, after_id.as_deref(), limit)
        }
        Operation::GetOwnedConversation {
            owner,
            conversation_id,
        } => owned_conversation_response(
            request_id,
            service.get_owned_conversation(&owner, &conversation_id),
        ),
        Operation::OwnedPromptPage {
            owner,
            conversation_id,
            before,
            limit,
        } => execute_owned_prompt_page(
            service,
            request_id,
            &owner,
            &conversation_id,
            before.as_ref(),
            limit,
        ),
        Operation::OwnedProjectConversationIdentity {
            owner,
            conversation_id,
        } => owned_operation_response(
            request_id,
            service.owned_project_conversation_identity(&owner, &conversation_id),
        ),
        Operation::OwnedChangesAfter {
            owner,
            after_cursor,
            limit,
        } => execute_owned_changes_after(service, request_id, &owner, after_cursor, limit),
        _ => error_response(request_id, "invalid_owned_request"),
    }
}

fn execute_owned_intent_read_operation(
    service: &HubService,
    request_id: &str,
    operation: Operation,
) -> Vec<u8> {
    match operation {
        Operation::OwnedPendingRunIntentPage {
            owner,
            conversation_id,
            before,
            limit,
        } => owned_pending_intent_response(
            request_id,
            service.owned_pending_run_intent_page(&owner, &conversation_id, before.as_ref(), limit),
        ),
        Operation::OwnedPendingRunIntentTimelinePage {
            owner,
            conversation_id,
            intent_id,
            after_sequence,
            limit,
        } => owned_pending_intent_response(
            request_id,
            service.owned_pending_run_intent_timeline_page(
                &owner,
                &conversation_id,
                &intent_id,
                after_sequence,
                limit,
            ),
        ),
        _ => error_response(request_id, "invalid_owned_request"),
    }
}

fn execute_owned_conversation_list(
    service: &HubService,
    request_id: &str,
    owner: &ConversationOwner,
    after_id: Option<&str>,
    limit: usize,
) -> Vec<u8> {
    owned_conversation_response(
        request_id,
        service.list_owned_conversations(owner, after_id, limit),
    )
}

fn execute_owned_changes_after(
    service: &HubService,
    request_id: &str,
    owner: &ConversationOwner,
    after_cursor: u64,
    limit: usize,
) -> Vec<u8> {
    owned_change_response(
        request_id,
        service.owned_conversation_changes_after(owner, after_cursor, limit),
    )
}

fn execute_owned_run_page(
    service: &HubService,
    request_id: &str,
    owner: &ConversationOwner,
    conversation_id: &str,
    before: Option<&OwnedRunCursor>,
    limit: usize,
) -> Vec<u8> {
    owned_operation_response(
        request_id,
        service.owned_run_page(owner, conversation_id, before, limit),
    )
}

fn execute_owned_run_operation(
    service: &HubService,
    request_id: &str,
    operation: Operation,
) -> Vec<u8> {
    match operation {
        Operation::OwnedRunPage {
            owner,
            conversation_id,
            before,
            limit,
        } => execute_owned_run_page(
            service,
            request_id,
            &owner,
            &conversation_id,
            before.as_ref(),
            limit,
        ),
        Operation::OwnedRunObservation {
            owner,
            conversation_id,
            run_id,
        } => execute_owned_run_observation(service, request_id, &owner, &conversation_id, &run_id),
        Operation::OwnedRunTimelinePage {
            owner,
            conversation_id,
            run_id,
            after_sequence,
            limit,
        } => execute_owned_run_timeline_page(
            service,
            request_id,
            &owner,
            &conversation_id,
            &run_id,
            after_sequence,
            limit,
        ),
        _ => error_response(request_id, "invalid_owned_request"),
    }
}

fn execute_owned_prompt_page(
    service: &HubService,
    request_id: &str,
    owner: &ConversationOwner,
    conversation_id: &str,
    before: Option<&ConversationPromptCursor>,
    limit: usize,
) -> Vec<u8> {
    owned_operation_response(
        request_id,
        service.owned_conversation_prompt_page(owner, conversation_id, before, limit),
    )
}

fn execute_owned_run_timeline_page(
    service: &HubService,
    request_id: &str,
    owner: &ConversationOwner,
    conversation_id: &str,
    run_id: &str,
    after_sequence: u64,
    limit: usize,
) -> Vec<u8> {
    owned_operation_response(
        request_id,
        service.owned_run_timeline_page(owner, conversation_id, run_id, after_sequence, limit),
    )
}

fn execute_owned_run_observation(
    service: &HubService,
    request_id: &str,
    owner: &ConversationOwner,
    conversation_id: &str,
    run_id: &str,
) -> Vec<u8> {
    owned_operation_response(
        request_id,
        service.owned_run_observation(owner, conversation_id, run_id),
    )
}

fn execute_owned_write_operation(
    service: &HubService,
    request_id: &str,
    operation: Operation,
) -> Vec<u8> {
    match operation {
        operation @ Operation::CreateOwnedConversation { .. } => {
            execute_create_owned_conversation(service, request_id, operation)
        }
        operation @ Operation::AppendOwnedPrompt { .. } => {
            execute_append_owned_prompt(service, request_id, operation)
        }
        operation @ Operation::SubmitOwnedPromptRunIntent { .. } => {
            execute_submit_owned_prompt_run_intent(service, request_id, operation)
        }
        operation @ Operation::ImportOwnedConversation { .. } => {
            execute_import_owned_conversation(service, request_id, operation)
        }
        consent_operation @ (Operation::GrantProjectExecutionConsent { .. }
        | Operation::RevokeProjectExecutionConsent { .. }) => {
            execute_owned_consent_write_operation(service, request_id, consent_operation)
        }
        _ => error_response(request_id, "invalid_owned_request"),
    }
}

fn execute_create_owned_conversation(
    service: &HubService,
    request_id: &str,
    operation: Operation,
) -> Vec<u8> {
    match operation {
        Operation::CreateOwnedConversation {
            owner,
            scope,
            title,
            idempotency_key,
        } => owned_conversation_response(
            request_id,
            service.create_owned_conversation(&owner, &scope, &title, &idempotency_key),
        ),
        _ => error_response(request_id, "invalid_owned_request"),
    }
}

fn execute_append_owned_prompt(
    service: &HubService,
    request_id: &str,
    operation: Operation,
) -> Vec<u8> {
    match operation {
        Operation::AppendOwnedPrompt {
            owner,
            conversation_id,
            content,
            idempotency_key,
            expected_version,
        } => owned_prompt_append_response(
            request_id,
            service.append_owned_prompt(
                &owner,
                &conversation_id,
                &content,
                &idempotency_key,
                expected_version,
            ),
        ),
        _ => error_response(request_id, "invalid_owned_request"),
    }
}

fn execute_submit_owned_prompt_run_intent(
    service: &HubService,
    request_id: &str,
    operation: Operation,
) -> Vec<u8> {
    match operation {
        Operation::SubmitOwnedPromptRunIntent {
            owner,
            conversation_id,
            content,
            idempotency_key,
            expected_version,
            profile_id,
            profile_sha256,
        } => owned_pending_intent_response(
            request_id,
            service.submit_owned_prompt_run_intent(
                &owner,
                SubmitPendingRunIntent {
                    conversation_id: &conversation_id,
                    content: &content,
                    idempotency_key: &idempotency_key,
                    expected_version,
                    profile_id: &profile_id,
                    profile_sha256: &profile_sha256,
                },
            ),
        ),
        _ => error_response(request_id, "invalid_owned_request"),
    }
}

fn execute_import_owned_conversation(
    service: &HubService,
    request_id: &str,
    operation: Operation,
) -> Vec<u8> {
    match operation {
        Operation::ImportOwnedConversation {
            owner,
            title,
            prompts,
            idempotency_key,
        } => owned_conversation_response(
            request_id,
            service.import_owned_conversation(&owner, &title, &prompts, &idempotency_key),
        ),
        _ => error_response(request_id, "invalid_owned_request"),
    }
}

fn execute_owned_consent_write_operation(
    service: &HubService,
    request_id: &str,
    operation: Operation,
) -> Vec<u8> {
    match operation {
        Operation::GrantProjectExecutionConsent {
            owner,
            project_id,
            profile_id,
            profile_sha256,
            expires_at_ms,
            idempotency_key,
        } => owned_operation_response(
            request_id,
            service.grant_project_execution_consent(
                &owner,
                &project_id,
                &profile_id,
                &profile_sha256,
                expires_at_ms,
                &idempotency_key,
            ),
        ),
        Operation::RevokeProjectExecutionConsent {
            owner,
            grant_id,
            idempotency_key,
        } => owned_operation_response(
            request_id,
            service.revoke_project_execution_consent(&owner, &grant_id, &idempotency_key),
        ),
        _ => error_response(request_id, "invalid_owned_request"),
    }
}

fn owned_operation_response<T: Serialize>(
    request_id: &str,
    result: Result<T, HubError>,
) -> Vec<u8> {
    result.map_or_else(
        |error| owned_error_response(request_id, &error),
        |value| success_response(request_id, value),
    )
}

fn owned_prompt_append_response(
    request_id: &str,
    result: Result<OwnedPromptAppendResult, HubError>,
) -> Vec<u8> {
    result.map_or_else(
        |error| owned_error_response(request_id, &error),
        |value| {
            if value.prompt.created_at_ms <= 9_007_199_254_740_991
                && value.aggregate_version > 0
                && value.aggregate_version <= 9_007_199_254_740_991
            {
                success_response(request_id, value)
            } else {
                error_response(request_id, "storage_corrupt")
            }
        },
    )
}

fn owned_pending_intent_response<T: Serialize + PendingRunIntentJsonSafe>(
    request_id: &str,
    result: Result<T, HubError>,
) -> Vec<u8> {
    result.map_or_else(
        |error| owned_error_response(request_id, &error),
        |value| {
            if value.pending_run_intent_json_safe() {
                success_response(request_id, value)
            } else {
                error_response(request_id, "storage_corrupt")
            }
        },
    )
}

fn owned_conversation_response<T>(request_id: &str, result: Result<T, HubError>) -> Vec<u8>
where
    T: Serialize + ConversationTimestampsJsonSafe,
{
    result.map_or_else(
        |error| owned_error_response(request_id, &error),
        |value| {
            if value.conversation_timestamps_json_safe() {
                success_response(request_id, value)
            } else {
                error_response(request_id, "storage_corrupt")
            }
        },
    )
}

fn owned_change_response<T: Serialize + ConversationChangesJsonSafe>(
    request_id: &str,
    result: Result<T, HubError>,
) -> Vec<u8> {
    result.map_or_else(
        |error| owned_error_response(request_id, &error),
        |value| {
            if value.conversation_changes_json_safe() {
                success_response(request_id, value)
            } else {
                error_response(request_id, "storage_corrupt")
            }
        },
    )
}
