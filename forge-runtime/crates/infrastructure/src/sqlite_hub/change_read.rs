use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

use super::{
    ConversationChangeKind, ConversationChangePage, ConversationScope, HubEntity,
    HubSnapshotAtCursor, HubStoreError, MAX_CONVERSATION_CHANGE_PAGE_LIMIT, read_error, rows,
};

const CHANGE_JOURNAL_MISMATCH_SQL: &str = r"SELECT EXISTS(
  SELECT 1 FROM conversation_change_heads AS h
  LEFT JOIN (
    SELECT conversation_id, COUNT(*) AS event_count,
           COALESCE(MAX(aggregate_version), 0) AS max_version
    FROM conversation_changes GROUP BY conversation_id
  ) AS j ON j.conversation_id = h.conversation_id
  WHERE h.last_version != COALESCE(j.event_count, 0)
     OR h.last_version != COALESCE(j.max_version, 0)
) OR EXISTS(
  SELECT 1 FROM conversation_changes AS c
  LEFT JOIN conversation_change_heads AS h
    ON h.conversation_id = c.conversation_id
  WHERE h.conversation_id IS NULL
) OR EXISTS(
  SELECT 1 FROM conversation_change_baselines AS b
  LEFT JOIN conversation_change_heads AS h
    ON h.conversation_id = b.conversation_id
  WHERE h.conversation_id IS NULL
) OR EXISTS(
  SELECT 1 FROM conversation_change_heads AS h
  LEFT JOIN conversation_change_baselines AS b
    ON b.conversation_id = h.conversation_id
  WHERE h.last_version = 0 AND b.conversation_id IS NULL
) OR EXISTS(
  SELECT 1 FROM conversation_change_heads AS h
  JOIN conversation_changes AS c
    ON c.conversation_id = h.conversation_id AND c.aggregate_version = 1
  LEFT JOIN conversation_change_baselines AS b
    ON b.conversation_id = h.conversation_id
  WHERE h.last_version > 0
    AND ((b.conversation_id IS NOT NULL AND c.event_kind != 'prompt_appended')
      OR (b.conversation_id IS NULL AND c.event_kind != 'conversation_created'))
) OR EXISTS(
  SELECT 1 FROM conversations AS c
  LEFT JOIN conversation_change_heads AS h
    ON h.conversation_id = c.id
  WHERE h.conversation_id IS NULL AND (?1 IS NULL OR c.id != ?1)
) OR EXISTS(
  SELECT 1 FROM (
    SELECT conversation_id, aggregate_version,
           ROW_NUMBER() OVER (
             PARTITION BY conversation_id ORDER BY cursor
           ) AS expected_version
    FROM conversation_changes
  ) AS ordered
  WHERE ordered.aggregate_version != ordered.expected_version
)";
pub(super) fn snapshot_at_cursor(
    connection: &mut Connection,
) -> Result<HubSnapshotAtCursor, HubStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let snapshot = super::read::snapshot_locked(&transaction, &ConversationScope::Global)?;
    let cursor = change_head_cursor(&transaction)?;
    transaction.commit().map_err(read_error)?;
    Ok(HubSnapshotAtCursor { snapshot, cursor })
}

pub(super) fn conversation_changes_after(
    connection: &mut Connection,
    after_cursor: u64,
    limit: usize,
) -> Result<ConversationChangePage, HubStoreError> {
    let after_cursor_sql = cursor_sql(after_cursor)?;
    let limit_sql = page_limit_sql(limit)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let head_cursor = change_head_cursor(&transaction)?;
    validate_cursor_not_ahead(after_cursor, head_cursor)?;
    let changes = load_change_rows(&transaction, after_cursor_sql, limit_sql)?;
    validate_change_page(&transaction, after_cursor, limit, head_cursor, &changes)?;
    let next_cursor = changes.last().map_or(after_cursor, |change| change.cursor);
    let page = ConversationChangePage {
        after_cursor,
        next_cursor,
        head_cursor,
        has_more: next_cursor < head_cursor,
        changes,
    };
    transaction.commit().map_err(read_error)?;
    Ok(page)
}

fn cursor_sql(cursor: u64) -> Result<i64, HubStoreError> {
    i64::try_from(cursor).map_err(|error| HubStoreError::Conflict {
        entity: HubEntity::Conversation,
        message: format!("change cursor exceeds SQLite's signed integer range: {error}"),
    })
}

fn page_limit_sql(limit: usize) -> Result<i64, HubStoreError> {
    if !(1..=MAX_CONVERSATION_CHANGE_PAGE_LIMIT).contains(&limit) {
        return Err(HubStoreError::Conflict {
            entity: HubEntity::Conversation,
            message: format!(
                "change page limit must be between 1 and {MAX_CONVERSATION_CHANGE_PAGE_LIMIT}"
            ),
        });
    }
    i64::try_from(limit).map_err(|error| HubStoreError::Conflict {
        entity: HubEntity::Conversation,
        message: error.to_string(),
    })
}

fn validate_cursor_not_ahead(after_cursor: u64, head_cursor: u64) -> Result<(), HubStoreError> {
    if after_cursor > head_cursor {
        return Err(HubStoreError::Conflict {
            entity: HubEntity::Conversation,
            message: format!("change cursor {after_cursor} is beyond observed head {head_cursor}"),
        });
    }
    Ok(())
}

fn load_change_rows(
    transaction: &rusqlite::Transaction<'_>,
    after_cursor: i64,
    limit: i64,
) -> Result<Vec<super::ConversationChange>, HubStoreError> {
    let mut statement = transaction
        .prepare(
            "SELECT cursor, event_schema_version, conversation_id, entity_id,
                    aggregate_version, event_kind, created_at_ms
             FROM conversation_changes WHERE cursor > ?1 ORDER BY cursor LIMIT ?2",
        )
        .map_err(read_error)?;
    statement
        .query_map(
            rusqlite::params![after_cursor, limit],
            rows::conversation_change,
        )
        .map_err(read_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(read_error)
}

fn validate_change_page(
    transaction: &rusqlite::Transaction<'_>,
    after_cursor: u64,
    limit: usize,
    head_cursor: u64,
    changes: &[super::ConversationChange],
) -> Result<(), HubStoreError> {
    let mut expected_cursor =
        after_cursor
            .checked_add(1)
            .ok_or_else(|| HubStoreError::Corrupt {
                message: "change cursor overflowed while checking page continuity".into(),
            })?;
    for change in changes {
        if change.cursor != expected_cursor {
            return Err(HubStoreError::Corrupt {
                message: format!(
                    "Hub change cursor gap: expected {expected_cursor}, found {}",
                    change.cursor
                ),
            });
        }
        validate_conversation_change(transaction, change)?;
        expected_cursor = expected_cursor
            .checked_add(1)
            .ok_or_else(|| HubStoreError::Corrupt {
                message: "change cursor overflowed while checking page continuity".into(),
            })?;
    }
    let next_cursor = changes.last().map_or(after_cursor, |change| change.cursor);
    if changes.len() < limit && next_cursor != head_cursor {
        return Err(HubStoreError::Corrupt {
            message: format!(
                "Hub change page ended at {next_cursor} before observed head {head_cursor}"
            ),
        });
    }
    Ok(())
}

pub(super) fn change_head_cursor(connection: &Connection) -> Result<u64, HubStoreError> {
    let head: Option<i64> = connection
        .query_row(
            "SELECT last_cursor FROM conversation_change_state WHERE state_id = 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(read_error)?;
    let (event_count, max_cursor): (i64, i64) = connection
        .query_row(
            "SELECT COUNT(*), COALESCE(MAX(cursor), 0) FROM conversation_changes",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(read_error)?;
    let head = match head {
        Some(head) if event_count == head && max_cursor == head => head,
        _ => {
            return Err(HubStoreError::Corrupt {
                message: "Hub change head does not match journal rows".into(),
            });
        }
    };
    validate_all_change_heads(connection, None)?;
    u64::try_from(head).map_err(|error| HubStoreError::Corrupt {
        message: format!("invalid Hub change head cursor: {error}"),
    })
}

pub(super) fn validate_all_change_heads(
    connection: &Connection,
    pending_conversation_id: Option<&str>,
) -> Result<(), HubStoreError> {
    let has_mismatch: bool = connection
        .query_row(
            CHANGE_JOURNAL_MISMATCH_SQL,
            rusqlite::params![pending_conversation_id],
            |row| row.get(0),
        )
        .map_err(read_error)?;
    if has_mismatch {
        return Err(HubStoreError::Corrupt {
            message: "Conversation change heads do not match journal rows".into(),
        });
    }
    Ok(())
}

pub(super) fn validate_conversation_change(
    connection: &Connection,
    change: &super::ConversationChange,
) -> Result<(), HubStoreError> {
    validate_change_version(connection, change)?;
    if !change_entity_exists(connection, change)? {
        return Err(HubStoreError::Corrupt {
            message: format!(
                "Hub change at cursor {} does not reference its canonical entity",
                change.cursor
            ),
        });
    }
    Ok(())
}

fn validate_change_version(
    connection: &Connection,
    change: &super::ConversationChange,
) -> Result<(), HubStoreError> {
    let cursor = i64::try_from(change.cursor).map_err(|error| HubStoreError::Corrupt {
        message: format!("invalid Hub change cursor: {error}"),
    })?;
    let previous: Option<i64> = connection
        .query_row(
            "SELECT MAX(aggregate_version) FROM conversation_changes
             WHERE conversation_id = ?1 AND cursor < ?2",
            rusqlite::params![change.conversation_id, cursor],
            |row| row.get(0),
        )
        .map_err(read_error)?;
    if previous.is_none() && !change_origin_matches_baseline(connection, change)? {
        return Err(HubStoreError::Corrupt {
            message: format!(
                "invalid first Hub change origin for Conversation '{}'",
                change.conversation_id
            ),
        });
    }
    let expected_version =
        previous
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| HubStoreError::Corrupt {
                message: "Conversation change aggregate version overflowed".into(),
            })?;
    if i64::try_from(change.aggregate_version).ok() != Some(expected_version) {
        return Err(HubStoreError::Corrupt {
            message: format!(
                "invalid Conversation change version {} for '{}'",
                change.aggregate_version, change.conversation_id
            ),
        });
    }
    Ok(())
}

fn change_origin_matches_baseline(
    connection: &Connection,
    change: &super::ConversationChange,
) -> Result<bool, HubStoreError> {
    let legacy_baseline: bool = connection
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM conversation_change_baselines WHERE conversation_id = ?1
             )",
            [&change.conversation_id],
            |row| row.get(0),
        )
        .map_err(read_error)?;
    Ok(match change.kind {
        ConversationChangeKind::ConversationCreated => !legacy_baseline,
        ConversationChangeKind::PromptAppended => legacy_baseline,
    })
}

fn change_entity_exists(
    connection: &Connection,
    change: &super::ConversationChange,
) -> Result<bool, HubStoreError> {
    match change.kind {
        ConversationChangeKind::ConversationCreated => {
            if change.entity_id != change.conversation_id || change.aggregate_version != 1 {
                return Ok(false);
            }
            connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ?1)",
                    [&change.conversation_id],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(read_error)
        }
        ConversationChangeKind::PromptAppended => connection
            .query_row(
                "SELECT EXISTS(
                   SELECT 1 FROM prompts WHERE id = ?1 AND conversation_id = ?2
                 )",
                rusqlite::params![change.entity_id, change.conversation_id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(read_error),
    }
}
