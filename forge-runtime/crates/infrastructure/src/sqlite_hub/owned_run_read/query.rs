use super::{HubEntity, HubStoreError};

pub(super) const RUN_SUMMARY_QUERY: &str = "SELECT r.id, r.prompt_id, r.created_at_ms,
       COALESCE((SELECT MAX(e.seq) FROM run_events AS e
                 WHERE e.run_id = r.id), 0),
       COALESCE((SELECT length(CAST(e.event_json AS BLOB))
                 FROM run_events AS e WHERE e.run_id = r.id
                 ORDER BY e.seq DESC LIMIT 1), 0)
FROM conversation_owners AS o
JOIN conversations AS c ON c.id = o.conversation_id
JOIN runs AS r ON r.conversation_id = c.id
WHERE o.issuer = ?1 AND o.subject = ?2 AND o.tenant_id = ?3
  AND c.id = ?4
  AND (?5 IS NULL OR r.created_at_ms < ?5
       OR (r.created_at_ms = ?5
           AND r.id COLLATE BINARY < ?6 COLLATE BINARY))
ORDER BY r.created_at_ms DESC, r.id COLLATE BINARY DESC
LIMIT ?7";

pub(super) fn not_found_conversation(conversation_id: &str) -> HubStoreError {
    HubStoreError::NotFound {
        entity: HubEntity::Conversation,
        id: conversation_id.to_owned(),
    }
}

pub(super) fn invalid_page(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::Conversation,
        message: message.to_owned(),
    }
}

pub(super) fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.to_owned(),
    }
}
