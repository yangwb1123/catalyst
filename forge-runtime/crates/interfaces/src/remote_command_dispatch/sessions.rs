use super::{
    Error, RemoteClient, RemoteCommand, RemoteError, Value, client_instance_session_scope,
    required_idempotency_key,
};

pub(super) async fn execute_session_command(
    client: &RemoteClient,
    command: &RemoteCommand,
    idempotency_key: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    match command {
        RemoteCommand::SessionsList { .. } => execute_session_list(client, command).await,
        RemoteCommand::SessionsShow { .. } => execute_session_show(client, command).await,
        RemoteCommand::SessionsCreate { .. } => {
            execute_session_create(client, command, idempotency_key).await
        }
        RemoteCommand::SessionsImport { .. } => {
            Err(RemoteError("use the local import preview entry point".into()).into())
        }
        _ => unreachable!("only session commands are routed here"),
    }
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
        None => client.read_converged_client_instance_views().await?,
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

/// Validates the optional client-instance display boundary before an
/// owner-wide Conversation create. Creation remains a storage write; this
/// guard only prevents a missing or unknown private projection from being
/// treated as a valid instance-scoped view by the caller.
async fn ensure_session_create_instance_projection(
    client: &RemoteClient,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let Some(instance_id) = instance_id else {
        if instance_view.is_some() {
            return Err(RemoteError(
                "remote session create instance view requires --instance".into(),
            )
            .into());
        }
        return Ok(());
    };
    let view = match instance_view {
        Some(path) => client_instance_session_scope::read_local_view(path).map_err(|error| {
            RemoteError(format!(
                "remote session create client-instance view is invalid: {error}"
            ))
        })?,
        None => client.read_converged_client_instance_views().await?,
    };
    client_instance_session_scope::scope_from_view(&view, instance_id).map_err(|error| {
        RemoteError(format!(
            "remote session create client-instance filter is invalid: {error}"
        ))
    })?;
    Ok(())
}

async fn execute_session_list(
    client: &RemoteClient,
    command: &RemoteCommand,
) -> Result<Value, Box<dyn Error>> {
    let RemoteCommand::SessionsList {
        after_id,
        scope,
        instance_id,
        instance_view,
        all_pages,
    } = command
    else {
        unreachable!("command was matched before dispatch")
    };
    let instance_scope =
        read_session_list_instance_scope(client, instance_id.as_deref(), instance_view.as_deref())
            .await?;
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

async fn execute_session_show(
    client: &RemoteClient,
    command: &RemoteCommand,
) -> Result<Value, Box<dyn Error>> {
    let RemoteCommand::SessionsShow {
        conversation_id,
        instance_id,
        instance_view,
    } = command
    else {
        unreachable!("command was matched before dispatch")
    };
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

async fn execute_session_create(
    client: &RemoteClient,
    command: &RemoteCommand,
    idempotency_key: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    let RemoteCommand::SessionsCreate {
        title,
        scope,
        instance_id,
        instance_view,
    } = command
    else {
        unreachable!("command was matched before dispatch")
    };
    let key = required_idempotency_key(idempotency_key)?;
    ensure_session_create_instance_projection(
        client,
        instance_id.as_deref(),
        instance_view.as_deref(),
    )
    .await?;
    Ok(client.create_conversation(title, scope, key).await?)
}

async fn read_session_list_instance_scope(
    client: &RemoteClient,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
) -> Result<Option<client_instance_session_scope::ClientInstanceSessionScope>, Box<dyn Error>> {
    let instance_scope = match instance_id {
        Some(instance_id) => {
            let view = match instance_view {
                Some(path) => client_instance_session_scope::read_local_view(path)?,
                // Online instance filters require both owner-bound
                // observations to describe one converged snapshot.
                // A local FILE remains an explicit offline fixture;
                // server-side drift fails closed before session data
                // is read.
                None => client.read_converged_client_instance_views().await?,
            };
            Some(
                client_instance_session_scope::scope_from_view(&view, instance_id).map_err(
                    |error| {
                        RemoteError(format!(
                            "remote client-instance session filter is invalid: {error}"
                        ))
                    },
                )?,
            )
        }
        None => None,
    };
    Ok(instance_scope)
}
