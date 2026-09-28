use super::{Error, RemoteClient, RemoteError, Value, client_instance_session_scope};

/// Applies a caller-selected client-instance display projection before the
/// read-only execution-consent preview. The projection never changes the
/// owner-bound request and does not establish execution authority.
async fn ensure_execution_consent_instance_projection(
    client: &RemoteClient,
    conversation_id: &str,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let Some(instance_id) = instance_id else {
        if instance_view.is_some() {
            return Err(RemoteError(
                "remote execution-consent instance view requires --instance".into(),
            )
            .into());
        }
        return Ok(());
    };
    let view = match instance_view {
        Some(path) => client_instance_session_scope::read_local_view(path).map_err(|error| {
            RemoteError(format!(
                "remote execution-consent client-instance view is invalid: {error}"
            ))
        })?,
        None => client.read_converged_client_instance_views().await?,
    };
    let scope =
        client_instance_session_scope::scope_from_view(&view, instance_id).map_err(|error| {
            RemoteError(format!(
                "remote execution-consent client-instance filter is invalid: {error}"
            ))
        })?;
    if scope.session_ids.contains(conversation_id) {
        return Ok(());
    }
    Err(RemoteError(format!(
        "remote execution-consent client-instance filter rejected conversation {conversation_id:?} for instance {instance_id:?}; no execution-consent request was sent"
    ))
    .into())
}

pub(super) async fn execute_execution_consent_preview(
    client: &RemoteClient,
    conversation_id: &str,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    ensure_execution_consent_instance_projection(
        client,
        conversation_id,
        instance_id,
        instance_view,
    )
    .await?;
    let response = client.preview_execution_consent(conversation_id).await?;
    super::super::execution_consent_preview::validate_response(&response, conversation_id)?;
    Ok(response)
}
