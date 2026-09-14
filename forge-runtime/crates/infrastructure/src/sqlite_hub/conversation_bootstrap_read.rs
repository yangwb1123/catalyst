use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior};

use super::{
    Conversation, ConversationBootstrapCursor, ConversationBootstrapEntry,
    ConversationBootstrapPage, ConversationBootstrapPhase, ConversationChangeKind, HubEntity,
    HubStoreError, MAX_CONVERSATION_BOOTSTRAP_PAGE_LIMIT, change_read, read_error, rows,
};

pub(super) fn conversation_bootstrap_page(
    connection: &mut Connection,
    cursor: Option<&ConversationBootstrapCursor>,
    limit: usize,
) -> Result<ConversationBootstrapPage, HubStoreError> {
    let limit_sql = page_limit_sql(limit)?;
    let (transaction, snapshot_cursor) = begin_bootstrap_transaction(connection, cursor)?;
    let (conversations, scanned_through_cursor, next_cursor, has_more) =
        read_bootstrap_page(&transaction, cursor, snapshot_cursor, limit, limit_sql)?;
    transaction.commit().map_err(read_error)?;
    Ok(ConversationBootstrapPage {
        snapshot_cursor,
        conversations,
        scanned_through_cursor,
        next_cursor,
        has_more,
    })
}

fn begin_bootstrap_transaction<'a>(
    connection: &'a mut Connection,
    cursor: Option<&ConversationBootstrapCursor>,
) -> Result<(Transaction<'a>, u64), HubStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let current_head = change_read::change_head_cursor(&transaction)?;
    let snapshot_cursor = cursor.map_or(current_head, |value| value.snapshot_cursor);
    if snapshot_cursor > current_head {
        return Err(HubStoreError::Conflict {
            entity: HubEntity::Conversation,
            message: format!(
                "bootstrap cursor head {snapshot_cursor} is beyond observed head {current_head}"
            ),
        });
    }
    Ok((transaction, snapshot_cursor))
}

fn read_bootstrap_page(
    transaction: &Transaction<'_>,
    cursor: Option<&ConversationBootstrapCursor>,
    snapshot_cursor: u64,
    limit: usize,
    limit_sql: i64,
) -> Result<BootstrapPageParts, HubStoreError> {
    match cursor {
        None => read_baseline_page(transaction, snapshot_cursor, "", limit, limit_sql),
        Some(value) if value.phase == ConversationBootstrapPhase::LegacyBaseline => {
            read_baseline_page(
                transaction,
                snapshot_cursor,
                value
                    .after_conversation_id
                    .as_deref()
                    .ok_or_else(invalid_cursor)?,
                limit,
                limit_sql,
            )
        }
        Some(value) => read_change_page(
            transaction,
            snapshot_cursor,
            value.after_change_cursor.ok_or_else(invalid_cursor)?,
            limit,
            limit_sql,
        ),
    }
}

type BootstrapPageParts = (
    Vec<ConversationBootstrapEntry>,
    u64,
    Option<ConversationBootstrapCursor>,
    bool,
);

fn read_baseline_page(
    transaction: &Transaction<'_>,
    snapshot_cursor: u64,
    after_id: &str,
    limit: usize,
    limit_sql: i64,
) -> Result<BootstrapPageParts, HubStoreError> {
    let mut entries = read_baseline_entries(transaction, after_id, limit_sql)?;
    let has_extra = entries.len() > limit;
    if has_extra {
        entries.truncate(limit);
        return baseline_continuation(entries, snapshot_cursor);
    }
    Ok(exhausted_baseline_page(entries, snapshot_cursor))
}

type BaselineRow = (Conversation, Option<i64>);

fn read_baseline_entries(
    transaction: &Transaction<'_>,
    after_id: &str,
    limit_sql: i64,
) -> Result<Vec<ConversationBootstrapEntry>, HubStoreError> {
    let rows = read_baseline_rows(transaction, after_id, limit_sql)?;
    rows.into_iter()
        .map(|(conversation, version)| project_baseline_row(conversation, version))
        .collect()
}

fn read_baseline_rows(
    transaction: &Transaction<'_>,
    after_id: &str,
    limit_sql: i64,
) -> Result<Vec<BaselineRow>, HubStoreError> {
    let mut statement = transaction
        .prepare(
            "SELECT c.id, c.scope_kind, c.scope_id, c.title, c.created_at_ms,
                    c.updated_at_ms, h.last_version
             FROM conversation_change_baselines AS b
             LEFT JOIN conversations AS c ON c.id = b.conversation_id
             LEFT JOIN conversation_change_heads AS h ON h.conversation_id = c.id
             WHERE b.conversation_id COLLATE BINARY > ?1 COLLATE BINARY
             ORDER BY b.conversation_id COLLATE BINARY LIMIT ?2",
        )
        .map_err(read_error)?;
    statement
        .query_map(rusqlite::params![after_id, limit_sql + 1], |row| {
            let conversation = rows::conversation(row)?;
            let version = row.get(6)?;
            Ok((conversation, version))
        })
        .map_err(read_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(read_error)
}

fn project_baseline_row(
    conversation: Conversation,
    version: Option<i64>,
) -> Result<ConversationBootstrapEntry, HubStoreError> {
    let aggregate_version = version.ok_or_else(|| HubStoreError::Corrupt {
        message: format!(
            "legacy Conversation '{}' has no aggregate head",
            conversation.id
        ),
    })?;
    let aggregate_version =
        u64::try_from(aggregate_version).map_err(|error| HubStoreError::Corrupt {
            message: format!("invalid Conversation aggregate head: {error}"),
        })?;
    Ok(ConversationBootstrapEntry {
        conversation,
        creation_cursor: 0,
        aggregate_version,
    })
}

fn baseline_continuation(
    entries: Vec<ConversationBootstrapEntry>,
    snapshot_cursor: u64,
) -> Result<BootstrapPageParts, HubStoreError> {
    let after_conversation_id = entries
        .last()
        .map(|entry| entry.conversation.id.clone())
        .ok_or_else(|| HubStoreError::Corrupt {
            message: "bootstrap baseline page overflow had no emitted row".into(),
        })?;
    Ok((
        entries,
        0,
        Some(ConversationBootstrapCursor {
            snapshot_cursor,
            phase: ConversationBootstrapPhase::LegacyBaseline,
            after_conversation_id: Some(after_conversation_id),
            after_change_cursor: None,
        }),
        true,
    ))
}

fn exhausted_baseline_page(
    entries: Vec<ConversationBootstrapEntry>,
    snapshot_cursor: u64,
) -> BootstrapPageParts {
    if snapshot_cursor == 0 {
        return (entries, 0, None, false);
    }
    (
        entries,
        0,
        Some(ConversationBootstrapCursor {
            snapshot_cursor,
            phase: ConversationBootstrapPhase::ChangeLog,
            after_conversation_id: None,
            after_change_cursor: Some(0),
        }),
        true,
    )
}

fn read_change_page(
    transaction: &Transaction<'_>,
    snapshot_cursor: u64,
    after_cursor: u64,
    limit: usize,
    limit_sql: i64,
) -> Result<BootstrapPageParts, HubStoreError> {
    let changes = read_change_rows(transaction, snapshot_cursor, after_cursor, limit_sql)?;
    let has_extra = changes.len() > limit;
    let mut changes = changes;
    if has_extra {
        changes.truncate(limit);
    }
    let entries = project_change_entries(transaction, &changes, after_cursor)?;
    finish_change_page(&changes, entries, has_extra, snapshot_cursor, after_cursor)
}

fn read_change_rows(
    transaction: &Transaction<'_>,
    snapshot_cursor: u64,
    after_cursor: u64,
    limit_sql: i64,
) -> Result<Vec<super::ConversationChange>, HubStoreError> {
    let after_sql = cursor_sql(after_cursor)?;
    let head_sql = cursor_sql(snapshot_cursor)?;
    let mut statement = transaction
        .prepare(
            "SELECT cursor, event_schema_version, conversation_id, entity_id,
                    aggregate_version, event_kind, created_at_ms
             FROM conversation_changes
             WHERE cursor > ?1 AND cursor <= ?2
             ORDER BY cursor LIMIT ?3",
        )
        .map_err(read_error)?;
    statement
        .query_map(
            rusqlite::params![after_sql, head_sql, limit_sql + 1],
            rows::conversation_change,
        )
        .map_err(read_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(read_error)
}

fn project_change_entries(
    transaction: &Transaction<'_>,
    changes: &[super::ConversationChange],
    after_cursor: u64,
) -> Result<Vec<ConversationBootstrapEntry>, HubStoreError> {
    let mut expected_cursor =
        after_cursor
            .checked_add(1)
            .ok_or_else(|| HubStoreError::Corrupt {
                message: "bootstrap change cursor overflowed".into(),
            })?;
    let mut entries = Vec::new();
    for change in changes {
        if change.cursor != expected_cursor {
            return Err(HubStoreError::Corrupt {
                message: format!(
                    "Hub change cursor gap during bootstrap: expected {expected_cursor}, found {}",
                    change.cursor
                ),
            });
        }
        change_read::validate_conversation_change(transaction, change)?;
        if change.kind == ConversationChangeKind::ConversationCreated {
            let (conversation, aggregate_version) =
                load_created_conversation(transaction, &change.conversation_id)?;
            entries.push(ConversationBootstrapEntry {
                conversation,
                creation_cursor: change.cursor,
                aggregate_version,
            });
        }
        expected_cursor = expected_cursor
            .checked_add(1)
            .ok_or_else(|| HubStoreError::Corrupt {
                message: "bootstrap change cursor overflowed".into(),
            })?;
    }

    Ok(entries)
}

fn finish_change_page(
    changes: &[super::ConversationChange],
    entries: Vec<ConversationBootstrapEntry>,
    has_extra: bool,
    snapshot_cursor: u64,
    after_cursor: u64,
) -> Result<BootstrapPageParts, HubStoreError> {
    let next_position = changes.last().map_or(after_cursor, |change| change.cursor);
    if !has_extra && next_position != snapshot_cursor {
        return Err(HubStoreError::Corrupt {
            message: format!(
                "Hub bootstrap change page ended at {next_position} before frozen head {snapshot_cursor}"
            ),
        });
    }
    let has_more = has_extra || next_position < snapshot_cursor;
    let next_cursor = has_more.then_some(ConversationBootstrapCursor {
        snapshot_cursor,
        phase: ConversationBootstrapPhase::ChangeLog,
        after_conversation_id: None,
        after_change_cursor: Some(next_position),
    });
    Ok((entries, next_position, next_cursor, has_more))
}

fn load_created_conversation(
    transaction: &Transaction<'_>,
    conversation_id: &str,
) -> Result<(Conversation, u64), HubStoreError> {
    let row: Option<(Conversation, Option<i64>)> = transaction
        .query_row(
            "SELECT c.id, c.scope_kind, c.scope_id, c.title, c.created_at_ms,
                    c.updated_at_ms, h.last_version
             FROM conversations AS c
             LEFT JOIN conversation_change_heads AS h ON h.conversation_id = c.id
             WHERE c.id = ?1",
            [conversation_id],
            |row| Ok((rows::conversation(row)?, row.get(6)?)),
        )
        .optional()
        .map_err(read_error)?;
    let (conversation, aggregate_version) = row.ok_or_else(|| HubStoreError::Corrupt {
        message: format!(
            "Conversation-created event references missing Conversation '{conversation_id}'"
        ),
    })?;
    let aggregate_version =
        u64::try_from(aggregate_version.ok_or_else(|| HubStoreError::Corrupt {
            message: format!("Conversation '{conversation_id}' has no aggregate head"),
        })?)
        .map_err(|error| HubStoreError::Corrupt {
            message: format!("invalid Conversation aggregate head: {error}"),
        })?;
    Ok((conversation, aggregate_version))
}

fn cursor_sql(cursor: u64) -> Result<i64, HubStoreError> {
    i64::try_from(cursor).map_err(|error| HubStoreError::Conflict {
        entity: HubEntity::Conversation,
        message: format!("bootstrap cursor exceeds SQLite's signed integer range: {error}"),
    })
}

fn page_limit_sql(limit: usize) -> Result<i64, HubStoreError> {
    if !(1..=MAX_CONVERSATION_BOOTSTRAP_PAGE_LIMIT).contains(&limit) {
        return Err(HubStoreError::Conflict {
            entity: HubEntity::Conversation,
            message: format!(
                "bootstrap page limit must be between 1 and {MAX_CONVERSATION_BOOTSTRAP_PAGE_LIMIT}"
            ),
        });
    }
    i64::try_from(limit).map_err(|error| HubStoreError::Conflict {
        entity: HubEntity::Conversation,
        message: error.to_string(),
    })
}

fn invalid_cursor() -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::Conversation,
        message: "invalid Conversation bootstrap cursor shape".into(),
    }
}
