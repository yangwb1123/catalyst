use super::{ChangeProgress, apply_conversation_change};
use super::{RemoteClient, RemoteError, TuiState, io_error};
use std::io::Write;

pub(super) async fn drain_changes<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<Option<ChangeProgress>, RemoteError> {
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
                report_change_error(state, &error, writer)?;
                return Ok(None);
            }
        };
        if page.scanned_through_cursor < next_cursor {
            writeln!(
                writer,
                "Sync change feed made the cursor regress. The change cursor was not advanced."
            )
            .map_err(io_error)?;
            return Ok(None);
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
    Ok(Some(ChangeProgress {
        next_cursor,
        change_count,
        has_more,
    }))
}

pub(super) fn report_change_error<W: Write>(
    state: &mut TuiState,
    error: &RemoteError,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let cleared = super::super::clear_session_view_after_authorization_error(state, error);
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
    Ok(())
}
