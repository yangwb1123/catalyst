use std::time::Duration;

use serde::Serialize;

use super::super::changes::OwnedConversationChange;
use super::{RemoteClient, RemoteError};

const MAX_WATCH_POLLS: usize = 64;
const MAX_WATCH_DELAY_MS: u64 = 60_000;

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OwnedConversationChangeWatch {
    pub(super) start_cursor: u64,
    pub(super) scanned_through_cursor: u64,
    pub(super) polls: usize,
    pub(super) has_more: bool,
    pub(super) changes: Vec<OwnedConversationChange>,
}

impl RemoteClient {
    /// Polls the owner-bound change feed for a bounded window.
    ///
    /// Empty pages use exponential backoff; a page containing changes or a
    /// continuation page resets the delay so a catch-up does not wait. A
    /// saved cursor is committed after each valid advancing page, which lets
    /// an interrupted watch resume without replaying acknowledged changes.
    /// An explicit cursor remains one-off and never writes a checkpoint.
    pub(crate) async fn watch_conversation_changes(
        &self,
        explicit_after: Option<u64>,
        polls: usize,
        min_delay_ms: u64,
        max_delay_ms: u64,
    ) -> Result<serde_json::Value, RemoteError> {
        if polls == 0
            || polls > MAX_WATCH_POLLS
            || min_delay_ms > max_delay_ms
            || max_delay_ms > MAX_WATCH_DELAY_MS
        {
            return Err(RemoteError(
                "conversation change watch bounds are invalid".into(),
            ));
        }
        let start_cursor = explicit_after.map_or_else(|| self.saved_change_cursor(), Ok)?;
        let mut cursor = start_cursor;
        let mut delay_ms = min_delay_ms;
        let mut changes = Vec::new();
        let mut has_more = false;

        for poll in 0..polls {
            let page = self.conversation_changes_after(cursor).await?;
            let advanced = page.scanned_through_cursor > cursor;
            let page_has_more = page.has_more;
            let page_had_changes = !page.changes.is_empty();
            changes.extend(page.changes);
            cursor = page.scanned_through_cursor;
            has_more = page_has_more;

            if explicit_after.is_none() && advanced {
                self.persist_change_cursor(cursor)?;
            }

            if poll + 1 < polls {
                if page_has_more || page_had_changes {
                    delay_ms = min_delay_ms;
                } else {
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    delay_ms = delay_ms
                        .saturating_mul(2)
                        .min(max_delay_ms)
                        .max(min_delay_ms);
                }
            }
        }

        serde_json::to_value(OwnedConversationChangeWatch {
            start_cursor,
            scanned_through_cursor: cursor,
            polls,
            has_more,
            changes,
        })
        .map_err(|_| RemoteError("conversation change watch could not be encoded".into()))
    }
}
