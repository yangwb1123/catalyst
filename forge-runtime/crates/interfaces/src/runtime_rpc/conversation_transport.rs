use crate::runtime_domain::{
    Conversation, ConversationBootstrapEntry, ConversationBootstrapPage, ConversationChangePage,
    HubSnapshotAtCursor, OwnedConversationChangePage, OwnedConversationEntry,
    OwnedConversationImportResult, OwnedConversationPage, OwnedPromptAppendResult,
};

const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

pub(super) trait ConversationTimestampsJsonSafe {
    fn conversation_timestamps_json_safe(&self) -> bool;
}

pub(super) trait ConversationChangesJsonSafe {
    fn conversation_changes_json_safe(&self) -> bool;
}

fn change_numbers_json_safe(
    cursor: u64,
    schema_version: u16,
    aggregate_version: u64,
    created_at_ms: u64,
) -> bool {
    cursor <= MAX_SAFE_JSON_INTEGER
        && u64::from(schema_version) <= MAX_SAFE_JSON_INTEGER
        && aggregate_version <= MAX_SAFE_JSON_INTEGER
        && created_at_ms <= MAX_SAFE_JSON_INTEGER
}

impl ConversationChangesJsonSafe for ConversationChangePage {
    fn conversation_changes_json_safe(&self) -> bool {
        self.after_cursor <= MAX_SAFE_JSON_INTEGER
            && self.next_cursor <= MAX_SAFE_JSON_INTEGER
            && self.head_cursor <= MAX_SAFE_JSON_INTEGER
            && self.changes.iter().all(|change| {
                change_numbers_json_safe(
                    change.cursor,
                    change.schema_version,
                    change.aggregate_version,
                    change.created_at_ms,
                )
            })
    }
}

impl ConversationChangesJsonSafe for OwnedConversationChangePage {
    fn conversation_changes_json_safe(&self) -> bool {
        self.after_cursor <= MAX_SAFE_JSON_INTEGER
            && self.scanned_through_cursor <= MAX_SAFE_JSON_INTEGER
            && self.changes.iter().all(|change| {
                change_numbers_json_safe(
                    change.cursor,
                    change.schema_version,
                    change.aggregate_version,
                    change.created_at_ms,
                )
            })
    }
}

impl ConversationTimestampsJsonSafe for Conversation {
    fn conversation_timestamps_json_safe(&self) -> bool {
        self.created_at_ms <= self.updated_at_ms
            && self.created_at_ms <= MAX_SAFE_JSON_INTEGER
            && self.updated_at_ms <= MAX_SAFE_JSON_INTEGER
    }
}

impl ConversationTimestampsJsonSafe for OwnedConversationEntry {
    fn conversation_timestamps_json_safe(&self) -> bool {
        self.conversation.conversation_timestamps_json_safe()
            && aggregate_version_json_safe(self.aggregate_version)
    }
}

impl ConversationTimestampsJsonSafe for OwnedConversationPage {
    fn conversation_timestamps_json_safe(&self) -> bool {
        self.conversations
            .iter()
            .all(ConversationTimestampsJsonSafe::conversation_timestamps_json_safe)
    }
}

impl ConversationTimestampsJsonSafe for OwnedConversationImportResult {
    fn conversation_timestamps_json_safe(&self) -> bool {
        self.conversation.conversation_timestamps_json_safe()
            && aggregate_version_json_safe(self.aggregate_version)
    }
}

impl ConversationTimestampsJsonSafe for OwnedPromptAppendResult {
    fn conversation_timestamps_json_safe(&self) -> bool {
        self.prompt.created_at_ms <= MAX_SAFE_JSON_INTEGER
            && aggregate_version_json_safe(self.aggregate_version)
    }
}

impl ConversationTimestampsJsonSafe for ConversationBootstrapPage {
    fn conversation_timestamps_json_safe(&self) -> bool {
        self.conversations
            .iter()
            .all(ConversationTimestampsJsonSafe::conversation_timestamps_json_safe)
    }
}

impl ConversationTimestampsJsonSafe for ConversationBootstrapEntry {
    fn conversation_timestamps_json_safe(&self) -> bool {
        self.conversation.conversation_timestamps_json_safe()
            && self.creation_cursor <= MAX_SAFE_JSON_INTEGER
            && aggregate_version_json_safe(self.aggregate_version)
    }
}

fn aggregate_version_json_safe(value: u64) -> bool {
    value > 0 && value <= MAX_SAFE_JSON_INTEGER
}

impl ConversationTimestampsJsonSafe for HubSnapshotAtCursor {
    fn conversation_timestamps_json_safe(&self) -> bool {
        self.snapshot
            .conversations
            .iter()
            .all(ConversationTimestampsJsonSafe::conversation_timestamps_json_safe)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_domain::{ConversationChange, ConversationChangeKind, ConversationScope};

    fn conversation(timestamp: u64) -> Conversation {
        Conversation {
            id: "conversation-1".into(),
            scope: ConversationScope::Global,
            title: "Shared".into(),
            created_at_ms: timestamp,
            updated_at_ms: timestamp,
        }
    }

    #[test]
    fn conversation_timestamp_transport_accepts_ceiling_and_rejects_next_integer() {
        assert!(conversation(MAX_SAFE_JSON_INTEGER).conversation_timestamps_json_safe());
        assert!(!conversation(MAX_SAFE_JSON_INTEGER + 1).conversation_timestamps_json_safe());

        let chronologically_invalid = Conversation {
            id: "conversation-1".into(),
            scope: ConversationScope::Global,
            title: "Shared".into(),
            created_at_ms: 20,
            updated_at_ms: 10,
        };
        assert!(!chronologically_invalid.conversation_timestamps_json_safe());
    }

    #[test]
    fn change_transport_accepts_ceiling_and_rejects_next_integer() {
        let page = ConversationChangePage {
            after_cursor: MAX_SAFE_JSON_INTEGER - 1,
            next_cursor: MAX_SAFE_JSON_INTEGER,
            head_cursor: MAX_SAFE_JSON_INTEGER,
            has_more: false,
            changes: vec![ConversationChange {
                cursor: MAX_SAFE_JSON_INTEGER,
                schema_version: 1,
                conversation_id: "conversation-1".into(),
                entity_id: "prompt-1".into(),
                aggregate_version: MAX_SAFE_JSON_INTEGER,
                kind: ConversationChangeKind::PromptAppended,
                created_at_ms: MAX_SAFE_JSON_INTEGER,
            }],
        };
        assert!(page.conversation_changes_json_safe());
        let mut unsafe_page = page.clone();
        unsafe_page.changes[0].created_at_ms = MAX_SAFE_JSON_INTEGER + 1;
        assert!(!unsafe_page.conversation_changes_json_safe());

        let owned = OwnedConversationChangePage {
            after_cursor: MAX_SAFE_JSON_INTEGER - 1,
            scanned_through_cursor: MAX_SAFE_JSON_INTEGER,
            has_more: false,
            changes: page.changes,
        };
        assert!(owned.conversation_changes_json_safe());
        let mut unsafe_owned = owned;
        unsafe_owned.changes[0].aggregate_version = MAX_SAFE_JSON_INTEGER + 1;
        assert!(!unsafe_owned.conversation_changes_json_safe());
    }

    #[test]
    fn owned_aggregate_versions_accept_ceiling_and_reject_next_integer() {
        let conversation = conversation(1);
        let entry = OwnedConversationEntry {
            conversation: conversation.clone(),
            aggregate_version: MAX_SAFE_JSON_INTEGER,
        };
        assert!(entry.conversation_timestamps_json_safe());
        let mut unsafe_entry = entry.clone();
        unsafe_entry.aggregate_version = MAX_SAFE_JSON_INTEGER + 1;
        assert!(!unsafe_entry.conversation_timestamps_json_safe());

        let import = OwnedConversationImportResult {
            conversation,
            aggregate_version: MAX_SAFE_JSON_INTEGER,
            imported_prompt_count: 0,
            replayed: false,
        };
        assert!(import.conversation_timestamps_json_safe());
        let mut unsafe_import = import.clone();
        unsafe_import.aggregate_version = MAX_SAFE_JSON_INTEGER + 1;
        assert!(!unsafe_import.conversation_timestamps_json_safe());

        let append = OwnedPromptAppendResult {
            prompt: crate::runtime_domain::ConversationPrompt {
                id: "prompt-1".into(),
                conversation_id: "conversation-1".into(),
                role: "user".into(),
                content: "ship it".into(),
                created_at_ms: 1,
            },
            aggregate_version: MAX_SAFE_JSON_INTEGER,
            replayed: false,
        };
        assert!(append.conversation_timestamps_json_safe());
        let mut unsafe_append = append;
        unsafe_append.aggregate_version = MAX_SAFE_JSON_INTEGER + 1;
        assert!(!unsafe_append.conversation_timestamps_json_safe());
    }
}
