use super::*;

pub(super) async fn execute_run_command(
    client: &RemoteClient,
    command: &RemoteCommand,
) -> Result<Value, Box<dyn Error>> {
    match command {
        RemoteCommand::RunsList { .. } => execute_run_list(client, command).await,
        RemoteCommand::RunObserved { .. } => execute_run_observed(client, command).await,
        RemoteCommand::RunTimeline { .. } => execute_run_timeline(client, command).await,
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
        None => client.read_converged_client_instance_views().await?,
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

async fn execute_run_list(
    client: &RemoteClient,
    command: &RemoteCommand,
) -> Result<Value, Box<dyn Error>> {
    let RemoteCommand::RunsList {
        conversation_id,
        limit,
        before_created_at_ms,
        before_run_id,
        instance_id,
        instance_view,
    } = command
    else {
        unreachable!("command was matched before dispatch")
    };
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

async fn execute_run_observed(
    client: &RemoteClient,
    command: &RemoteCommand,
) -> Result<Value, Box<dyn Error>> {
    let RemoteCommand::RunObserved {
        conversation_id,
        run_id,
        instance_id,
        instance_view,
    } = command
    else {
        unreachable!("command was matched before dispatch")
    };
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

async fn execute_run_timeline(
    client: &RemoteClient,
    command: &RemoteCommand,
) -> Result<Value, Box<dyn Error>> {
    let RemoteCommand::RunTimeline {
        conversation_id,
        run_id,
        after_sequence,
        limit,
        resume,
        instance_id,
        instance_view,
    } = command
    else {
        unreachable!("command was matched before dispatch")
    };
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
