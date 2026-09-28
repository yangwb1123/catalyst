use super::{RemoteClient, RemoteError, TuiState, io_error};
use serde_json::Value;
use std::io::Write;

/// Refreshes the v1 inventory only after the user explicitly opened it with
/// `inventory read`. The candidate is owner-derived by the server and
/// validated by the remote client; this process-local opt-in prevents a
/// normal session sync from silently probing a disabled device route.
pub(super) async fn sync_device_inventory<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    if state.device_inventory_observed.is_none() {
        return Ok(true);
    }
    let value = match client.read_device_inventory().await {
        Ok(value) => value,
        Err(error) => {
            let cleared = super::super::clear_session_view_after_authorization_error(state, &error);
            writeln!(
                writer,
                "Sync device inventory refresh failed: {error}. The change cursor was not advanced."
            )
            .map_err(io_error)?;
            if cleared {
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(io_error)?;
            }
            return Ok(false);
        }
    };
    super::super::commands::write_remote_inventory(&value, writer)?;
    state.device_inventory_observed = Some(value);
    writeln!(writer, "Selected inventory observation refreshed.").map_err(io_error)?;
    Ok(true)
}

/// Refreshes the lossless v2 inventory only after the user explicitly opened
/// it with `inventory read-v2`. The candidate is owner-derived by the server
/// and validated by the remote client; this process-local opt-in prevents a
/// normal session sync from silently probing a disabled device route.
pub(super) async fn sync_device_inventory_v2<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    if state.device_inventory_v2_observed.is_none() {
        return Ok(true);
    }
    let value = match client.read_device_inventory_v2().await {
        Ok(value) => value,
        Err(error) => {
            let cleared = super::super::clear_session_view_after_authorization_error(state, &error);
            writeln!(
                writer,
                "Sync device inventory refresh failed: {error}. The change cursor was not advanced."
            )
            .map_err(io_error)?;
            if cleared {
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(io_error)?;
            }
            return Ok(false);
        }
    };
    super::super::commands::write_remote_inventory_v2(&value, writer)?;
    state.device_inventory_v2_observed = Some(value);
    writeln!(writer, "Selected inventory observation refreshed.").map_err(io_error)?;
    Ok(true)
}

/// Refreshes the exact pending Run-intent metadata page opened by the user.
/// This remains opt-in process-local state: a normal sync never requests the
/// private candidate unless `run-intents` first established the binding.
pub(super) async fn sync_pending_run_intent_page<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    let Some(observation) = state.pending_run_intent_page.clone() else {
        return Ok(true);
    };
    if state.selected_id.as_deref() != Some(observation.conversation_id.as_str()) {
        state.clear_pending_run_intent_page();
        return Ok(true);
    }
    if !super::super::writes::ensure_pending_run_intent_visible_to_client_instance(
        state,
        &observation.conversation_id,
        writer,
    )? {
        return Ok(true);
    }
    let page = match client
        .list_pending_run_intents(
            &observation.conversation_id,
            super::super::pending_run_intents::PAGE_LIMIT,
            observation.before_submitted_at_ms,
            observation.before_intent_id.as_deref(),
        )
        .await
    {
        Ok(page) => page,
        Err(error) => {
            report_pending_error(state, &error, writer)?;
            return Ok(false);
        }
    };
    super::super::pending_run_intents::render_page(&page, writer)?;
    state.record_pending_run_intent_page(
        &observation.conversation_id,
        observation
            .before_submitted_at_ms
            .zip(observation.before_intent_id),
    );
    writeln!(writer, "Selected pending Run-intent metadata refreshed.").map_err(io_error)?;
    Ok(true)
}

pub(super) fn report_pending_error<W: Write>(
    state: &mut TuiState,
    error: &RemoteError,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let cleared = super::super::clear_session_view_after_authorization_error(state, error);
    writeln!(
        writer,
        "Sync pending Run-intent refresh failed: {error}. The change cursor was not advanced."
    )
    .map_err(io_error)?;
    if cleared {
        writeln!(
            writer,
            "Local session view cleared after authorization failure."
        )
        .map_err(io_error)?;
    }
    Ok(())
}

pub(super) async fn sync_observations<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
    previous_resource_view: Option<Value>,
    client_instance_refreshed_before_owner_reads: bool,
) -> Result<bool, RemoteError> {
    let previous_inventory_v2 = state.device_inventory_v2_observed.clone();
    if !sync_device_inventory_v2(client, state, writer).await? {
        // An explicitly opened inventory view is part of the same owner
        // refresh boundary. Do not advance the conversation cursor when its
        // replacement could not be validated.
        return Ok(false);
    }
    let previous_client_instance_session = state.client_instance_session_view_observed.clone();
    let previous_client_instance_resource = state.client_instance_resource_view_observed.clone();
    if !sync_pending_run_intent_page(client, state, writer).await? {
        // An explicitly opened pending Run-intent page is another owner-bound
        // observation. Keep the conversation cursor unchanged when its
        // replacement cannot be validated, so the next sync retries the same
        // page rather than silently dropping the refresh boundary.
        return Ok(false);
    }
    if !client_instance_refreshed_before_owner_reads
        && !super::super::sync_client_instances::sync_client_instance_views(client, state, writer)
            .await?
    {
        // Client-instance observations are explicit owner-bound reads.
        // Keep the conversation cursor unchanged when either replacement
        // cannot be validated so the next sync retries the same boundary.
        return Ok(false);
    }
    if !reconcile_client_pair(
        state,
        previous_client_instance_session,
        previous_client_instance_resource,
        writer,
    )? {
        return Ok(false);
    }
    reconcile_inventory_pair(state, previous_inventory_v2, previous_resource_view, writer)
}

pub(super) fn reconcile_client_pair<W: Write>(
    state: &mut TuiState,
    previous_client_instance_session: Option<Value>,
    previous_client_instance_resource: Option<Value>,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    if let (Some(previous_session), Some(previous_resource)) = (
        previous_client_instance_session,
        previous_client_instance_resource,
    ) {
        let current_session = state.client_instance_session_view_observed.as_ref();
        let current_resource = state.client_instance_resource_view_observed.as_ref();
        if let (Some(current_session), Some(current_resource)) = (current_session, current_resource)
            && !super::super::client_instance_convergence::observations_converged(
                current_session,
                current_resource,
            )
        {
            state.client_instance_session_view_observed = Some(previous_session);
            state.client_instance_resource_view_observed = Some(previous_resource);
            state.mark_client_instance_observations_not_converged();
            writeln!(
                writer,
                "Client-instance session/resource observations did not converge; previous snapshots were retained and the change cursor was not advanced."
            )
            .map_err(io_error)?;
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn reconcile_inventory_pair<W: Write>(
    state: &mut TuiState,
    previous_inventory_v2: Option<Value>,
    previous_resource_view: Option<Value>,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    if previous_inventory_v2.is_some()
        && previous_resource_view.is_some()
        && !super::super::inventory_convergence::observations_converged(
            state.device_inventory_v2_observed.as_ref().unwrap(),
            state
                .client_instance_resource_view_observed
                .as_ref()
                .unwrap(),
        )
    {
        state.device_inventory_v2_observed = previous_inventory_v2;
        state.client_instance_resource_view_observed = previous_resource_view;
        writeln!(
            writer,
            "Inventory/resource observations did not converge; previous snapshots were retained and the change cursor was not advanced."
        )
        .map_err(io_error)?;
        return Ok(false);
    }
    Ok(true)
}
