use std::io::Write;

use super::super::{RemoteClient, RemoteError};
use super::state::{TuiState, io_error};

/// Refreshes only the client-instance candidates explicitly opened in this
/// TUI process. Ordinary session sync never probes either private route.
pub(super) async fn sync_client_instance_views<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    let read_session = state.client_instance_session_view_observed.is_some();
    let read_resource = state.client_instance_resource_view_observed.is_some();
    let previous_session = state.client_instance_session_view_observed.clone();
    let previous_resource = state.client_instance_resource_view_observed.clone();

    // Read both sides into local values first. A paired refresh must never
    // publish a newer session declaration beside an older resource image when
    // the second request fails or the two observations drift.
    let (next_session, next_resource) = match read_pair(client, read_session, read_resource).await {
        Ok(pair) => pair,
        Err((kind, error)) => {
            return report_refresh_failure(
                state,
                kind,
                &error,
                writer,
                previous_session,
                previous_resource,
            );
        }
    };

    if let (Some(session), Some(resource)) = (&next_session, &next_resource)
        && !super::client_instance_convergence::observations_converged(session, resource)
    {
        state.client_instance_session_view_observed = previous_session;
        state.client_instance_resource_view_observed = previous_resource;
        state.mark_client_instance_observations_not_converged();
        writeln!(
            writer,
            "Client-instance session/resource observations did not converge; previous snapshots were retained and the change cursor was not advanced."
        )
        .map_err(io_error)?;
        writeln!(
            writer,
            "Previous client-instance observations were retained for display only; no mixed pair was committed, and filtering and private reads through this client-instance projection remain blocked until both snapshots converge."
        )
        .map_err(io_error)?;
        return Ok(false);
    }

    commit_pair(state, next_session, next_resource, writer)
}

fn commit_pair<W: Write>(
    state: &mut TuiState,
    next_session: Option<serde_json::Value>,
    next_resource: Option<serde_json::Value>,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    // Render before committing so an unexpected renderer failure also leaves
    // the last validated process-local pair untouched.
    if let Some(response) = next_session.as_ref() {
        crate::device_client_session_view_command::write_remote_output(response, writer)
            .map_err(io_error)?;
    }
    if let Some(response) = next_resource.as_ref() {
        crate::device_client_instance_resource_view_command::write_remote_output(response, writer)
            .map_err(io_error)?;
    }
    if next_session.is_some() {
        writeln!(writer, "Selected client-instance/session-view refreshed.").map_err(io_error)?;
    }
    if next_resource.is_some() {
        writeln!(writer, "Selected client-instance/resource-view refreshed.").map_err(io_error)?;
    }
    state.client_instance_session_view_observed = next_session;
    state.client_instance_resource_view_observed = next_resource;
    state.refresh_client_instance_observation_status();
    Ok(true)
}

type ClientInstancePair = (Option<serde_json::Value>, Option<serde_json::Value>);

async fn read_pair(
    client: &RemoteClient,
    read_session: bool,
    read_resource: bool,
) -> Result<ClientInstancePair, (&'static str, RemoteError)> {
    let session = if read_session {
        Some(
            client
                .read_client_instance_session_view()
                .await
                .map_err(|error| ("session-view", error))?,
        )
    } else {
        None
    };
    let resource = if read_resource {
        Some(
            client
                .read_client_instance_resource_view()
                .await
                .map_err(|error| ("resource-view", error))?,
        )
    } else {
        None
    };
    Ok((session, resource))
}

fn report_refresh_failure<W: Write>(
    state: &mut TuiState,
    kind: &str,
    error: &RemoteError,
    writer: &mut W,
    previous_session: Option<serde_json::Value>,
    previous_resource: Option<serde_json::Value>,
) -> Result<bool, RemoteError> {
    let authorization_cleared = super::clear_session_view_after_authorization_error(state, error);
    let projection_cleared = if authorization_cleared {
        false
    } else {
        state.client_instance_session_view_observed = previous_session;
        state.client_instance_resource_view_observed = previous_resource;
        state.mark_client_instance_observations_not_converged();
        false
    };
    writeln!(
        writer,
        "Sync client-instance/{kind} refresh failed: {error}. The change cursor was not advanced."
    )
    .map_err(io_error)?;
    if authorization_cleared {
        writeln!(
            writer,
            "Local session view cleared after authorization failure."
        )
        .map_err(io_error)?;
    }
    if projection_cleared {
        writeln!(
            writer,
            "Local client-instance/{kind} view cleared after refresh failure."
        )
        .map_err(io_error)?;
    } else if !authorization_cleared {
        writeln!(
            writer,
            "Previous client-instance observations were retained for display only; no mixed pair was committed and filtering/private reads through this client-instance projection remain blocked until both snapshots converge."
        )
        .map_err(io_error)?;
    }
    Ok(false)
}
