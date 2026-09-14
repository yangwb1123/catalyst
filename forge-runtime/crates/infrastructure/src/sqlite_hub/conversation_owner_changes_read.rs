use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};

use super::{
    ConversationChange, ConversationOwner, HubEntity, HubStoreError,
    MAX_CONVERSATION_CHANGE_PAGE_LIMIT, OwnedConversationChangePage, change_read, read_error, rows,
};

pub(super) fn owned_conversation_changes_after(
    connection: &mut Connection,
    owner: &ConversationOwner,
    after_cursor: u64,
    limit: usize,
) -> Result<OwnedConversationChangePage, HubStoreError> {
    let after_cursor_sql = cursor_sql(after_cursor)?;
    let scan_limit = scan_limit_sql(limit)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let owner_head = owner_change_head(&transaction, owner)?;
    reject_cursor_ahead(after_cursor, owner_head)?;
    let mut rows = load_owned_changes(&transaction, owner, after_cursor_sql, scan_limit)?;
    validate_and_project_changes(
        &transaction,
        after_cursor,
        owner_head,
        scan_limit,
        &mut rows,
    )?;

    let has_more = rows.len() > limit;
    if has_more {
        rows.truncate(limit);
    }
    let scanned_through_cursor = rows
        .last()
        .map_or(after_cursor, |change| change.owner_cursor);
    let changes = rows.into_iter().map(|change| change.change).collect();
    let page = OwnedConversationChangePage {
        after_cursor,
        scanned_through_cursor,
        has_more,
        changes,
    };
    transaction.commit().map_err(read_error)?;
    Ok(page)
}

fn cursor_sql(cursor: u64) -> Result<i64, HubStoreError> {
    i64::try_from(cursor).map_err(|error| HubStoreError::Conflict {
        entity: HubEntity::Conversation,
        message: format!("owner change cursor exceeds SQLite's signed integer range: {error}"),
    })
}

fn scan_limit_sql(limit: usize) -> Result<i64, HubStoreError> {
    if !(1..=MAX_CONVERSATION_CHANGE_PAGE_LIMIT).contains(&limit) {
        return Err(HubStoreError::Conflict {
            entity: HubEntity::Conversation,
            message: format!(
                "owner change page limit must be between 1 and {MAX_CONVERSATION_CHANGE_PAGE_LIMIT}"
            ),
        });
    }
    i64::try_from(limit.saturating_add(1)).map_err(|error| HubStoreError::Conflict {
        entity: HubEntity::Conversation,
        message: error.to_string(),
    })
}

fn owner_change_head(
    transaction: &Transaction<'_>,
    owner: &ConversationOwner,
) -> Result<u64, HubStoreError> {
    let stored_head: Option<i64> = transaction
        .query_row(
            "SELECT last_cursor FROM conversation_owner_change_heads
             WHERE issuer COLLATE BINARY = ?1 COLLATE BINARY
               AND subject COLLATE BINARY = ?2 COLLATE BINARY
               AND tenant_id COLLATE BINARY = ?3 COLLATE BINARY",
            params![owner.issuer, owner.subject, owner.tenant_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(read_error)?;
    let head = stored_head.unwrap_or(0);
    if head < 0 {
        return Err(HubStoreError::Corrupt {
            message: "owner change head cursor is negative".into(),
        });
    }
    u64::try_from(head).map_err(|error| HubStoreError::Corrupt {
        message: format!("invalid owner change head cursor: {error}"),
    })
}

fn reject_cursor_ahead(after_cursor: u64, owner_head: u64) -> Result<(), HubStoreError> {
    if after_cursor > owner_head {
        return Err(HubStoreError::Conflict {
            entity: HubEntity::Conversation,
            message: "owner change cursor is beyond the current owner position".into(),
        });
    }
    Ok(())
}

struct StoredOwnerChange {
    hub_cursor: u64,
    owner_cursor: u64,
    change: ConversationChange,
}

fn load_owned_changes(
    transaction: &Transaction<'_>,
    owner: &ConversationOwner,
    after_cursor: i64,
    scan_limit: i64,
) -> Result<Vec<StoredOwnerChange>, HubStoreError> {
    let mut statement = transaction
        .prepare(
            "SELECT c.cursor, c.event_schema_version, c.conversation_id, c.entity_id,
                    c.aggregate_version, c.event_kind, c.created_at_ms, r.owner_cursor
             FROM conversation_owner_change_rows AS r
             JOIN conversation_owners AS o
               ON o.conversation_id = r.conversation_id
              AND o.issuer = r.issuer AND o.subject = r.subject AND o.tenant_id = r.tenant_id
             JOIN conversation_changes AS c
               ON c.conversation_id = r.conversation_id AND c.cursor = r.hub_cursor
             WHERE r.issuer COLLATE BINARY = ?1 COLLATE BINARY
               AND r.subject COLLATE BINARY = ?2 COLLATE BINARY
               AND r.tenant_id COLLATE BINARY = ?3 COLLATE BINARY
               AND r.owner_cursor > ?4
             ORDER BY r.owner_cursor LIMIT ?5",
        )
        .map_err(read_error)?;
    statement
        .query_map(
            params![
                owner.issuer,
                owner.subject,
                owner.tenant_id,
                after_cursor,
                scan_limit
            ],
            stored_owner_change,
        )
        .map_err(read_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(read_error)
}

fn stored_owner_change(row: &Row<'_>) -> rusqlite::Result<StoredOwnerChange> {
    let change = rows::conversation_change(row)?;
    let hub_cursor = change.cursor;
    let local_cursor = row.get::<_, i64>(7)?;
    let owner_cursor = u64::try_from(local_cursor).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            7,
            rusqlite::types::Type::Integer,
            Box::new(error),
        )
    })?;
    Ok(StoredOwnerChange {
        hub_cursor,
        owner_cursor,
        change,
    })
}

fn validate_and_project_changes(
    transaction: &Transaction<'_>,
    after_cursor: u64,
    owner_head: u64,
    scan_limit: i64,
    changes: &mut [StoredOwnerChange],
) -> Result<(), HubStoreError> {
    let scan_limit = u64::try_from(scan_limit).map_err(|error| HubStoreError::Corrupt {
        message: format!("invalid owner change scan limit: {error}"),
    })?;
    let expected_count = usize::try_from(owner_head.saturating_sub(after_cursor).min(scan_limit))
        .map_err(|error| HubStoreError::Corrupt {
        message: format!("owner change page size is out of range: {error}"),
    })?;
    if changes.len() != expected_count {
        return Err(HubStoreError::Corrupt {
            message: "owner change head does not match its bounded page rows".into(),
        });
    }
    let mut expected_cursor = after_cursor;
    for stored in changes {
        expected_cursor = expected_cursor
            .checked_add(1)
            .ok_or_else(|| HubStoreError::Corrupt {
                message: "owner change cursor overflowed while checking page continuity".into(),
            })?;
        if stored.owner_cursor != expected_cursor {
            return Err(HubStoreError::Corrupt {
                message: "owner change rows are not densely cursor ordered".into(),
            });
        }
        stored.change.cursor = stored.hub_cursor;
        change_read::validate_conversation_change(transaction, &stored.change)?;
        stored.change.cursor = stored.owner_cursor;
    }
    Ok(())
}
