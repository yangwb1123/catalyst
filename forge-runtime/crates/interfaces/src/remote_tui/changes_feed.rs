use std::io::Write;

use serde_json::Value;

use crate::client_instance_session_scope;

use super::super::super::super::changes::OwnedConversationChange;
use super::super::super::state::{TuiState, io_error, json_text};
use super::super::super::{RemoteClient, RemoteError};
use super::{ListOptions, StreamOptions, StreamResult, WatchOptions, WatchResult};
pub(super) async fn list<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    options: ListOptions,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if !refresh_client_instance_before_change_feed(
        client,
        state,
        options.instance_id.as_deref(),
        writer,
    )
    .await?
    {
        return Ok(());
    }
    let expected_start = match options.after_cursor {
        Some(cursor) => cursor,
        None => client.saved_change_cursor()?,
    };
    let page = match client
        .resumed_conversation_changes(options.after_cursor)
        .await
    {
        Ok(page) => page,
        Err(error) => {
            report_feed_request_failure(state, &error, "list", writer)?;
            return Ok(());
        }
    };
    let changes = project_changes(state, &page.changes)?;
    apply_changes(state, &changes, page.scanned_through_cursor);
    let checkpoint = if options.after_cursor.is_some() {
        "one-off; saved checkpoint unchanged"
    } else {
        "saved checkpoint advanced after a valid page"
    };
    writeln!(
        writer,
        "Changes list start_cursor={} scanned_through_cursor={} changes={} has_more={} ({checkpoint}).",
        expected_start,
        page.scanned_through_cursor,
        changes.len(),
        page.has_more,
    )
    .map_err(io_error)?;
    write_change_rows(writer, &changes)?;
    write_list_continuation(page.has_more, writer)?;
    Ok(())
}

pub(super) async fn watch<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    options: WatchOptions,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if !refresh_client_instance_before_change_feed(
        client,
        state,
        options.instance_id.as_deref(),
        writer,
    )
    .await?
    {
        return Ok(());
    }
    let expected_start = match options.after_cursor {
        Some(cursor) => cursor,
        None => client.saved_change_cursor()?,
    };
    let response = match client
        .watch_conversation_changes(
            options.after_cursor,
            options.polls,
            options.min_delay_ms,
            options.max_delay_ms,
        )
        .await
    {
        Ok(response) => response,
        Err(error) => {
            report_feed_request_failure(state, &error, "watch", writer)?;
            return Ok(());
        }
    };
    let result = match decode_result(&response, expected_start, options.polls) {
        Ok(result) => result,
        Err(error) => {
            writeln!(writer, "Changes watch response failed validation: {error}")
                .map_err(io_error)?;
            return Ok(());
        }
    };

    apply_and_render_watch(state, &result, options.after_cursor.is_some(), writer)
}

pub(super) async fn stream<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    options: StreamOptions,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if !refresh_client_instance_before_change_feed(
        client,
        state,
        options.instance_id.as_deref(),
        writer,
    )
    .await?
    {
        return Ok(());
    }
    let expected_start = match options.after_cursor {
        Some(cursor) => cursor,
        None => client.saved_change_cursor()?,
    };
    let response = match client
        .stream_conversation_changes(options.after_cursor, options.wait_ms)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            report_feed_request_failure(state, &error, "stream", writer)?;
            return Ok(());
        }
    };
    let result = match decode_stream_result(&response, expected_start) {
        Ok(result) => result,
        Err(error) => {
            writeln!(writer, "Changes stream response failed validation: {error}")
                .map_err(io_error)?;
            return Ok(());
        }
    };
    apply_and_render_stream(
        state,
        &result,
        options.after_cursor.is_some(),
        options.wait_ms,
        writer,
    )
}

/// Refreshes an explicitly selected client-instance projection before a
/// change-feed request. The feed is owner-scoped, but its rows are applied to
/// the local session projection; using an older session/resource image could
/// therefore reintroduce a revoked or moved Conversation into that view.
///
/// The default TUI path has no instance filter and remains request-compatible:
/// it performs no extra reads before the existing watch/stream request.
async fn refresh_client_instance_before_change_feed<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    requested_instance_id: Option<&str>,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    let filter_changed = requested_instance_id
        .is_some_and(|instance_id| state.client_instance_filter.as_deref() != Some(instance_id));
    if let Some(instance_id) = requested_instance_id {
        state.client_instance_filter = Some(instance_id.to_owned());
        if filter_changed {
            state.clear_client_instance_private_projection();
        }
    }
    let Some(instance_id) = state.client_instance_filter.clone() else {
        return Ok(true);
    };
    let previous_selected_id = state.selected_id.clone();
    if !refresh_converged_instance_projection(client, state, &instance_id, writer).await? {
        return Ok(false);
    }
    if filter_changed || previous_selected_id.is_none() {
        return Ok(true);
    }
    let previous_selected_id = previous_selected_id.expect("checked above");
    super::super::super::commands::ensure_conversation_visible_to_client_instance(
        state,
        &previous_selected_id,
        writer,
    )
}

async fn refresh_converged_instance_projection<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    instance_id: &str,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    let response = match client.read_converged_client_instance_views().await {
        Ok(response) => response,
        Err(error) => return report_projection_refresh_failure(state, &error, writer),
    };
    let session_view = response
        .get("session_view")
        .cloned()
        .ok_or_else(|| RemoteError("Forge API returned an invalid client-instance pair".into()))?;
    let resource_view = response
        .get("resource_view")
        .cloned()
        .ok_or_else(|| RemoteError("Forge API returned an invalid client-instance pair".into()))?;
    if client_instance_session_scope::scope_from_view(&session_view, instance_id).is_err() {
        writeln!(
            writer,
            "Changes feed blocked: client-instance {instance_id:?} is not declared by the converged session/resource pair. The in-memory TUI cursor was not advanced."
        )
        .map_err(io_error)?;
        return Ok(false);
    }
    state.client_instance_session_view_observed = Some(session_view);
    state.client_instance_resource_view_observed = Some(resource_view);
    state.mark_client_instance_observations_converged();
    state.reconcile_client_instance_selection();
    writeln!(writer, "Selected client-instance/session-view refreshed.").map_err(io_error)?;
    writeln!(writer, "Selected client-instance/resource-view refreshed.").map_err(io_error)?;
    Ok(true)
}

fn report_projection_refresh_failure<W: Write>(
    state: &mut TuiState,
    error: &RemoteError,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    let cleared = super::super::super::clear_session_view_after_authorization_error(state, error);
    if !cleared {
        state.mark_client_instance_observations_not_converged();
    }
    writeln!(
        writer,
        "Changes feed client-instance projection refresh failed: {error}. The in-memory TUI cursor was not advanced."
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
            "Previous client-instance observations were retained for display only; filtering remains blocked until both snapshots converge."
        )
        .map_err(io_error)?;
    }
    Ok(false)
}

fn project_changes(
    state: &TuiState,
    changes: &[OwnedConversationChange],
) -> Result<Vec<OwnedConversationChange>, RemoteError> {
    let Some(instance_id) = state.client_instance_filter.as_deref() else {
        return Ok(changes.to_vec());
    };
    let view = state
        .active_client_instance_view()
        .ok_or_else(|| RemoteError("remote TUI client-instance view is unavailable".into()))?;
    let scope = client_instance_session_scope::scope_from_view(view, instance_id)
        .map_err(|error| RemoteError(format!("client-instance projection is invalid: {error}")))?;
    Ok(changes
        .iter()
        .filter(|change| scope.session_ids.contains(&change.conversation_id))
        .cloned()
        .collect())
}

fn apply_changes(state: &mut TuiState, changes: &[OwnedConversationChange], scanned_cursor: u64) {
    for change in changes {
        super::super::super::sync::apply_conversation_change(state, change);
    }
    // The feed cursor covers owner-visible rows, including rows hidden by the
    // local instance projection. Advancing it prevents hidden rows from being
    // replayed on every subsequent watch/list/stream call.
    state.change_cursor = scanned_cursor;
}

fn write_change_rows<W: Write>(
    writer: &mut W,
    changes: &[OwnedConversationChange],
) -> Result<(), RemoteError> {
    for change in changes {
        writeln!(
            writer,
            "  Change cursor={} conversation={} entity={} aggregate_version={} kind={} created_at_ms={}",
            change.cursor,
            json_text(&change.conversation_id),
            json_text(&change.entity_id),
            change.aggregate_version,
            json_text(&change.kind),
            change.created_at_ms,
        )
        .map_err(io_error)?;
    }
    Ok(())
}

fn decode_stream_result(value: &Value, expected_start: u64) -> Result<StreamResult, RemoteError> {
    let result: StreamResult = serde_json::from_value(value.clone())
        .map_err(|_| RemoteError("Forge API returned an invalid change stream".into()))?;
    if result.start_cursor != expected_start || result.scanned_through_cursor < expected_start {
        return Err(RemoteError(
            "Forge API returned an invalid change stream".into(),
        ));
    }
    if result.timed_out {
        if result.scanned_through_cursor != expected_start
            || result.has_more
            || !result.changes.is_empty()
        {
            return Err(RemoteError(
                "Forge API returned an invalid change stream timeout".into(),
            ));
        }
        return Ok(result);
    }
    let mut previous = expected_start;
    for change in &result.changes {
        let expected = previous
            .checked_add(1)
            .ok_or_else(|| RemoteError("Forge API returned an invalid change stream".into()))?;
        if change.cursor != expected || change.cursor > result.scanned_through_cursor {
            return Err(RemoteError(
                "Forge API returned an invalid change stream".into(),
            ));
        }
        previous = change.cursor;
    }
    if previous != result.scanned_through_cursor {
        return Err(RemoteError(
            "Forge API returned an invalid change stream".into(),
        ));
    }
    Ok(result)
}

fn decode_result(
    value: &Value,
    expected_start: u64,
    expected_polls: usize,
) -> Result<WatchResult, RemoteError> {
    let result: WatchResult = serde_json::from_value(value.clone())
        .map_err(|_| RemoteError("Forge API returned an invalid change watch".into()))?;
    if result.start_cursor != expected_start
        || result.polls != expected_polls
        || result.scanned_through_cursor < result.start_cursor
    {
        return Err(RemoteError(
            "Forge API returned an invalid change watch".into(),
        ));
    }
    let mut previous = result.start_cursor;
    for change in &result.changes {
        let expected = previous
            .checked_add(1)
            .ok_or_else(|| RemoteError("Forge API returned an invalid change watch".into()))?;
        if change.cursor != expected || change.cursor > result.scanned_through_cursor {
            return Err(RemoteError(
                "Forge API returned an invalid change watch".into(),
            ));
        }
        previous = change.cursor;
    }
    if previous != result.scanned_through_cursor {
        return Err(RemoteError(
            "Forge API returned an invalid change watch".into(),
        ));
    }
    Ok(result)
}

fn report_feed_request_failure<W: Write>(
    state: &mut TuiState,
    error: &RemoteError,
    operation: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let cleared = super::super::super::clear_session_view_after_authorization_error(state, error);
    writeln!(
        writer,
        "Changes {operation} request failed: {error}. The in-memory TUI cursor was not advanced."
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

fn apply_and_render_watch<W: Write>(
    state: &mut TuiState,
    result: &WatchResult,
    explicit_cursor: bool,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let changes = project_changes(state, &result.changes)?;
    apply_changes(state, &changes, result.scanned_through_cursor);

    let checkpoint = if explicit_cursor {
        "one-off; saved checkpoint unchanged"
    } else {
        "saved checkpoint advanced per valid page"
    };
    writeln!(
        writer,
        "Changes watch start_cursor={} scanned_through_cursor={} polls={} changes={} has_more={} ({checkpoint}).",
        result.start_cursor,
        result.scanned_through_cursor,
        result.polls,
        changes.len(),
        result.has_more,
    )
    .map_err(io_error)?;
    write_change_rows(writer, &changes)?;
    if result.has_more {
        writeln!(
            writer,
            "  More owner-visible changes remain; run changes watch again to continue."
        )
        .map_err(io_error)?;
    }
    Ok(())
}

fn apply_and_render_stream<W: Write>(
    state: &mut TuiState,
    result: &StreamResult,
    explicit_cursor: bool,
    wait_ms: u64,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let changes = project_changes(state, &result.changes)?;
    apply_changes(state, &changes, result.scanned_through_cursor);
    let checkpoint = if explicit_cursor {
        "one-off; saved checkpoint unchanged"
    } else {
        "saved checkpoint advanced after a valid page"
    };
    writeln!(
        writer,
        "Changes stream start_cursor={} scanned_through_cursor={} wait_ms={} changes={} has_more={} timed_out={} ({checkpoint}).",
        result.start_cursor,
        result.scanned_through_cursor,
        wait_ms,
        changes.len(),
        result.has_more,
        result.timed_out,
    )
    .map_err(io_error)?;
    write_change_rows(writer, &changes)?;
    if result.has_more {
        writeln!(
            writer,
            "  More owner-visible changes remain; run changes stream again to continue."
        )
        .map_err(io_error)?;
    }
    Ok(())
}

fn write_list_continuation<W: Write>(has_more: bool, writer: &mut W) -> Result<(), RemoteError> {
    if has_more {
        writeln!(
            writer,
            "  More owner-visible changes remain; run changes list again to continue."
        )
        .map_err(io_error)?;
    }
    Ok(())
}
