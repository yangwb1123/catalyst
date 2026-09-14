use serde::{Deserialize, Serialize};

use crate::runtime_domain::MAX_HUB_ENTITY_ID_BYTES;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OwnedConversationChangePage {
    pub(super) after_cursor: u64,
    pub(super) scanned_through_cursor: u64,
    pub(super) has_more: bool,
    pub(super) changes: Vec<OwnedConversationChange>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OwnedConversationChange {
    pub(super) cursor: u64,
    pub(super) schema_version: u16,
    pub(super) conversation_id: String,
    pub(super) entity_id: String,
    pub(super) aggregate_version: u64,
    pub(super) kind: String,
    pub(super) created_at_ms: u64,
}

impl OwnedConversationChangePage {
    pub(super) fn validate(&self, requested_after: u64, limit: usize) -> Result<(), String> {
        if self.after_cursor != requested_after
            || self.scanned_through_cursor < requested_after
            || self.scanned_through_cursor > i64::MAX as u64
            || limit == 0
            || self.changes.len() > limit
            || (self.has_more && self.changes.len() != limit)
            || (self.has_more && self.scanned_through_cursor == requested_after)
            || (self.changes.is_empty() && self.scanned_through_cursor != requested_after)
        {
            return Err("Forge API returned an invalid change page".into());
        }
        let mut previous = requested_after;
        for change in &self.changes {
            let Some(expected_cursor) = previous.checked_add(1) else {
                return Err("Forge API returned an invalid change cursor".into());
            };
            if change.cursor != expected_cursor
                || change.cursor > self.scanned_through_cursor
                || change.schema_version != 1
                || change.aggregate_version == 0
                || change.created_at_ms > i64::MAX as u64
                || !valid_id(&change.conversation_id)
                || !valid_id(&change.entity_id)
                || !matches!(
                    change.kind.as_str(),
                    "conversation_created" | "prompt_appended"
                )
                || (change.kind == "conversation_created"
                    && change.entity_id != change.conversation_id)
            {
                return Err("Forge API returned an invalid change row".into());
            }
            previous = change.cursor;
        }
        if self
            .changes
            .last()
            .map_or(requested_after, |change| change.cursor)
            != self.scanned_through_cursor
        {
            return Err("Forge API returned an invalid scanned cursor".into());
        }
        Ok(())
    }
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_HUB_ENTITY_ID_BYTES
        && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::OwnedConversationChangePage;

    #[test]
    fn change_page_validation_requires_dense_progress_and_full_has_more_pages() {
        assert!(page(4, 5, false, &[5]).validate(4, 2).is_ok());
        assert!(page(4, 4, false, &[]).validate(4, 2).is_ok());
        assert!(page(4, 7, false, &[7]).validate(4, 2).is_err());
        assert!(page(4, 4, false, &[5]).validate(4, 2).is_err());
        assert!(page(4, 5, true, &[5]).validate(4, 2).is_err());
        assert!(page(4, 4, true, &[]).validate(4, 2).is_err());
        assert!(page(4, 6, true, &[5, 6]).validate(4, 2).is_ok());
        assert!(page(4, 6, true, &[5, 6]).validate(4, 3).is_err());
    }

    fn page(
        after_cursor: u64,
        scanned_through_cursor: u64,
        has_more: bool,
        cursors: &[u64],
    ) -> OwnedConversationChangePage {
        serde_json::from_value(json!({
            "after_cursor": after_cursor,
            "scanned_through_cursor": scanned_through_cursor,
            "has_more": has_more,
            "changes": cursors.iter().copied().map(change).collect::<Vec<_>>(),
        }))
        .expect("deserialize change page fixture")
    }

    fn change(cursor: u64) -> serde_json::Value {
        json!({
            "cursor": cursor,
            "schema_version": 1,
            "conversation_id": format!("c-{cursor}"),
            "entity_id": format!("p-{cursor}"),
            "aggregate_version": 1,
            "kind": "prompt_appended",
            "created_at_ms": 1,
        })
    }
}
