use super::*;

pub(super) async fn execute_prompt_command(
    client: &RemoteClient,
    command: &RemoteCommand,
    idempotency_key: Option<&str>,
    resolved_prompt: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    match command {
        RemoteCommand::PromptsList { .. } => execute_prompt_list(client, command).await,
        RemoteCommand::PromptsAdd { .. } => {
            execute_prompt_add(client, command, idempotency_key, resolved_prompt).await
        }
        RemoteCommand::PromptsReceipt { .. } => {
            execute_prompt_receipt(client, command, idempotency_key, resolved_prompt).await
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
        None => client.read_converged_client_instance_views().await?,
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

/// Refreshes the owner-bound inventory/resource observation only for an
/// online, explicitly instance-scoped Prompt or scheduler lease claim/renewal.
/// The client-instance pair guard must pass first, so hidden or drifted
/// instances fail before the inventory source is touched. A local
/// `--instance-view` is an offline declaration and keeps its request-free
/// behavior; unfiltered operations remain compatible with their ordinary
/// owner/CAS or lease boundary.
pub(super) async fn ensure_online_inventory_resource_convergence(
    client: &RemoteClient,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    if instance_id.is_some() && instance_view.is_none() {
        client.read_converged_inventory_resource_view().await?;
    }
    Ok(())
}

async fn execute_prompt_list(
    client: &RemoteClient,
    command: &RemoteCommand,
) -> Result<Value, Box<dyn Error>> {
    let RemoteCommand::PromptsList {
        conversation_id,
        before_created_at_ms,
        before_prompt_id,
        instance_id,
        instance_view,
    } = command
    else {
        unreachable!("command was matched before dispatch")
    };
    ensure_prompt_instance_projection(
        client,
        conversation_id,
        instance_id.as_deref(),
        instance_view.as_deref(),
    )
    .await?;
    let before =
        before_created_at_ms
            .zip(before_prompt_id.clone())
            .map(|(created_at_ms, prompt_id)| crate::args::PromptPageCursor {
                created_at_ms,
                prompt_id,
            });
    Ok(client
        .list_prompts(conversation_id, before.as_ref())
        .await?)
}

async fn execute_prompt_add(
    client: &RemoteClient,
    command: &RemoteCommand,
    idempotency_key: Option<&str>,
    resolved_prompt: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    let RemoteCommand::PromptsAdd {
        conversation_id,
        expected_version,
        content,
        instance_id,
        instance_view,
    } = command
    else {
        unreachable!("command was matched before dispatch")
    };
    ensure_prompt_instance_projection(
        client,
        conversation_id,
        instance_id.as_deref(),
        instance_view.as_deref(),
    )
    .await?;
    ensure_online_inventory_resource_convergence(
        client,
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

async fn execute_prompt_receipt(
    client: &RemoteClient,
    command: &RemoteCommand,
    idempotency_key: Option<&str>,
    resolved_prompt: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    let RemoteCommand::PromptsReceipt {
        conversation_id,
        expected_version,
        content,
        instance_id,
        instance_view,
    } = command
    else {
        unreachable!("command was matched before dispatch")
    };
    ensure_prompt_instance_projection(
        client,
        conversation_id,
        instance_id.as_deref(),
        instance_view.as_deref(),
    )
    .await?;
    ensure_online_inventory_resource_convergence(
        client,
        instance_id.as_deref(),
        instance_view.as_deref(),
    )
    .await?;
    let key = required_idempotency_key(idempotency_key)?;
    let content = resolved_prompt.unwrap_or(content);
    Ok(client
        .append_prompt_receipt(conversation_id, *expected_version, content, key)
        .await?)
}
