use std::io::Write;

#[path = "inventory/offline.rs"]
mod offline;
#[path = "inventory/render.rs"]
mod render;
use offline::{
    show_inventory_observation, show_inventory_persisted_observation,
    show_inventory_persisted_observation_v2, show_inventory_persistence_preview,
    show_inventory_placement_batch_evaluation, show_inventory_placement_evaluation,
    show_inventory_placement_evaluation_v2, show_inventory_resource_summary,
    show_inventory_session_observation, show_inventory_snapshot_canonical, show_inventory_status,
    write_inventory_usage,
};
pub(super) use render::{write_remote_inventory, write_remote_inventory_v2};

use super::super::state::{TuiState, io_error};
use super::super::{RemoteClient, RemoteError};

/// Displays one caller-supplied inventory observation in the TUI.
///
/// The TUI deliberately accepts a filesystem path only. Reading `-` here
/// would consume the interactive command stream, so stdin remains available
/// to the TUI and callers can use the standalone CLI for stdin input.
pub(super) async fn show_inventory<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if argument.trim() == "read" {
        return show_remote_inventory(client, state, writer).await;
    }
    if argument.trim() == "read-v2" {
        return show_remote_inventory_v2(client, state, writer).await;
    }
    if argument.trim() == "show-converged" {
        return show_converged_inventory_resource_view(client, state, writer).await;
    }
    let Some((kind, suffix)) = inventory_input(argument) else {
        return write_inventory_usage(writer);
    };
    if !suffix.chars().next().is_some_and(char::is_whitespace) {
        return write_inventory_usage(writer);
    }
    let input = suffix.trim();
    if input.is_empty() || input == "-" {
        return write_inventory_usage(writer);
    }
    match kind {
        "show" => show_inventory_observation(input, writer)?,
        "persistence-preview" => show_inventory_persistence_preview(input, writer)?,
        "persisted-observation" => show_inventory_persisted_observation(input, writer)?,
        "persisted-observation-v2" => show_inventory_persisted_observation_v2(input, writer)?,
        "placement-evaluation" => show_inventory_placement_evaluation(input, writer)?,
        "status" => show_inventory_status(input, writer)?,
        "snapshot-canonical" => show_inventory_snapshot_canonical(input, writer)?,
        "resource-summary" => show_inventory_resource_summary(input, writer)?,
        "placement-batch-evaluation" => show_inventory_placement_batch_evaluation(input, writer)?,
        "placement-evaluation-v2" => show_inventory_placement_evaluation_v2(input, writer)?,
        _ => show_inventory_session_observation(input, writer)?,
    }
    Ok(())
}

async fn show_remote_inventory<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let value = match client.read_device_inventory().await {
        Ok(value) => value,
        Err(error) => {
            let cleared = super::super::clear_session_view_after_authorization_error(state, &error);
            writeln!(writer, "Remote device inventory request failed: {error}")
                .map_err(io_error)?;
            if cleared {
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(io_error)?;
            }
            return Ok(());
        }
    };
    write_remote_inventory(&value, writer)?;
    // An explicit read opts this TUI process into refreshing the same
    // owner-scoped observation during later `sync` commands. The value is
    // display state only; it is never used as placement or execution input.
    state.device_inventory_observed = Some(value);
    Ok(())
}

async fn show_remote_inventory_v2<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let value = match client.read_device_inventory_v2().await {
        Ok(value) => value,
        Err(error) => {
            let cleared = super::super::clear_session_view_after_authorization_error(state, &error);
            writeln!(writer, "Remote v2 device inventory request failed: {error}")
                .map_err(io_error)?;
            if cleared {
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(io_error)?;
            }
            return Ok(());
        }
    };
    write_remote_inventory_v2(&value, writer)?;
    // An explicit read opts this TUI process into refreshing the same
    // owner-scoped observation during later `sync` commands. The value is
    // display state only; it is never used as placement or execution input.
    state.device_inventory_v2_observed = Some(value);
    Ok(())
}

async fn show_converged_inventory_resource_view<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let response = match client.read_converged_inventory_resource_view().await {
        Ok(response) => response,
        Err(error) => return report_convergence_failure(state, &error, writer),
    };
    let inventory = response.get("inventory").ok_or_else(|| {
        RemoteError("Forge API returned an invalid inventory/resource convergence envelope".into())
    })?;
    let resource_view = response.get("resource_view").ok_or_else(|| {
        RemoteError("Forge API returned an invalid inventory/resource convergence envelope".into())
    })?;
    writeln!(
        writer,
        "remote inventory/resource-convergence [forge.device-inventory-resource-convergence/v1] converged=true read_only=true"
    )
    .map_err(io_error)?;
    write_remote_inventory_v2(inventory, writer)?;
    crate::device_client_instance_resource_view_command::write_remote_output(resource_view, writer)
        .map_err(|error| {
            RemoteError(format!(
                "client-instance/resource-view response could not be rendered: {error}"
            ))
        })?;
    writeln!(
        writer,
        "Inventory/resource observations converged; both snapshots committed."
    )
    .map_err(io_error)?;
    // The client has validated both source observations and their join. Keep
    // the pair atomic in local TUI state so a later sync refreshes the same
    // owner-bound resource image rather than mixing independent reads.
    state.device_inventory_v2_observed = Some(inventory.clone());
    state.client_instance_resource_view_observed = Some(resource_view.clone());
    // The refreshed resource image can revoke the locally selected instance's
    // session_ids. Reconcile immediately so private Prompt/Run panels cannot
    // survive outside the newly committed display projection.
    state.reconcile_client_instance_selection();
    Ok(())
}

fn report_convergence_failure<W: Write>(
    state: &mut TuiState,
    error: &RemoteError,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let cleared = super::super::clear_session_view_after_authorization_error(state, error);
    writeln!(
        writer,
        "Remote inventory/resource convergence request failed: {error}"
    )
    .map_err(io_error)?;
    if cleared {
        writeln!(
            writer,
            "Local session view cleared after authorization failure."
        )
        .map_err(io_error)?;
    } else {
        writeln!(
            writer,
            "Previous inventory/resource snapshots were retained; no mixed pair was committed."
        )
        .map_err(io_error)?;
    }
    Ok(())
}

fn inventory_input(argument: &str) -> Option<(&str, &str)> {
    if let Some(suffix) = argument.strip_prefix("show --input") {
        return Some(("show", suffix));
    }
    if let Some(suffix) = argument.strip_prefix("persistence-preview --input") {
        return Some(("persistence-preview", suffix));
    }
    if let Some(suffix) = argument.strip_prefix("persisted-observation --input") {
        return Some(("persisted-observation", suffix));
    }
    if let Some(suffix) = argument.strip_prefix("persisted-observation-v2 --input") {
        return Some(("persisted-observation-v2", suffix));
    }
    if let Some(suffix) = argument.strip_prefix("placement-evaluation --input") {
        return Some(("placement-evaluation", suffix));
    }
    if let Some(suffix) = argument.strip_prefix("status --input") {
        return Some(("status", suffix));
    }
    if let Some(suffix) = argument.strip_prefix("snapshot-canonical --input") {
        return Some(("snapshot-canonical", suffix));
    }
    if let Some(suffix) = argument.strip_prefix("resource-summary --input") {
        return Some(("resource-summary", suffix));
    }
    if let Some(suffix) = argument.strip_prefix("session-observation --input") {
        return Some(("session-observation", suffix));
    }
    if let Some(suffix) = argument.strip_prefix("placement-batch-evaluation --input") {
        return Some(("placement-batch-evaluation", suffix));
    }
    if let Some(suffix) = argument.strip_prefix("placement-evaluation-v2 --input") {
        return Some(("placement-evaluation-v2", suffix));
    }
    None
}
