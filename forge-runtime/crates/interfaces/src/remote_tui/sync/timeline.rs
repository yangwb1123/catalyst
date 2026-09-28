use super::{RemoteClient, RemoteError, TuiState, io_error};
use serde_json::Value;
use std::io::Write;

/// Refreshes the Run selected by the last `timeline` command from its
/// in-process sequence. This gives the TUI the same bounded metadata-only
/// forward observation that the Flutter screen performs during its periodic
/// sync, while leaving durable cross-process resume to `timeline RUN --resume`.
pub(in super::super) async fn sync_selected_run_timeline<W: Write>(
    client: &RemoteClient,
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
    if !super::super::commands::ensure_conversation_visible_to_client_instance(
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
            super::super::runs::RUN_TIMELINE_LIMIT,
        )
        .await
    {
        Ok(page) => page,
        Err(error) => {
            report_timeline_error(state, &conversation_id, &error, writer)?;
            return Ok(false);
        }
    };
    if !record_timeline(state, &conversation_id, &page, after_sequence, writer)? {
        return Ok(false);
    }
    sync_run_observation(client, state, &conversation_id, &run_id, writer).await
}

pub(super) async fn sync_run_observation<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    conversation_id: &str,
    run_id: &str,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    // Keep the selected Run's summary metadata alongside its incremental
    // timeline. Both reads are owner/path bound and display-only; the
    // observation is refreshed only after the timeline response has passed
    // its cursor check.
    let observation = match client.read_run_observation(conversation_id, run_id).await {
        Ok(observation) => observation,
        Err(error) => {
            report_observation_error(state, conversation_id, &error, writer)?;
            return Ok(false);
        }
    };
    crate::device_run_observed_command::write_remote_output(&observation, writer)
        .map_err(io_error)?;
    state.selected_run_observed = Some(observation);
    Ok(true)
}

pub(super) fn record_timeline<W: Write>(
    state: &mut TuiState,
    conversation_id: &str,
    page: &Value,
    after_sequence: u64,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    let scanned_through_sequence = page
        .get("scanned_through_sequence")
        .and_then(Value::as_u64)
        .ok_or_else(|| RemoteError("Forge API returned an invalid Run timeline".into()));
    let scanned_through_sequence = match scanned_through_sequence {
        Ok(sequence) => sequence,
        Err(error) => {
            let dropped = super::super::commands::drop_selected_session_after_read_rejection(
                state,
                conversation_id,
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
    super::super::runs::render_timeline(page, writer)?;
    state.run_timeline_sequence = scanned_through_sequence;

    Ok(true)
}

pub(super) fn report_timeline_error<W: Write>(
    state: &mut TuiState,
    conversation_id: &str,
    error: &RemoteError,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let cleared = super::super::clear_session_view_after_authorization_error(state, error);
    let dropped = if cleared {
        false
    } else {
        super::super::commands::drop_selected_session_after_read_rejection(
            state,
            conversation_id,
            error,
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
    Ok(())
}

pub(super) fn report_observation_error<W: Write>(
    state: &mut TuiState,
    conversation_id: &str,
    error: &RemoteError,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let cleared = super::super::clear_session_view_after_authorization_error(state, error);
    let dropped = if cleared {
        false
    } else {
        super::super::commands::drop_selected_session_after_read_rejection(
            state,
            conversation_id,
            error,
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
    Ok(())
}
