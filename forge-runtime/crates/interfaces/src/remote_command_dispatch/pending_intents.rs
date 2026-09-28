use super::{
    Error, RemoteClient, RemoteCommand, RemoteError, Value, client_instance_session_scope,
    required_idempotency_key,
};

pub(super) async fn execute_pending_run_intent_command(
    client: &RemoteClient,
    command: &RemoteCommand,
    idempotency_key: Option<&str>,
    resolved_prompt: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    match command {
        RemoteCommand::PendingRunIntentsList { .. } => {
            execute_pending_run_intent_list(client, command).await
        }
        RemoteCommand::PendingRunIntentSubmit { .. } => {
            execute_pending_run_intent_submit(client, command, idempotency_key, resolved_prompt)
                .await
        }
        RemoteCommand::PendingRunIntentTimeline { .. } => {
            execute_pending_run_intent_timeline(client, command).await
        }
        _ => unreachable!("only pending Run-intent commands are routed here"),
    }
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
        None => client.read_converged_client_instance_views().await?,
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

/// Refreshes the owner-bound inventory/resource observation only for an
/// online, explicitly instance-scoped pending Run-intent submission. The
/// display projection guard above must pass first, so hidden instances fail
/// before the resource source is touched. A local `--instance-view` remains an
/// offline declaration and keeps its request-free behavior; unfiltered
/// submissions retain the ordinary consent/CAS boundary.
async fn ensure_pending_run_intent_inventory_resource_convergence(
    client: &RemoteClient,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    if instance_id.is_some() && instance_view.is_none() {
        client.read_converged_inventory_resource_view().await?;
    }
    Ok(())
}

async fn execute_pending_run_intent_list(
    client: &RemoteClient,
    command: &RemoteCommand,
) -> Result<Value, Box<dyn Error>> {
    let RemoteCommand::PendingRunIntentsList {
        conversation_id,
        limit,
        before_submitted_at_ms,
        before_intent_id,
        instance_id,
        instance_view,
    } = command
    else {
        unreachable!("command was matched before dispatch")
    };
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

async fn execute_pending_run_intent_submit(
    client: &RemoteClient,
    command: &RemoteCommand,
    idempotency_key: Option<&str>,
    resolved_prompt: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    let RemoteCommand::PendingRunIntentSubmit {
        conversation_id,
        expected_version,
        content,
        instance_id,
        instance_view,
    } = command
    else {
        unreachable!("command was matched before dispatch")
    };
    ensure_pending_run_intent_instance_projection(
        client,
        conversation_id,
        instance_id.as_deref(),
        instance_view.as_deref(),
    )
    .await?;
    ensure_pending_run_intent_inventory_resource_convergence(
        client,
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

async fn execute_pending_run_intent_timeline(
    client: &RemoteClient,
    command: &RemoteCommand,
) -> Result<Value, Box<dyn Error>> {
    let RemoteCommand::PendingRunIntentTimeline {
        conversation_id,
        intent_id,
        after_sequence,
        limit,
        instance_id,
        instance_view,
    } = command
    else {
        unreachable!("command was matched before dispatch")
    };
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
