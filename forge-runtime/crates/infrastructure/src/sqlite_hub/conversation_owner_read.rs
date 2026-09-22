use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use super::{
    ConversationOwner, HubEntity, HubStoreError, MAX_OWNED_CONVERSATION_PAGE_LIMIT,
    OwnedConversationEntry, OwnedConversationPage, read_error, rows,
};

pub(super) fn list_owned_conversations(
    connection: &mut Connection,
    owner: &ConversationOwner,
    after_id: Option<&str>,
    limit: usize,
) -> Result<OwnedConversationPage, HubStoreError> {
    let scan_limit = validated_scan_limit(after_id, limit)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let mut entries = fetch_owned_conversation_entries(&transaction, owner, after_id, scan_limit)?;
    let has_more = entries.len() > limit;
    if has_more {
        entries.truncate(limit);
    }
    let next_after_id = has_more
        .then(|| entries.last().map(|entry| entry.conversation.id.clone()))
        .flatten();
    transaction.commit().map_err(read_error)?;
    Ok(OwnedConversationPage {
        conversations: entries,
        next_after_id,
        has_more,
    })
}

pub(super) fn get_owned_conversation(
    connection: &mut Connection,
    owner: &ConversationOwner,
    conversation_id: &str,
) -> Result<OwnedConversationEntry, HubStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let row = transaction
        .query_row(
            "SELECT c.id, c.scope_kind, c.scope_id, c.title, c.created_at_ms,
                    c.updated_at_ms, h.last_version
             FROM conversation_owners AS o
             JOIN conversations AS c ON c.id = o.conversation_id
             JOIN conversation_change_heads AS h ON h.conversation_id = c.id
             WHERE c.id = ?1
               AND o.issuer = ?2 AND o.subject = ?3 AND o.tenant_id = ?4",
            params![
                conversation_id,
                owner.issuer,
                owner.subject,
                owner.tenant_id
            ],
            |row| {
                let conversation = rows::conversation(row)?;
                let version = row.get::<_, i64>(6)?;
                Ok((conversation, version))
            },
        )
        .optional()
        .map_err(read_error)?;
    let Some(row) = row else {
        return Err(HubStoreError::NotFound {
            entity: HubEntity::Conversation,
            id: conversation_id.to_owned(),
        });
    };
    let entry = decode_owned_conversation_entry(Ok(row))?;
    transaction.commit().map_err(read_error)?;
    Ok(entry)
}

fn validated_scan_limit(after_id: Option<&str>, limit: usize) -> Result<i64, HubStoreError> {
    if !(1..=MAX_OWNED_CONVERSATION_PAGE_LIMIT).contains(&limit) {
        return Err(HubStoreError::Conflict {
            entity: HubEntity::Conversation,
            message: "invalid owner Conversation page limit".into(),
        });
    }
    if after_id.is_some_and(|id| id.trim().is_empty() || id.len() > 128) {
        return Err(HubStoreError::Conflict {
            entity: HubEntity::Conversation,
            message: "invalid owner Conversation page cursor".into(),
        });
    }
    i64::try_from(limit.saturating_add(1)).map_err(|error| HubStoreError::Conflict {
        entity: HubEntity::Conversation,
        message: error.to_string(),
    })
}

fn fetch_owned_conversation_entries(
    transaction: &Transaction<'_>,
    owner: &ConversationOwner,
    after_id: Option<&str>,
    scan_limit: i64,
) -> Result<Vec<OwnedConversationEntry>, HubStoreError> {
    let mut statement = transaction
        .prepare(
            "SELECT c.id, c.scope_kind, c.scope_id, c.title, c.created_at_ms,
                    c.updated_at_ms, h.last_version
             FROM conversation_owners AS o
             JOIN conversations AS c ON c.id = o.conversation_id
             JOIN conversation_change_heads AS h ON h.conversation_id = c.id
             WHERE o.issuer = ?1 AND o.subject = ?2 AND o.tenant_id = ?3
               AND (?4 IS NULL OR c.id COLLATE BINARY > ?4 COLLATE BINARY)
             ORDER BY c.id COLLATE BINARY LIMIT ?5",
        )
        .map_err(read_error)?;
    let mapped = statement
        .query_map(
            params![
                owner.issuer,
                owner.subject,
                owner.tenant_id,
                after_id,
                scan_limit
            ],
            |row| {
                let conversation = rows::conversation(row)?;
                let version = row.get::<_, i64>(6)?;
                Ok((conversation, version))
            },
        )
        .map_err(read_error)?;
    mapped.map(decode_owned_conversation_entry).collect()
}

fn decode_owned_conversation_entry(
    row: rusqlite::Result<(super::Conversation, i64)>,
) -> Result<OwnedConversationEntry, HubStoreError> {
    let (conversation, version) = row.map_err(read_error)?;
    let aggregate_version = u64::try_from(version).map_err(|error| HubStoreError::Corrupt {
        message: format!("invalid Conversation aggregate version: {error}"),
    })?;
    Ok(OwnedConversationEntry {
        conversation,
        aggregate_version,
    })
}
