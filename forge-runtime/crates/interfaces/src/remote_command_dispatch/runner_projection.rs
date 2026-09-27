use super::*;

/// Applies the selected client-instance/resource boundary before an admission
/// candidate. Online selection refreshes both owner-bound pairs and requires
/// the lease-proof target to be present in the refreshed resource image.
pub(super) async fn ensure_runner_admission_instance_projection(
    client: &RemoteClient,
    conversation_id: &str,
    target_id: &str,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
    operation: &str,
) -> Result<(), Box<dyn Error>> {
    let Some(instance_id) = instance_id else {
        if instance_view.is_some() {
            return Err(RemoteError(format!(
                "remote {operation} preview instance view requires --instance"
            ))
            .into());
        }
        return Ok(());
    };
    let (view, local_resource) =
        read_runner_admission_instance_view(client, instance_view, operation).await?;
    let scope =
        client_instance_session_scope::scope_from_view(&view, instance_id).map_err(|error| {
            RemoteError(format!(
                "remote {operation} client-instance filter is invalid: {error}"
            ))
        })?;
    if !scope.session_ids.contains(conversation_id) {
        return Err(RemoteError(format!(
            "remote {operation} client-instance filter rejected conversation {conversation_id:?} for instance {instance_id:?}; no {operation} request was sent"
        ))
        .into());
    }
    ensure_runner_admission_resource_projection(
        client,
        local_resource.as_ref(),
        conversation_id,
        instance_id,
        target_id,
        operation,
    )
    .await
}

async fn read_runner_admission_instance_view(
    client: &RemoteClient,
    instance_view: Option<&str>,
    operation: &str,
) -> Result<(Value, Option<Value>), Box<dyn Error>> {
    match instance_view {
        Some(path) => {
            let resource =
                client_instance_session_scope::read_local_resource_view(path).map_err(|error| {
                    RemoteError(format!(
                        "remote {operation} client-instance resource view is invalid: {error}"
                    ))
                })?;
            Ok((resource.clone(), Some(resource)))
        }
        None => Ok((client.read_converged_client_instance_views().await?, None)),
    }
}

async fn ensure_runner_admission_resource_projection(
    client: &RemoteClient,
    local_resource: Option<&Value>,
    conversation_id: &str,
    instance_id: &str,
    target_id: &str,
    operation: &str,
) -> Result<(), Box<dyn Error>> {
    if let Some(resource_view) = local_resource {
        return ensure_runner_admission_resource_scope(
            resource_view,
            conversation_id,
            instance_id,
            target_id,
            operation,
        );
    }
    let observation = client.read_converged_inventory_resource_view().await?;
    let resource_view = observation.get("resource_view").ok_or_else(|| {
        RemoteError(format!(
            "remote {operation} inventory/resource observation is invalid"
        ))
    })?;
    ensure_runner_admission_resource_scope(
        resource_view,
        conversation_id,
        instance_id,
        target_id,
        operation,
    )
}

fn ensure_runner_admission_resource_scope(
    resource_view: &Value,
    conversation_id: &str,
    instance_id: &str,
    target_id: &str,
    operation: &str,
) -> Result<(), Box<dyn Error>> {
    let scope = client_instance_session_scope::scope_from_view(resource_view, instance_id)
        .map_err(|error| {
            RemoteError(format!(
                "remote {operation} refreshed client-instance resource view is invalid: {error}"
            ))
        })?;
    if !scope.session_ids.contains(conversation_id) {
        return Err(RemoteError(format!(
            "remote {operation} refreshed client-instance filter rejected conversation {conversation_id:?} for instance {instance_id:?}; no {operation} request was sent"
        ))
        .into());
    }
    let target_observed = resource_view
        .get("devices")
        .and_then(Value::as_array)
        .is_some_and(|devices| {
            devices.iter().any(|device| {
                device.get("device_id").and_then(Value::as_str) == Some(target_id)
                    || device.get("runner_instance_id").and_then(Value::as_str) == Some(target_id)
            })
        });
    if target_observed {
        return Ok(());
    }
    Err(RemoteError(format!(
        "remote {operation} client-instance resource filter rejected target {target_id:?} for instance {instance_id:?}; no {operation} request was sent"
    ))
    .into())
}

/// Applies an optional caller-selected client-instance display projection
/// before an authenticated Runner metadata preview. The client-instance pair
/// and, for an online explicit instance, the owner-bound device
/// inventory/resource pair are read-only evidence: they scope the Conversation
/// and resource image that may reach the metadata preview but cannot grant
/// consent, select a target, or execute a command.
pub(super) async fn ensure_runner_metadata_instance_projection(
    client: &RemoteClient,
    request: &Value,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
    operation: &str,
) -> Result<(), Box<dyn Error>> {
    let Some(instance_id) = instance_id else {
        if instance_view.is_some() {
            return Err(RemoteError(format!(
                "remote {operation} preview instance view requires --instance"
            ))
            .into());
        }
        return Ok(());
    };
    let conversation_id = request
        .get("conversation_id")
        .and_then(Value::as_str)
        .ok_or_else(|| RemoteError(format!("remote {operation} request is invalid")))?;
    let view = match instance_view {
        Some(path) => client_instance_session_scope::read_local_view(path).map_err(|error| {
            RemoteError(format!(
                "remote {operation} client-instance view is invalid: {error}"
            ))
        })?,
        None => client.read_converged_client_instance_views().await?,
    };
    let scope =
        client_instance_session_scope::scope_from_view(&view, instance_id).map_err(|error| {
            RemoteError(format!(
                "remote {operation} client-instance filter is invalid: {error}"
            ))
        })?;
    if scope.session_ids.contains(conversation_id) {
        if instance_view.is_none() {
            refresh_runner_metadata_scope(client, conversation_id, instance_id, operation).await?;
        }
        return Ok(());
    }
    Err(RemoteError(format!(
        "remote {operation} client-instance filter rejected conversation {conversation_id:?} for instance {instance_id:?}; no {operation} request was sent"
    ))
    .into())
}

// Refresh the canonical inventory/resource pair only for an online explicit
// instance. Local --instance-view remains a request-free display declaration.
async fn refresh_runner_metadata_scope(
    client: &RemoteClient,
    conversation_id: &str,
    instance_id: &str,
    operation: &str,
) -> Result<(), Box<dyn Error>> {
    let observation = client.read_converged_inventory_resource_view().await?;
    let resource_view = observation.get("resource_view").ok_or_else(|| {
        RemoteError(format!(
            "remote {operation} inventory/resource observation is invalid"
        ))
    })?;
    let refreshed_scope =
        client_instance_session_scope::scope_from_view(resource_view, instance_id).map_err(
            |error| {
                RemoteError(format!(
                    "remote {operation} refreshed client-instance resource view is invalid: {error}"
                ))
            },
        )?;
    if !refreshed_scope.session_ids.contains(conversation_id) {
        return Err(RemoteError(format!(
            "remote {operation} refreshed client-instance filter rejected conversation {conversation_id:?} for instance {instance_id:?}; no {operation} request was sent"
        ))
        .into());
    }
    Ok(())
}
