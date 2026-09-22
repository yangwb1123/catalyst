use std::error::Error;

use serde_json::Value;

use crate::args::RemoteCommand;
use crate::client_instance_session_scope;

use super::{RemoteClient, RemoteError, required_idempotency_key};

pub(crate) async fn execute(
    command: &RemoteCommand,
    idempotency_key: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    execute_inner(command, idempotency_key, None).await
}

pub(crate) async fn execute_with_resolved_prompt(
    command: &RemoteCommand,
    idempotency_key: Option<&str>,
    content: &str,
) -> Result<Value, Box<dyn Error>> {
    execute_inner(command, idempotency_key, Some(content)).await
}

async fn execute_inner(
    command: &RemoteCommand,
    idempotency_key: Option<&str>,
    resolved_prompt: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    if matches!(command, RemoteCommand::Login) {
        return Err(RemoteError("use the remote login entry point".into()).into());
    }
    let client = RemoteClient::from_env().await?;
    match command {
        RemoteCommand::Login => unreachable!("remote login is handled before API client setup"),
        RemoteCommand::Tui => {
            Err(RemoteError("use the interactive remote TUI entry point".into()).into())
        }
        RemoteCommand::CredentialStorageStatus => {
            Err(RemoteError("use the local credential storage status entry point".into()).into())
        }
        RemoteCommand::InventoryShow => Ok(client.read_device_inventory().await?),
        RemoteCommand::InventoryShowV2 => Ok(client.read_device_inventory_v2().await?),
        RemoteCommand::LifecycleRegistryShow => Ok(client.read_lifecycle_registry().await?),
        RemoteCommand::ClientInstanceSessionView => {
            Ok(client.read_client_instance_session_view().await?)
        }
        RemoteCommand::ClientInstanceResourceView => {
            Ok(client.read_client_instance_resource_view().await?)
        }
        RemoteCommand::PlacementPreview { input } => {
            let request = super::placement::read_request(input)?;
            let response = client.preview_device_placement(&request).await?;
            super::placement::validate_response(&response, &request)?;
            Ok(response)
        }
        RemoteCommand::PlacementRegistryPreview { input } => {
            let request = super::placement_registry::read_request(input)?;
            let response = client.preview_device_placement_registry(&request).await?;
            super::placement_registry::validate_response(&response)?;
            Ok(response)
        }
        RemoteCommand::CredentialCandidatePreview { input } => {
            let request = crate::remote_credential_candidate::read_request(input)?;
            let response = client.preview_device_credential_candidate(&request).await?;
            crate::remote_credential_candidate::validate_response(&response, &request)?;
            Ok(response)
        }
        RemoteCommand::SessionObservationPreview { input } => {
            let request = super::session_observation::read_request(input)?;
            let request_object = request
                .as_object()
                .ok_or_else(|| RemoteError("remote session observation input is invalid".into()))?;
            let conversation_id = request_object
                .get("conversation_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    RemoteError("remote session observation conversation is invalid".into())
                })?;
            let run_id = request_object
                .get("run_id")
                .and_then(Value::as_str)
                .ok_or_else(|| RemoteError("remote session observation Run is invalid".into()))?;
            let response = client
                .preview_session_device_observation(conversation_id, run_id, &request)
                .await?;
            super::session_observation::validate_response(&response, &request)?;
            Ok(response)
        }
        RemoteCommand::SessionRunnerReceiptPreview { input } => {
            let request = super::session_runner_receipt::read_request(input)?;
            let (conversation_id, run_id) =
                super::session_runner_receipt::conversation_and_run(&request)?;
            let response = client
                .preview_session_runner_receipt_observation(conversation_id, run_id, &request)
                .await?;
            super::session_runner_receipt::validate_response(
                &response,
                &request,
                conversation_id,
                run_id,
            )?;
            Ok(response)
        }
        RemoteCommand::RunAttemptLeaseDispatchPreflightPreview { input } => {
            let request = super::run_attempt_lease_dispatch_preflight::read_request(input)?;
            let (conversation_id, run_id) =
                super::run_attempt_lease_dispatch_preflight::conversation_and_run(&request)?;
            let response = client
                .preview_run_attempt_lease_dispatch_preflight(&conversation_id, &run_id, &request)
                .await?;
            super::run_attempt_lease_dispatch_preflight::validate_response(
                &response,
                &request,
                &conversation_id,
                &run_id,
            )?;
            Ok(response)
        }
        RemoteCommand::RunnerDispatchPlanPreview { input } => {
            let request = super::runner_dispatch_plan_preview::read_request(input)?;
            let (conversation_id, run_id) =
                super::runner_dispatch_plan_preview::conversation_and_run(&request)?;
            let dispatch_plan = super::runner_dispatch_plan_preview::dispatch_plan(&request)?;
            let response = client
                .preview_runner_dispatch_plan(&conversation_id, &run_id, &dispatch_plan)
                .await?;
            super::runner_dispatch_plan_preview::validate_response(
                &response,
                &request,
                &conversation_id,
                &run_id,
            )?;
            Ok(response)
        }
        RemoteCommand::LocalRunnerPreview { input } => {
            let request = super::local_runner_preview::read_request(input)?;
            let (conversation_id, intent_id) =
                super::local_runner_preview::conversation_and_intent(&request)?;
            let response = client
                .preview_local_runner_execution_readiness(conversation_id, intent_id, &request)
                .await?;
            super::local_runner_preview::validate_response(
                &response,
                &request,
                conversation_id,
                intent_id,
            )?;
            Ok(response)
        }
        RemoteCommand::ExecutionConsentPreview { conversation_id } => {
            let response = client.preview_execution_consent(conversation_id).await?;
            super::execution_consent_preview::validate_response(&response, conversation_id)?;
            Ok(response)
        }
        RemoteCommand::ExecutionReconciliationPreview { input } => {
            let request = super::execution_reconciliation::read_request(input)?;
            let (conversation_id, run_id) =
                super::execution_reconciliation::conversation_and_run(&request)?;
            let response = client
                .preview_execution_reconciliation(&conversation_id, &run_id, &request)
                .await?;
            super::execution_reconciliation::validate_response(
                &response,
                &request,
                &conversation_id,
                &run_id,
            )?;
            Ok(response)
        }
        RemoteCommand::SessionsList { .. }
        | RemoteCommand::SessionsShow { .. }
        | RemoteCommand::SessionsCreate { .. }
        | RemoteCommand::SessionsImport { .. } => {
            execute_session_command(&client, command, idempotency_key).await
        }
        RemoteCommand::RunsList { .. }
        | RemoteCommand::RunObserved { .. }
        | RemoteCommand::RunTimeline { .. } => execute_run_command(&client, command).await,
        RemoteCommand::PendingRunIntentsList { .. }
        | RemoteCommand::PendingRunIntentSubmit { .. }
        | RemoteCommand::PendingRunIntentTimeline { .. } => {
            execute_pending_run_intent_command(&client, command, idempotency_key, resolved_prompt)
                .await
        }
        RemoteCommand::PromptsList { .. } | RemoteCommand::PromptsAdd { .. } => {
            execute_prompt_command(&client, command, idempotency_key, resolved_prompt).await
        }
        RemoteCommand::ChangesList { after_cursor } => Ok(serde_json::to_value(
            client.resumed_conversation_changes(*after_cursor).await?,
        )?),
        RemoteCommand::ChangesWatch {
            after_cursor,
            polls,
            min_delay_ms,
            max_delay_ms,
        } => Ok(client
            .watch_conversation_changes(*after_cursor, *polls, *min_delay_ms, *max_delay_ms)
            .await?),
    }
}

async fn execute_session_command(
    client: &RemoteClient,
    command: &RemoteCommand,
    idempotency_key: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    match command {
        RemoteCommand::SessionsList {
            after_id,
            scope,
            instance_id,
            instance_view,
            all_pages,
        } => {
            let instance_scope = match instance_id.as_deref() {
                Some(instance_id) => {
                    let view = match instance_view.as_deref() {
                        Some(path) => client_instance_session_scope::read_local_view(path)?,
                        None => client.read_client_instance_session_view().await?,
                    };
                    Some(
                        client_instance_session_scope::scope_from_view(&view, instance_id)
                            .map_err(|error| {
                                RemoteError(format!(
                                    "remote client-instance session filter is invalid: {error}"
                                ))
                            })?,
                    )
                }
                None => None,
            };
            let value = if let Some(instance_scope) = instance_scope.as_ref() {
                client
                    .list_conversations_json_with_instance(
                        after_id.as_deref(),
                        scope.as_ref(),
                        Some(instance_scope),
                        *all_pages,
                    )
                    .await?
            } else {
                client
                    .list_conversations_json(after_id.as_deref(), scope.as_ref(), *all_pages)
                    .await?
            };
            Ok(value)
        }
        RemoteCommand::SessionsShow {
            conversation_id,
            instance_id,
            instance_view,
        } => {
            ensure_conversation_instance_projection(
                client,
                conversation_id,
                instance_id.as_deref(),
                instance_view.as_deref(),
            )
            .await?;
            Ok(serde_json::to_value(
                client.get_conversation(conversation_id).await?,
            )?)
        }
        RemoteCommand::SessionsCreate { title, scope } => {
            let key = required_idempotency_key(idempotency_key)?;
            Ok(client.create_conversation(title, scope, key).await?)
        }
        RemoteCommand::SessionsImport { .. } => {
            Err(RemoteError("use the local import preview entry point".into()).into())
        }
        _ => unreachable!("only session commands are routed here"),
    }
}

async fn execute_run_command(
    client: &RemoteClient,
    command: &RemoteCommand,
) -> Result<Value, Box<dyn Error>> {
    match command {
        RemoteCommand::RunsList {
            conversation_id,
            limit,
            before_created_at_ms,
            before_run_id,
            instance_id,
            instance_view,
        } => {
            ensure_run_instance_projection(
                client,
                conversation_id,
                instance_id.as_deref(),
                instance_view.as_deref(),
            )
            .await?;
            Ok(client
                .list_runs(
                    conversation_id,
                    *limit,
                    *before_created_at_ms,
                    before_run_id.as_deref(),
                )
                .await?)
        }
        RemoteCommand::RunObserved {
            conversation_id,
            run_id,
            instance_id,
            instance_view,
        } => {
            ensure_run_instance_projection(
                client,
                conversation_id,
                instance_id.as_deref(),
                instance_view.as_deref(),
            )
            .await?;
            Ok(serde_json::to_value(
                client.read_run_observation(conversation_id, run_id).await?,
            )?)
        }
        RemoteCommand::RunTimeline {
            conversation_id,
            run_id,
            after_sequence,
            limit,
            resume,
            instance_id,
            instance_view,
        } => {
            ensure_run_instance_projection(
                client,
                conversation_id,
                instance_id.as_deref(),
                instance_view.as_deref(),
            )
            .await?;
            if *resume {
                Ok(client
                    .resumed_run_timeline(conversation_id, run_id, *limit)
                    .await?)
            } else {
                Ok(client
                    .run_timeline(conversation_id, run_id, *after_sequence, *limit)
                    .await?)
            }
        }
        _ => unreachable!("only Run commands are routed here"),
    }
}

/// Applies the caller-selected client-instance projection before a Run read.
/// The declaration is local display data and never changes the authenticated
/// owner request or grants instance authority.
async fn ensure_run_instance_projection(
    client: &RemoteClient,
    conversation_id: &str,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let Some(instance_id) = instance_id else {
        if instance_view.is_some() {
            return Err(RemoteError("remote Run instance view requires --instance".into()).into());
        }
        return Ok(());
    };
    let view = match instance_view {
        Some(path) => client_instance_session_scope::read_local_view(path).map_err(|error| {
            RemoteError(format!(
                "remote Run client-instance view is invalid: {error}"
            ))
        })?,
        None => client.read_client_instance_session_view().await?,
    };
    let scope =
        client_instance_session_scope::scope_from_view(&view, instance_id).map_err(|error| {
            RemoteError(format!(
                "remote Run client-instance filter is invalid: {error}"
            ))
        })?;
    if scope.session_ids.contains(conversation_id) {
        return Ok(());
    }
    Err(RemoteError(format!(
        "remote Run client-instance filter rejected conversation {conversation_id:?} for instance {instance_id:?}; no Run request was sent"
    ))
    .into())
}

async fn execute_pending_run_intent_command(
    client: &RemoteClient,
    command: &RemoteCommand,
    idempotency_key: Option<&str>,
    resolved_prompt: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    match command {
        RemoteCommand::PendingRunIntentsList {
            conversation_id,
            limit,
            before_submitted_at_ms,
            before_intent_id,
            instance_id,
            instance_view,
        } => {
            ensure_pending_run_intent_instance_projection(
                client,
                conversation_id,
                instance_id.as_deref(),
                instance_view.as_deref(),
            )
            .await?;
            Ok(client
                .list_pending_run_intents(
                    conversation_id,
                    *limit,
                    *before_submitted_at_ms,
                    before_intent_id.as_deref(),
                )
                .await?)
        }
        RemoteCommand::PendingRunIntentSubmit {
            conversation_id,
            expected_version,
            content,
            instance_id,
            instance_view,
        } => {
            ensure_pending_run_intent_instance_projection(
                client,
                conversation_id,
                instance_id.as_deref(),
                instance_view.as_deref(),
            )
            .await?;
            let key = required_idempotency_key(idempotency_key)?;
            let content = resolved_prompt.unwrap_or(content);
            Ok(client
                .submit_pending_run_intent(conversation_id, *expected_version, content, key)
                .await?)
        }
        RemoteCommand::PendingRunIntentTimeline {
            conversation_id,
            intent_id,
            after_sequence,
            limit,
            instance_id,
            instance_view,
        } => {
            ensure_pending_run_intent_instance_projection(
                client,
                conversation_id,
                instance_id.as_deref(),
                instance_view.as_deref(),
            )
            .await?;
            Ok(client
                .pending_run_intent_timeline(conversation_id, intent_id, *after_sequence, *limit)
                .await?)
        }
        _ => unreachable!("only pending Run-intent commands are routed here"),
    }
}

async fn execute_prompt_command(
    client: &RemoteClient,
    command: &RemoteCommand,
    idempotency_key: Option<&str>,
    resolved_prompt: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    match command {
        RemoteCommand::PromptsList {
            conversation_id,
            before_created_at_ms,
            before_prompt_id,
            instance_id,
            instance_view,
        } => {
            ensure_prompt_instance_projection(
                client,
                conversation_id,
                instance_id.as_deref(),
                instance_view.as_deref(),
            )
            .await?;
            let before = before_created_at_ms.zip(before_prompt_id.clone()).map(
                |(created_at_ms, prompt_id)| crate::args::PromptPageCursor {
                    created_at_ms,
                    prompt_id,
                },
            );
            Ok(client
                .list_prompts(conversation_id, before.as_ref())
                .await?)
        }
        RemoteCommand::PromptsAdd {
            conversation_id,
            expected_version,
            content,
            instance_id,
            instance_view,
        } => {
            ensure_prompt_instance_projection(
                client,
                conversation_id,
                instance_id.as_deref(),
                instance_view.as_deref(),
            )
            .await?;
            let key = required_idempotency_key(idempotency_key)?;
            let content = resolved_prompt.unwrap_or(content);
            Ok(client
                .append_prompt(conversation_id, *expected_version, content, key)
                .await?)
        }
        _ => unreachable!("only Prompt commands are routed here"),
    }
}

/// Applies the caller-selected client-instance projection before a Prompt
/// request.  The view remains an unverified display declaration: it only
/// prevents an explicit CLI command from accidentally reading or writing a
/// Conversation outside that declaration and never changes the authenticated
/// owner request or grants instance authority.
async fn ensure_prompt_instance_projection(
    client: &RemoteClient,
    conversation_id: &str,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let Some(instance_id) = instance_id else {
        if instance_view.is_some() {
            return Err(
                RemoteError("remote Prompt instance view requires --instance".into()).into(),
            );
        }
        return Ok(());
    };
    let view = match instance_view {
        Some(path) => client_instance_session_scope::read_local_view(path).map_err(|error| {
            RemoteError(format!(
                "remote Prompt client-instance view is invalid: {error}"
            ))
        })?,
        None => client.read_client_instance_session_view().await?,
    };
    let scope =
        client_instance_session_scope::scope_from_view(&view, instance_id).map_err(|error| {
            RemoteError(format!(
                "remote Prompt client-instance filter is invalid: {error}"
            ))
        })?;
    if scope.session_ids.contains(conversation_id) {
        return Ok(());
    }
    Err(RemoteError(format!(
        "remote Prompt client-instance filter rejected conversation {conversation_id:?} for instance {instance_id:?}; no Prompt request was sent"
    ))
    .into())
}

/// Applies the caller-selected client-instance projection before a pending
/// Run-intent read or submit. The view is an unverified display declaration;
/// it only prevents a command from crossing the local session boundary and
/// never changes the authenticated owner request or grants execution
/// authority.
async fn ensure_pending_run_intent_instance_projection(
    client: &RemoteClient,
    conversation_id: &str,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let Some(instance_id) = instance_id else {
        if instance_view.is_some() {
            return Err(
                RemoteError("remote Run-intent instance view requires --instance".into()).into(),
            );
        }
        return Ok(());
    };
    let view = match instance_view {
        Some(path) => client_instance_session_scope::read_local_view(path).map_err(|error| {
            RemoteError(format!(
                "remote Run-intent client-instance view is invalid: {error}"
            ))
        })?,
        None => client.read_client_instance_session_view().await?,
    };
    let scope =
        client_instance_session_scope::scope_from_view(&view, instance_id).map_err(|error| {
            RemoteError(format!(
                "remote Run-intent client-instance filter is invalid: {error}"
            ))
        })?;
    if scope.session_ids.contains(conversation_id) {
        return Ok(());
    }
    Err(RemoteError(format!(
        "remote Run-intent client-instance filter rejected conversation {conversation_id:?} for instance {instance_id:?}; no pending Run-intent request was sent"
    ))
    .into())
}

/// Applies the caller-selected client-instance projection before a
/// Conversation detail request. The declaration is local display data and
/// never changes the authenticated owner request or grants instance authority.
async fn ensure_conversation_instance_projection(
    client: &RemoteClient,
    conversation_id: &str,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let Some(instance_id) = instance_id else {
        if instance_view.is_some() {
            return Err(
                RemoteError("remote session instance view requires --instance".into()).into(),
            );
        }
        return Ok(());
    };
    let view = match instance_view {
        Some(path) => client_instance_session_scope::read_local_view(path).map_err(|error| {
            RemoteError(format!(
                "remote session client-instance view is invalid: {error}"
            ))
        })?,
        None => client.read_client_instance_session_view().await?,
    };
    let scope =
        client_instance_session_scope::scope_from_view(&view, instance_id).map_err(|error| {
            RemoteError(format!(
                "remote session client-instance filter is invalid: {error}"
            ))
        })?;
    if scope.session_ids.contains(conversation_id) {
        return Ok(());
    }
    Err(RemoteError(format!(
        "remote session client-instance filter rejected conversation {conversation_id:?} for instance {instance_id:?}; no Conversation request was sent"
    ))
    .into())
}

#[cfg(test)]
#[path = "remote_command_dispatch_tests.rs"]
mod tests;
