use super::*;

/// Applies a caller-selected client-instance projection before the
/// read-only scheduler selection preview. The paired authenticated
/// session/resource view is only a display observation: it scopes the
/// Conversation that may reach the preview, while the preview itself still
/// evaluates the owner-visible resource image and grants no placement,
/// reservation, lease, or execution authority.
async fn ensure_scheduler_selection_instance_projection(
    client: &RemoteClient,
    request: &Value,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    ensure_scheduler_lease_instance_projection(
        client,
        request,
        instance_id,
        instance_view,
        "scheduler-selection",
    )
    .await?;

    // An online explicit instance selection is allowed to reach the
    // planning-only candidate route only after the owner-bound inventory-v2
    // and resource-view pair has also converged. A local --instance-view is
    // an explicit offline declaration and keeps its existing no-observation
    // behavior; the unfiltered command keeps its single POST behavior.
    if instance_id.is_some() && instance_view.is_none() {
        client.read_converged_inventory_resource_view().await?;
    }
    Ok(())
}

/// Applies an optional client-instance session/resource projection before an
/// effectful scheduler lease lifecycle request. The projection is only a
/// caller-visible Conversation boundary: it never grants lease, execution, or
/// dispatch authority. Missing, hidden, or drifted pair observations fail
/// closed before the lease route is contacted.
async fn ensure_scheduler_lease_instance_projection(
    client: &RemoteClient,
    request: &Value,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
    operation: &str,
) -> Result<(), Box<dyn Error>> {
    let Some(instance_id) = instance_id else {
        if instance_view.is_some() {
            return Err(RemoteError(format!(
                "remote {operation} instance view requires --instance"
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
        return Ok(());
    }
    Err(RemoteError(format!(
        "remote {operation} client-instance filter rejected conversation {conversation_id:?} for instance {instance_id:?}; no {operation} request was sent"
    ))
    .into())
}

pub(super) async fn execute_scheduler_selection_lease(
    client: &RemoteClient,
    input: &str,
    idempotency_key: &str,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    let request = super::super::scheduler_lease::read_request(input)?;
    ensure_scheduler_lease_instance_projection(
        client,
        &request,
        instance_id,
        instance_view,
        "scheduler lease",
    )
    .await?;
    ensure_online_inventory_resource_convergence(client, instance_id, instance_view).await?;
    let response = client
        .claim_scheduler_selection_lease(&request, idempotency_key)
        .await?;
    super::super::scheduler_lease::validate_response_for_request(&response, &request)?;
    Ok(response)
}

pub(super) async fn execute_scheduler_selection_lease_renewal(
    client: &RemoteClient,
    input: &str,
    idempotency_key: &str,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    let request = super::super::scheduler_lease_renew::read_request(input)?;
    ensure_scheduler_lease_instance_projection(
        client,
        &request,
        instance_id,
        instance_view,
        "scheduler lease renewal",
    )
    .await?;
    ensure_online_inventory_resource_convergence(client, instance_id, instance_view).await?;
    let response = client
        .renew_scheduler_selection_lease(&request, idempotency_key)
        .await?;
    super::super::scheduler_lease_renew::validate_response_for_request(&response, &request)?;
    Ok(response)
}

pub(super) async fn execute_scheduler_selection_lease_release(
    client: &RemoteClient,
    input: &str,
    idempotency_key: &str,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    let request = super::super::scheduler_lease_release::read_request(input)?;
    ensure_scheduler_lease_instance_projection(
        client,
        &request,
        instance_id,
        instance_view,
        "scheduler lease release",
    )
    .await?;
    let response = client
        .release_scheduler_selection_lease(&request, idempotency_key)
        .await?;
    super::super::scheduler_lease_release::validate_response_for_request(&response, &request)?;
    Ok(response)
}

pub(super) async fn execute_scheduler_selection_preview(
    client: &RemoteClient,
    input: &str,
    instance_id: Option<&str>,
    instance_view: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    let request = super::super::scheduler_selection::read_request(input)?;
    ensure_scheduler_selection_instance_projection(client, &request, instance_id, instance_view)
        .await?;
    let response = client.preview_scheduler_selection(&request).await?;
    super::super::scheduler_selection::validate_response_for_request(&response, &request)?;
    Ok(response)
}
