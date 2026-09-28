use std::io::Write;

use super::super::state::{TuiState, io_error};
use super::super::{RemoteClient, RemoteError};

/// Reads one owner-bound client-instance observation through the authenticated
/// remote transport and renders only its bounded display projection. The
/// server may mount it only after device-fabric activation; normal construction
/// keeps the route closed.
pub(super) async fn show<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    match argument.trim() {
        "session-view" => read_session_view(client, state, writer).await?,
        "resource-view" => read_resource_view(client, state, writer).await?,
        "show-converged" => read_converged_views(client, state, writer).await?,
        "clear session-view" | "clear resource-view" => {
            let kind = argument.trim().strip_prefix("clear ").unwrap();
            if state.clear_client_instance_view(kind) {
                writeln!(
                    writer,
                    "Client-instance/{kind} cleared; any active instance filter remains fail-closed until a new reader succeeds."
                )
                .map_err(io_error)?;
            } else {
                writeln!(writer, "No client-instance/{kind} reader is open.").map_err(io_error)?;
            }
        }
        _ => write_usage(writer)?,
    }
    Ok(())
}

fn report_failure<W: Write>(
    state: &mut TuiState,
    kind: &str,
    error: &RemoteError,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let (authorization_cleared, projection_cleared) =
        super::super::clear_client_instance_view_after_failure(state, kind, error);
    writeln!(
        writer,
        "Remote client-instance/{kind} request failed: {error}"
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
            "Existing client-instance observations were retained for display only; filtering and private reads through this client-instance projection are blocked until both snapshots converge."
        )
        .map_err(io_error)?;
    }
    Ok(())
}

fn report_converged_failure<W: Write>(
    state: &mut TuiState,
    error: &RemoteError,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let cleared = super::super::clear_session_view_after_authorization_error(state, error);
    writeln!(
        writer,
        "Remote client-instance session/resource convergence request failed: {error}"
    )
    .map_err(io_error)?;
    if cleared {
        writeln!(
            writer,
            "Local session view cleared after authorization failure."
        )
        .map_err(io_error)?;
    } else {
        state.mark_client_instance_observations_not_converged();
        writeln!(
            writer,
            "Previous client-instance snapshots were retained; no mixed pair was committed."
        )
        .map_err(io_error)?;
    }
    Ok(())
}

fn write_usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use client-instances session-view, client-instances resource-view, client-instances show-converged, or client-instances clear session-view|resource-view. These are authenticated, owner-bound display observations available only when the server's activation gate mounts them; they do not register an instance, bind a session, select a device, reserve capacity, or execute work."
    )
    .map_err(io_error)
}

async fn read_session_view<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let response = match client.read_client_instance_session_view().await {
        Ok(response) => response,
        Err(error) => return report_failure(state, "session-view", &error, writer),
    };
    crate::device_client_session_view_command::write_remote_output(&response, writer).map_err(
        |error| {
            RemoteError(format!(
                "client-instance/session-view response could not be rendered: {error}"
            ))
        },
    )?;
    // An explicit authenticated read opts this TUI process into
    // refreshing the same owner-bound observation during later `sync`
    // commands. The observation is display-only and never enables
    // instance registration or session authority.
    state.client_instance_session_view_observed = Some(response);
    state.refresh_client_instance_observation_status();
    state.reconcile_client_instance_selection();
    Ok(())
}

async fn read_resource_view<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let response = match client.read_client_instance_resource_view().await {
        Ok(response) => response,
        Err(error) => return report_failure(state, "resource-view", &error, writer),
    };
    crate::device_client_instance_resource_view_command::write_remote_output(&response, writer)
        .map_err(|error| {
            RemoteError(format!(
                "client-instance/resource-view response could not be rendered: {error}"
            ))
        })?;
    // Keep refresh opt-in and process-local, matching session-view.
    state.client_instance_resource_view_observed = Some(response);
    state.refresh_client_instance_observation_status();
    state.reconcile_client_instance_selection();
    Ok(())
}

async fn read_converged_views<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let response = match client.read_converged_client_instance_views().await {
        Ok(response) => response,
        Err(error) => return report_converged_failure(state, &error, writer),
    };
    let session_view = response.get("session_view").ok_or_else(|| {
        RemoteError("Forge API returned an invalid client-instance convergence envelope".into())
    })?;
    let resource_view = response.get("resource_view").ok_or_else(|| {
        RemoteError("Forge API returned an invalid client-instance convergence envelope".into())
    })?;
    writeln!(
        writer,
        "remote client-instance/session-resource-convergence [forge.client-instance-session-resource-convergence/v1] converged=true read_only=true"
    )
    .map_err(io_error)?;
    crate::device_client_session_view_command::write_remote_output(session_view, writer).map_err(
        |error| {
            RemoteError(format!(
                "client-instance/session-view response could not be rendered: {error}"
            ))
        },
    )?;
    crate::device_client_instance_resource_view_command::write_remote_output(resource_view, writer)
        .map_err(|error| {
            RemoteError(format!(
                "client-instance/resource-view response could not be rendered: {error}"
            ))
        })?;
    writeln!(
        writer,
        "Client-instance session/resource observations converged; both snapshots committed."
    )
    .map_err(io_error)?;
    // Commit both projections only after the paired transport and
    // strict source renderers have succeeded. A failed pair therefore
    // cannot leave the TUI with a newly mixed session/resource image.
    state.client_instance_session_view_observed = Some(session_view.clone());
    state.client_instance_resource_view_observed = Some(resource_view.clone());
    state.mark_client_instance_observations_converged();
    state.reconcile_client_instance_selection();
    Ok(())
}
