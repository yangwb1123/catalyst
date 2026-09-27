use std::io::Write;

use serde_json::Value;

use super::super::changes::OwnedConversationChange;
use super::super::{RemoteClient, RemoteError};
use super::state::{io_error, refresh_sessions};
use super::{load_session_history, state::TuiState};

pub(super) async fn sync_command<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<(), RemoteError> {
    // Drain a bounded number of dense owner-feed pages in one sync. The
    // checkpoint is still committed only after the conversation snapshot and
    // selected history have been refreshed, so a partial client refresh never
    // skips unread changes. Leave `has_more` visible when the bound is hit so
    // the next explicit sync continues from the committed cursor.
    const MAX_CHANGE_PAGES_PER_SYNC: usize = 4;
    let mut next_cursor = state.change_cursor;
    let mut change_count = 0usize;
    let mut has_more = false;
    for _ in 0..MAX_CHANGE_PAGES_PER_SYNC {
        let page = match client.conversation_changes_after(next_cursor).await {
            Ok(page) => page,
            Err(error) => {
                let cleared = super::clear_session_view_after_authorization_error(state, &error);
                writeln!(
                    writer,
                    "Sync change feed request failed: {error}. The change cursor was not advanced."
                )
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
        if page.scanned_through_cursor < next_cursor {
            writeln!(
                writer,
                "Sync change feed made the cursor regress. The change cursor was not advanced."
            )
            .map_err(io_error)?;
            return Ok(());
        }
        for change in &page.changes {
            apply_conversation_change(state, change);
        }
        change_count += page.changes.len();
        next_cursor = page.scanned_through_cursor;
        has_more = page.has_more;
        if !has_more {
            break;
        }
    }
    // An active client-instance filter is a private display boundary for the
    // selected Conversation. Refresh its explicit owner-bound observation
    // before the Conversation snapshot can hydrate Prompt history or Run
    // metadata; otherwise a revocation arriving in this sync would still
    // permit one round of reads under the previous instance image. The
    // default TUI path has no filter and keeps this candidate request-free.
    let previous_resource_view = state.client_instance_resource_view_observed.clone();
    let client_instance_refreshed_before_owner_reads = state.client_instance_filter.is_some();
    if client_instance_refreshed_before_owner_reads
        && !super::sync_client_instances::sync_client_instance_views(client, state, writer).await?
    {
        return Ok(());
    }
    if let Err(error) = refresh_sessions(client, state, false).await {
        let cleared = super::clear_session_view_after_authorization_error(state, &error);
        writeln!(
            writer,
            "Sync session refresh failed: {error}. The change cursor was not advanced."
        )
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
    if let Some(selected_id) = state.selected_id.clone() {
        if super::commands::ensure_conversation_visible_to_client_instance(
            state,
            &selected_id,
            writer,
        )? {
            let history = match load_session_history(client, &selected_id).await {
                Ok(history) => history,
                Err(error) => {
                    let cleared =
                        super::clear_session_view_after_authorization_error(state, &error);
                    let dropped = if cleared {
                        false
                    } else {
                        super::commands::drop_selected_session_after_read_rejection(
                            state,
                            &selected_id,
                            &error,
                        )
                    };
                    writeln!(writer, "Sync Prompt history refresh failed: {error}. The change cursor was not advanced.")
                        .map_err(io_error)?;
                    if cleared {
                        writeln!(
                            writer,
                            "Local session view cleared after authorization failure."
                        )
                        .map_err(io_error)?;
                    } else if dropped {
                        writeln!(
                            writer,
                            "Selected session was removed after its owner Prompt read was rejected."
                        )
                        .map_err(io_error)?;
                    }
                    return Ok(());
                }
            };
            state.record_prompt_history(&selected_id, &history);
            writeln!(writer, "Prompt history refreshed for the selected session.")
                .map_err(io_error)?;
        }
    }
    if !sync_selected_run_timeline(client, state, writer).await? {
        // Keep the conversation cursor unchanged when the selected Run could
        // not be refreshed. The next explicit sync retries both observations
        // from their last committed checkpoints.
        return Ok(());
    }
    if !sync_device_inventory(client, state, writer).await? {
        // An explicitly opened inventory view is part of the same owner
        // refresh boundary. Do not advance the conversation cursor when its
        // replacement could not be validated.
        return Ok(());
    }
    let previous_inventory_v2 = state.device_inventory_v2_observed.clone();
    if !sync_device_inventory_v2(client, state, writer).await? {
        // An explicitly opened inventory view is part of the same owner
        // refresh boundary. Do not advance the conversation cursor when its
        // replacement could not be validated.
        return Ok(());
    }
    let previous_client_instance_session = state.client_instance_session_view_observed.clone();
    let previous_client_instance_resource = state.client_instance_resource_view_observed.clone();
    if !sync_pending_run_intent_page(client, state, writer).await? {
        // An explicitly opened pending Run-intent page is another owner-bound
        // observation. Keep the conversation cursor unchanged when its
        // replacement cannot be validated, so the next sync retries the same
        // page rather than silently dropping the refresh boundary.
        return Ok(());
    }
    if !client_instance_refreshed_before_owner_reads
        && !super::sync_client_instances::sync_client_instance_views(client, state, writer).await?
    {
        // Client-instance observations are explicit owner-bound reads.
        // Keep the conversation cursor unchanged when either replacement
        // cannot be validated so the next sync retries the same boundary.
        return Ok(());
    }
    if let (Some(previous_session), Some(previous_resource)) = (
        previous_client_instance_session,
        previous_client_instance_resource,
    ) {
        let current_session = state.client_instance_session_view_observed.as_ref();
        let current_resource = state.client_instance_resource_view_observed.as_ref();
        if let (Some(current_session), Some(current_resource)) = (current_session, current_resource)
            && !super::client_instance_convergence::observations_converged(
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
            return Ok(());
        }
    }
    if previous_inventory_v2.is_some()
        && previous_resource_view.is_some()
        && !super::inventory_convergence::observations_converged(
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
        return Ok(());
    }
    // Commit selection only after the optional paired client-instance reads
    // have passed their convergence boundary. During a mixed refresh the
    // active projection intentionally returns None, so a Prompt/Run selection
    // cannot be cleared and then lost before the previous pair is restored.
    state.reconcile_client_instance_selection();
    client.persist_change_cursor(next_cursor)?;
    state.change_cursor = next_cursor;
    writeln!(
        writer,
        "Synced {change_count} owner-visible changes through cursor {next_cursor}{}.",
        if has_more { "; more remain" } else { "" }
    )
    .map_err(io_error)?;
    Ok(())
}

/// Refreshes the v1 inventory only after the user explicitly opened it with
/// `inventory read`. The candidate is owner-derived by the server and
/// validated by the remote client; this process-local opt-in prevents a
/// normal session sync from silently probing a disabled device route.
async fn sync_device_inventory<W: Write>(
    client: &super::super::RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    if state.device_inventory_observed.is_none() {
        return Ok(true);
    }
    let value = match client.read_device_inventory().await {
        Ok(value) => value,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
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
    super::commands::write_remote_inventory(&value, writer)?;
    state.device_inventory_observed = Some(value);
    writeln!(writer, "Selected inventory observation refreshed.").map_err(io_error)?;
    Ok(true)
}

/// Refreshes the exact pending Run-intent metadata page opened by the user.
/// This remains opt-in process-local state: a normal sync never requests the
/// private candidate unless `run-intents` first established the binding.
async fn sync_pending_run_intent_page<W: Write>(
    client: &super::super::RemoteClient,
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
    if !super::writes::ensure_pending_run_intent_visible_to_client_instance(
        state,
        &observation.conversation_id,
        writer,
    )? {
        return Ok(true);
    }
    let page = match client
        .list_pending_run_intents(
            &observation.conversation_id,
            super::pending_run_intents::PAGE_LIMIT,
            observation.before_submitted_at_ms,
            observation.before_intent_id.as_deref(),
        )
        .await
    {
        Ok(page) => page,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
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
            return Ok(false);
        }
    };
    super::pending_run_intents::render_page(&page, writer)?;
    state.record_pending_run_intent_page(
        &observation.conversation_id,
        observation
            .before_submitted_at_ms
            .zip(observation.before_intent_id),
    );
    writeln!(writer, "Selected pending Run-intent metadata refreshed.").map_err(io_error)?;
    Ok(true)
}

/// Refreshes the lossless v2 inventory only after the user explicitly opened
/// it with `inventory read-v2`. The candidate is owner-derived by the server
/// and validated by the remote client; this process-local opt-in prevents a
/// normal session sync from silently probing a disabled device route.
async fn sync_device_inventory_v2<W: Write>(
    client: &super::super::RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    if state.device_inventory_v2_observed.is_none() {
        return Ok(true);
    }
    let value = match client.read_device_inventory_v2().await {
        Ok(value) => value,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
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
    super::commands::write_remote_inventory_v2(&value, writer)?;
    state.device_inventory_v2_observed = Some(value);
    writeln!(writer, "Selected inventory observation refreshed.").map_err(io_error)?;
    Ok(true)
}

/// Refreshes the Run selected by the last `timeline` command from its
/// in-process sequence. This gives the TUI the same bounded metadata-only
/// forward observation that the Flutter screen performs during its periodic
/// sync, while leaving durable cross-process resume to `timeline RUN --resume`.
pub(super) async fn sync_selected_run_timeline<W: Write>(
    client: &super::super::RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    let Some(run_id) = state.selected_run_id.clone() else {
        return Ok(true);
    };
    let Some(conversation_id) = state.selected_id.clone() else {
        state.clear_run_timeline();
        return Ok(true);
    };
    if !super::commands::ensure_conversation_visible_to_client_instance(
        state,
        &conversation_id,
        writer,
    )? {
        // A selected Run may outlive a local instance projection change. Drop
        // only the Run observation so hidden metadata cannot remain visible;
        // pending Run-intent recovery state is intentionally preserved.
        state.selected_run_id = None;
        state.run_timeline_sequence = 0;
        state.selected_run_observed = None;
        return Ok(true);
    }
    let after_sequence = state.run_timeline_sequence;
    let page = match client
        .run_timeline(
            &conversation_id,
            &run_id,
            after_sequence,
            super::runs::RUN_TIMELINE_LIMIT,
        )
        .await
    {
        Ok(page) => page,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            let dropped = if cleared {
                false
            } else {
                super::commands::drop_selected_session_after_read_rejection(
                    state,
                    &conversation_id,
                    &error,
                )
            };
            writeln!(
                writer,
                "Sync Run timeline refresh failed: {error}. The change cursor was not advanced."
            )
            .map_err(io_error)?;
            if cleared {
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(io_error)?;
            } else if dropped {
                writeln!(
                    writer,
                    "Selected session was removed after its owner Run read was rejected."
                )
                .map_err(io_error)?;
            }
            return Ok(false);
        }
    };
    let scanned_through_sequence = page
        .get("scanned_through_sequence")
        .and_then(Value::as_u64)
        .ok_or_else(|| RemoteError("Forge API returned an invalid Run timeline".into()));
    let scanned_through_sequence = match scanned_through_sequence {
        Ok(sequence) => sequence,
        Err(error) => {
            let dropped = super::commands::drop_selected_session_after_read_rejection(
                state,
                &conversation_id,
                &error,
            );
            writeln!(
                writer,
                "Sync Run timeline refresh failed: {error}. The change cursor was not advanced."
            )
            .map_err(io_error)?;
            if dropped {
                writeln!(
                    writer,
                    "Selected session was removed after its owner Run read was rejected."
                )
                .map_err(io_error)?;
            }
            return Ok(false);
        }
    };
    if scanned_through_sequence < after_sequence {
        writeln!(
            writer,
            "Sync Run timeline cursor regressed. The change cursor was not advanced."
        )
        .map_err(io_error)?;
        return Ok(false);
    }
    super::runs::render_timeline(&page, writer)?;
    state.run_timeline_sequence = scanned_through_sequence;

    // Keep the selected Run's summary metadata alongside its incremental
    // timeline. Both reads are owner/path bound and display-only; the
    // observation is refreshed only after the timeline response has passed
    // its cursor check.
    let observation = match client.read_run_observation(&conversation_id, &run_id).await {
        Ok(observation) => observation,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            let dropped = if cleared {
                false
            } else {
                super::commands::drop_selected_session_after_read_rejection(
                    state,
                    &conversation_id,
                    &error,
                )
            };
            writeln!(
                writer,
                "Sync Run observation refresh failed: {error}. The change cursor was not advanced."
            )
            .map_err(io_error)?;
            if cleared {
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(io_error)?;
            } else if dropped {
                writeln!(
                    writer,
                    "Selected session was removed after its owner Run read was rejected."
                )
                .map_err(io_error)?;
            }
            return Ok(false);
        }
    };
    crate::device_run_observed_command::write_remote_output(&observation, writer)
        .map_err(io_error)?;
    state.selected_run_observed = Some(observation);
    Ok(true)
}

pub(super) fn apply_conversation_change(state: &mut TuiState, change: &OwnedConversationChange) {
    for entry in state
        .conversations
        .iter_mut()
        .chain(state.selected_entry.iter_mut())
    {
        if entry.conversation.get("id").and_then(Value::as_str)
            == Some(change.conversation_id.as_str())
        {
            entry.aggregate_version = entry.aggregate_version.max(change.aggregate_version);
        }
    }
}
