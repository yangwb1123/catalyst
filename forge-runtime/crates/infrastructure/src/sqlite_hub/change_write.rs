use rusqlite::{OptionalExtension, Transaction};

use super::write;
use super::{HubEntity, HubStoreError, read_error, write_error};

pub(super) fn append_conversation_change(
    transaction: &Transaction<'_>,
    event_kind: &str,
    conversation_id: &str,
    entity_id: &str,
    created_at_ms: u64,
) -> Result<(), HubStoreError> {
    super::change_read::validate_all_change_heads(
        transaction,
        (event_kind == "conversation_created").then_some(conversation_id),
    )?;
    let previous_version = previous_change_version(transaction, conversation_id)?;
    validate_change_start(transaction, event_kind, previous_version, conversation_id)?;
    let previous_cursor = current_change_cursor(transaction)?;
    let cursor = previous_cursor
        .checked_add(1)
        .ok_or_else(|| HubStoreError::Corrupt {
            message: "Hub change cursor overflowed".into(),
        })?;
    let aggregate_version =
        previous_version
            .checked_add(1)
            .ok_or_else(|| HubStoreError::Corrupt {
                message: "Conversation change aggregate version overflowed".into(),
            })?;
    insert_change_record(
        transaction,
        cursor,
        conversation_id,
        entity_id,
        aggregate_version,
        event_kind,
        created_at_ms,
    )?;
    advance_change_heads(
        transaction,
        conversation_id,
        previous_version,
        aggregate_version,
        previous_cursor,
        cursor,
    )?;
    append_owner_change_cursor(transaction, conversation_id, cursor)?;
    Ok(())
}

fn append_owner_change_cursor(
    transaction: &Transaction<'_>,
    conversation_id: &str,
    hub_cursor: i64,
) -> Result<(), HubStoreError> {
    let owner = conversation_owner(transaction, conversation_id)?;
    let Some((issuer, subject, tenant_id)) = owner else {
        return Ok(());
    };
    ensure_owner_change_head(transaction, &issuer, &subject, &tenant_id)?;
    let previous_cursor = owner_change_head(transaction, &issuer, &subject, &tenant_id)?;
    let owner_cursor = previous_cursor
        .checked_add(1)
        .ok_or_else(|| HubStoreError::Corrupt {
            message: "owner-local Conversation change cursor overflowed".into(),
        })?;
    insert_owner_change_row(
        transaction,
        &issuer,
        &subject,
        &tenant_id,
        owner_cursor,
        conversation_id,
        hub_cursor,
    )?;
    advance_owner_change_head(
        transaction,
        &issuer,
        &subject,
        &tenant_id,
        previous_cursor,
        owner_cursor,
    )
}

fn conversation_owner(
    transaction: &Transaction<'_>,
    conversation_id: &str,
) -> Result<Option<(String, String, String)>, HubStoreError> {
    transaction
        .query_row(
            "SELECT issuer, subject, tenant_id FROM conversation_owners
             WHERE conversation_id = ?1",
            [conversation_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(read_error)
}

fn ensure_owner_change_head(
    transaction: &Transaction<'_>,
    issuer: &str,
    subject: &str,
    tenant_id: &str,
) -> Result<(), HubStoreError> {
    transaction
        .execute(
            "INSERT INTO conversation_owner_change_heads(
               issuer, subject, tenant_id, last_cursor
             ) VALUES(?1, ?2, ?3, 0) ON CONFLICT DO NOTHING",
            rusqlite::params![issuer, subject, tenant_id],
        )
        .map_err(read_error)?;
    Ok(())
}

fn insert_owner_change_row(
    transaction: &Transaction<'_>,
    issuer: &str,
    subject: &str,
    tenant_id: &str,
    owner_cursor: i64,
    conversation_id: &str,
    hub_cursor: i64,
) -> Result<(), HubStoreError> {
    transaction
        .execute(
            "INSERT INTO conversation_owner_change_rows(
               issuer, subject, tenant_id, owner_cursor, conversation_id, hub_cursor
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                issuer,
                subject,
                tenant_id,
                owner_cursor,
                conversation_id,
                hub_cursor
            ],
        )
        .map_err(|error| write_error(HubEntity::Conversation, error))?;
    Ok(())
}

fn advance_owner_change_head(
    transaction: &Transaction<'_>,
    issuer: &str,
    subject: &str,
    tenant_id: &str,
    previous_cursor: i64,
    owner_cursor: i64,
) -> Result<(), HubStoreError> {
    let advanced = transaction
        .execute(
            "UPDATE conversation_owner_change_heads SET last_cursor = ?1
             WHERE issuer = ?2 AND subject = ?3 AND tenant_id = ?4 AND last_cursor = ?5",
            rusqlite::params![owner_cursor, issuer, subject, tenant_id, previous_cursor],
        )
        .map_err(read_error)?;
    if advanced != 1 {
        return Err(HubStoreError::Corrupt {
            message: "owner-local Conversation change head could not advance atomically".into(),
        });
    }
    Ok(())
}

fn owner_change_head(
    transaction: &Transaction<'_>,
    issuer: &str,
    subject: &str,
    tenant_id: &str,
) -> Result<i64, HubStoreError> {
    transaction
        .query_row(
            "SELECT last_cursor FROM conversation_owner_change_heads
             WHERE issuer = ?1 AND subject = ?2 AND tenant_id = ?3",
            rusqlite::params![issuer, subject, tenant_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(read_error)?
        .filter(|head| *head >= 0)
        .ok_or_else(|| HubStoreError::Corrupt {
            message: "owner-local Conversation change head is missing or negative".into(),
        })
}

fn insert_change_record(
    transaction: &Transaction<'_>,
    cursor: i64,
    conversation_id: &str,
    entity_id: &str,
    aggregate_version: i64,
    event_kind: &str,
    created_at_ms: u64,
) -> Result<(), HubStoreError> {
    let created_at_ms = write::to_i64(created_at_ms)?;
    transaction
        .execute(
            "INSERT INTO conversation_changes(
               cursor, conversation_id, entity_id, aggregate_version,
               event_kind, event_schema_version, created_at_ms
             ) VALUES(?1,?2,?3,?4,?5,1,?6)",
            rusqlite::params![
                cursor,
                conversation_id,
                entity_id,
                aggregate_version,
                event_kind,
                created_at_ms
            ],
        )
        .map_err(|error| write_error(HubEntity::Conversation, error))?;
    Ok(())
}

fn previous_change_version(
    transaction: &Transaction<'_>,
    conversation_id: &str,
) -> Result<i64, HubStoreError> {
    let last_version: Option<i64> = transaction
        .query_row(
            "SELECT last_version FROM conversation_change_heads WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(read_error)?;
    let (event_count, max_version): (i64, i64) = transaction
        .query_row(
            "SELECT COUNT(*), COALESCE(MAX(aggregate_version), 0)
             FROM conversation_changes WHERE conversation_id = ?1",
            [conversation_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(read_error)?;
    match last_version {
        Some(head) if event_count == head && max_version == head => Ok(head),
        None if event_count == 0
            && max_version == 0
            && !has_legacy_baseline(transaction, conversation_id)? =>
        {
            Ok(0)
        }
        _ => Err(HubStoreError::Corrupt {
            message: format!(
                "Conversation change head does not match journal rows for '{conversation_id}'"
            ),
        }),
    }
}

fn validate_change_start(
    transaction: &Transaction<'_>,
    event_kind: &str,
    previous_version: i64,
    conversation_id: &str,
) -> Result<(), HubStoreError> {
    let aggregate_version =
        previous_version
            .checked_add(1)
            .ok_or_else(|| HubStoreError::Corrupt {
                message: "Conversation change aggregate version overflowed".into(),
            })?;
    let legacy_baseline = has_legacy_baseline(transaction, conversation_id)?;
    let valid_start = match event_kind {
        "conversation_created" => {
            previous_version == 0 && aggregate_version == 1 && !legacy_baseline
        }
        // A v29 Conversation has no backfilled creation event. Its first
        // post-migration journal entry therefore begins at version one.
        "prompt_appended" => prompt_origin_is_valid(
            transaction,
            conversation_id,
            previous_version,
            legacy_baseline,
        )?,
        _ => false,
    };
    if !valid_start {
        return Err(HubStoreError::Corrupt {
            message: format!("invalid Conversation change sequence for '{conversation_id}'"),
        });
    }
    Ok(())
}

fn prompt_origin_is_valid(
    transaction: &Transaction<'_>,
    conversation_id: &str,
    previous_version: i64,
    legacy_baseline: bool,
) -> Result<bool, HubStoreError> {
    let has_creation_event: bool = transaction
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM conversation_changes
               WHERE conversation_id = ?1 AND aggregate_version = 1
                 AND event_kind = 'conversation_created'
             )",
            [conversation_id],
            |row| row.get(0),
        )
        .map_err(read_error)?;
    Ok(if legacy_baseline {
        !has_creation_event
    } else {
        previous_version >= 1 && has_creation_event
    })
}

fn has_legacy_baseline(
    transaction: &Transaction<'_>,
    conversation_id: &str,
) -> Result<bool, HubStoreError> {
    transaction
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM conversation_change_baselines WHERE conversation_id = ?1
             )",
            [conversation_id],
            |row| row.get(0),
        )
        .map_err(read_error)
}

fn current_change_cursor(transaction: &Transaction<'_>) -> Result<i64, HubStoreError> {
    let last_cursor: Option<i64> = transaction
        .query_row(
            "SELECT last_cursor FROM conversation_change_state WHERE state_id = 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(read_error)?;
    let (event_count, max_cursor): (i64, i64) = transaction
        .query_row(
            "SELECT COUNT(*), COALESCE(MAX(cursor), 0) FROM conversation_changes",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(read_error)?;
    match last_cursor {
        Some(head) if event_count == head && max_cursor == head => Ok(head),
        _ => Err(HubStoreError::Corrupt {
            message: "Hub change head does not match journal rows".into(),
        }),
    }
}

fn advance_change_heads(
    transaction: &Transaction<'_>,
    conversation_id: &str,
    previous_version: i64,
    aggregate_version: i64,
    previous_cursor: i64,
    cursor: i64,
) -> Result<(), HubStoreError> {
    let cursor_rows = transaction
        .execute(
            "UPDATE conversation_change_state SET last_cursor = ?1
             WHERE state_id = 1 AND last_cursor = ?2",
            rusqlite::params![cursor, previous_cursor],
        )
        .map_err(read_error)?;
    let version_rows = transaction
        .execute(
            "INSERT INTO conversation_change_heads(conversation_id, last_version)
             VALUES(?1, ?2)
             ON CONFLICT(conversation_id) DO UPDATE
               SET last_version = excluded.last_version
               WHERE conversation_change_heads.last_version = ?3",
            rusqlite::params![conversation_id, aggregate_version, previous_version],
        )
        .map_err(read_error)?;
    if cursor_rows != 1 || version_rows != 1 {
        return Err(HubStoreError::Corrupt {
            message: "Hub change durable heads could not advance atomically".into(),
        });
    }
    Ok(())
}
