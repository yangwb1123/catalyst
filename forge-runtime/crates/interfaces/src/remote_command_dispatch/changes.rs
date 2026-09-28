use super::{
    Error, RemoteClient, RemoteCommand, RemoteError, Value, client_instance_session_scope,
};

/// Reads an owner-wide change feed and applies an optional caller-declared
/// client-instance projection to its rows.  The feed request and cursor
/// checkpoint remain owner-scoped: rows hidden by the projection still count
/// toward `scanned_through_cursor`, so a later instance filter cannot replay
/// or stall the owner's change cursor.
pub(super) async fn execute_changes_command(
    client: &RemoteClient,
    command: &RemoteCommand,
) -> Result<Value, Box<dyn Error>> {
    let (instance_id, instance_view) = change_instance_projection(command)?;
    let instance_scope = read_change_instance_scope(client, instance_id, instance_view).await?;
    let response = match command {
        RemoteCommand::ChangesList { after_cursor, .. } => {
            serde_json::to_value(client.resumed_conversation_changes(*after_cursor).await?)?
        }
        RemoteCommand::ChangesWatch {
            after_cursor,
            polls,
            min_delay_ms,
            max_delay_ms,
            ..
        } => {
            client
                .watch_conversation_changes(*after_cursor, *polls, *min_delay_ms, *max_delay_ms)
                .await?
        }
        RemoteCommand::ChangesStream {
            after_cursor,
            wait_ms,
            ..
        } => {
            client
                .stream_conversation_changes(*after_cursor, *wait_ms)
                .await?
        }
        _ => unreachable!("changes command was validated above"),
    };
    project_change_feed_response(response, instance_scope.as_ref())
        .map_err(|error| Box::new(error) as Box<dyn Error>)
}

async fn read_change_instance_scope(
    client: &RemoteClient,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
) -> Result<Option<client_instance_session_scope::ClientInstanceSessionScope>, Box<dyn Error>> {
    let Some(instance_id) = instance_id else {
        return Ok(None);
    };
    let view = match instance_view {
        Some(path) => client_instance_session_scope::read_local_view(path)?,
        None => client.read_converged_client_instance_views().await?,
    };
    let scope =
        client_instance_session_scope::scope_from_view(&view, instance_id).map_err(|error| {
            RemoteError(format!(
                "remote client-instance change filter is invalid: {error}"
            ))
        })?;
    Ok(Some(scope))
}

pub(super) fn project_change_feed_response(
    mut response: Value,
    instance_scope: Option<&client_instance_session_scope::ClientInstanceSessionScope>,
) -> Result<Value, RemoteError> {
    let Some(instance_scope) = instance_scope else {
        return Ok(response);
    };
    let changes = response
        .get_mut("changes")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| RemoteError("Forge API returned an invalid change feed".into()))?;
    changes.retain(|change| {
        change
            .get("conversation_id")
            .and_then(Value::as_str)
            .is_some_and(|conversation_id| instance_scope.session_ids.contains(conversation_id))
    });
    Ok(response)
}

type InstanceProjection<'a> = (Option<&'a str>, Option<&'a str>);

fn change_instance_projection(
    command: &RemoteCommand,
) -> Result<InstanceProjection<'_>, Box<dyn Error>> {
    match command {
        RemoteCommand::ChangesList {
            instance_id,
            instance_view,
            ..
        }
        | RemoteCommand::ChangesWatch {
            instance_id,
            instance_view,
            ..
        }
        | RemoteCommand::ChangesStream {
            instance_id,
            instance_view,
            ..
        } => Ok((instance_id.as_deref(), instance_view.as_deref())),
        _ => Err(RemoteError("invalid changes command".into()).into()),
    }
}
