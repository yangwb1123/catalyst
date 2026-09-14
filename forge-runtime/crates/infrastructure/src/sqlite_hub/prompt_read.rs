use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};

use super::read::{ensure_conversation_exists, not_found};
use super::{
    ConversationOwner, ConversationPrompt, ConversationPromptCursor, ConversationPromptPage,
    HubEntity, HubStoreError, MAX_CONVERSATION_PROMPT_PAGE_CONTENT_BYTES,
    MAX_CONVERSATION_PROMPT_PAGE_LIMIT, MAX_HUB_ENTITY_ID_BYTES, MAX_HUB_ROLE_BYTES,
    MAX_PROMPT_CONTENT_BYTES, PromptRecord, read_error, rows,
};

const PROMPT_COLUMNS: &str = "id, conversation_id, role, content, idempotency_key, created_at_ms";
const CAUSAL_HISTORY_SQL: &str = "WITH causal AS (
  SELECT p.id, p.conversation_id, p.role, p.content,
         p.idempotency_key, p.created_at_ms,
         p.rowid AS prompt_rowid,
         COALESCE(source.rowid, p.rowid) AS anchor_rowid,
         CASE WHEN w.run_id IS NULL THEN 0 ELSE 1 END AS run_answer,
         r.rowid AS run_rowid
  FROM prompts p
  LEFT JOIN run_assistant_prompts w ON w.prompt_id = p.id
  LEFT JOIN runs r ON r.id = w.run_id
  LEFT JOIN prompts source ON source.id = r.prompt_id
  WHERE p.conversation_id = ?1 AND p.id <> ?2
    AND COALESCE(source.rowid, p.rowid) < ?3
), ranked AS (
  SELECT *, ROW_NUMBER() OVER (
    PARTITION BY anchor_rowid
    ORDER BY run_answer DESC, run_rowid DESC, prompt_rowid DESC
  ) AS anchor_rank
  FROM causal
), anchor_groups AS (
  SELECT anchor_rowid, COUNT(*) AS group_size
  FROM causal
  GROUP BY anchor_rowid
), anchor_budgets AS (
  SELECT anchor_rowid, COALESCE(SUM(group_size) OVER (
    ORDER BY anchor_rowid DESC
    ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING
  ), 0) AS newer_size
  FROM anchor_groups
)
SELECT r.id, r.conversation_id, r.role, r.content,
       r.idempotency_key, r.created_at_ms
FROM ranked r
JOIN anchor_budgets budget ON budget.anchor_rowid = r.anchor_rowid
WHERE budget.newer_size < ?4
  AND (
    r.run_answer = 0
    OR r.anchor_rank <= ?4 - budget.newer_size - 1
  )
ORDER BY r.anchor_rowid DESC, r.run_answer DESC,
         r.run_rowid DESC, r.prompt_rowid DESC
LIMIT ?4";
pub(super) fn list_prompts(
    connection: &Connection,
    conversation_id: Option<&str>,
    limit: usize,
) -> Result<Vec<PromptRecord>, HubStoreError> {
    if let Some(id) = conversation_id {
        ensure_conversation_exists(connection, id)?;
    }
    let limit = i64::try_from(limit).map_err(|error| HubStoreError::Conflict {
        entity: HubEntity::Prompt,
        message: error.to_string(),
    })?;
    let sql = match conversation_id {
        Some(_) => format!(
            "SELECT {PROMPT_COLUMNS} FROM prompts
             WHERE conversation_id = ?1
             ORDER BY created_at_ms DESC, id DESC LIMIT ?2"
        ),
        None => format!(
            "SELECT {PROMPT_COLUMNS} FROM prompts
             ORDER BY created_at_ms DESC, id DESC LIMIT ?1"
        ),
    };
    let mut statement = connection.prepare(&sql).map_err(read_error)?;
    let records = match conversation_id {
        Some(id) => statement.query_map(params![id, limit], rows::prompt),
        None => statement.query_map(params![limit], rows::prompt),
    }
    .map_err(read_error)?;
    records.collect::<Result<Vec<_>, _>>().map_err(read_error)
}

pub(super) fn conversation_prompt_page(
    connection: &mut Connection,
    conversation_id: &str,
    before: Option<&ConversationPromptCursor>,
    limit: usize,
) -> Result<ConversationPromptPage, HubStoreError> {
    conversation_prompt_page_impl(connection, None, conversation_id, before, limit)
}

pub(super) fn owned_conversation_prompt_page(
    connection: &mut Connection,
    owner: &ConversationOwner,
    conversation_id: &str,
    before: Option<&ConversationPromptCursor>,
    limit: usize,
) -> Result<ConversationPromptPage, HubStoreError> {
    conversation_prompt_page_impl(connection, Some(owner), conversation_id, before, limit)
}

fn conversation_prompt_page_impl(
    connection: &mut Connection,
    owner: Option<&ConversationOwner>,
    conversation_id: &str,
    before: Option<&ConversationPromptCursor>,
    limit: usize,
) -> Result<ConversationPromptPage, HubStoreError> {
    let (scan_limit, cursor_time) = validate_prompt_page(limit, before)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    if let Some(owner) = owner {
        ensure_conversation_owner(&transaction, owner, conversation_id)?;
    } else {
        ensure_conversation_exists(&transaction, conversation_id)?;
    }
    let metadata = prompt_page_metadata(
        &transaction,
        conversation_id,
        before,
        cursor_time,
        scan_limit,
    )?;
    let (prompts, has_more) = hydrate_prompt_page(&transaction, conversation_id, metadata, limit)?;
    let next_cursor = if has_more {
        prompts.last().map(|prompt| ConversationPromptCursor {
            created_at_ms: prompt.created_at_ms,
            prompt_id: prompt.id.clone(),
        })
    } else {
        None
    };
    transaction.commit().map_err(read_error)?;
    Ok(ConversationPromptPage {
        conversation_id: conversation_id.to_owned(),
        prompts,
        next_cursor,
        has_more,
    })
}

fn validate_prompt_page(
    limit: usize,
    before: Option<&ConversationPromptCursor>,
) -> Result<(i64, Option<i64>), HubStoreError> {
    if !(1..=MAX_CONVERSATION_PROMPT_PAGE_LIMIT).contains(&limit) {
        return Err(HubStoreError::Conflict {
            entity: HubEntity::Prompt,
            message: "invalid Conversation Prompt page limit".into(),
        });
    }
    let scan_limit = i64::try_from(limit + 1).map_err(|error| HubStoreError::Conflict {
        entity: HubEntity::Prompt,
        message: error.to_string(),
    })?;
    let cursor_time = before
        .map(|cursor| i64::try_from(cursor.created_at_ms))
        .transpose()
        .map_err(|_| corrupt_prompt_page("Prompt page cursor timestamp is out of range"))?;
    Ok((scan_limit, cursor_time))
}

fn ensure_conversation_owner(
    transaction: &rusqlite::Transaction<'_>,
    owner: &ConversationOwner,
    conversation_id: &str,
) -> Result<(), HubStoreError> {
    let owns: bool = transaction
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM conversation_owners
               WHERE conversation_id = ?1 AND issuer = ?2 AND subject = ?3 AND tenant_id = ?4
             )",
            params![
                conversation_id,
                owner.issuer,
                owner.subject,
                owner.tenant_id
            ],
            |row| row.get(0),
        )
        .map_err(read_error)?;
    if owns {
        return Ok(());
    }
    Err(HubStoreError::NotFound {
        entity: HubEntity::Conversation,
        id: conversation_id.into(),
    })
}

fn hydrate_prompt_page(
    transaction: &rusqlite::Transaction<'_>,
    conversation_id: &str,
    metadata: Vec<PromptPageMetadata>,
    limit: usize,
) -> Result<(Vec<ConversationPrompt>, bool), HubStoreError> {
    let mut prompts = Vec::with_capacity(limit.min(metadata.len()));
    let mut content_bytes = 0_usize;
    let mut has_more = false;
    for row in metadata {
        if prompts.len() == limit {
            has_more = true;
            break;
        }
        validate_prompt_page_metadata(&row)?;
        if row.content_bytes > MAX_CONVERSATION_PROMPT_PAGE_CONTENT_BYTES {
            return Err(corrupt_prompt_page(
                "Prompt exceeds the frozen history page content budget",
            ));
        }
        let next_bytes = content_bytes
            .checked_add(row.content_bytes)
            .ok_or_else(|| corrupt_prompt_page("Prompt page content budget overflow"))?;
        if next_bytes > MAX_CONVERSATION_PROMPT_PAGE_CONTENT_BYTES {
            has_more = true;
            break;
        }
        let prompt = transaction
            .query_row(
                "SELECT id, conversation_id, role, content, created_at_ms
                 FROM prompts WHERE rowid = ?1 AND conversation_id = ?2",
                params![row.rowid, conversation_id],
                rows::conversation_prompt,
            )
            .map_err(read_error)?;
        if prompt.content.len() != row.content_bytes || prompt.created_at_ms != row.created_at_ms {
            return Err(corrupt_prompt_page("Prompt changed during a read snapshot"));
        }
        content_bytes = next_bytes;
        prompts.push(prompt);
    }
    Ok((prompts, has_more))
}

fn prompt_page_metadata(
    connection: &Connection,
    conversation_id: &str,
    before: Option<&ConversationPromptCursor>,
    cursor_time: Option<i64>,
    limit: i64,
) -> Result<Vec<PromptPageMetadata>, HubStoreError> {
    const COLUMNS: &str = "SELECT rowid, created_at_ms,
        length(CAST(id AS BLOB)), length(CAST(role AS BLOB)),
        length(CAST(content AS BLOB)) FROM prompts WHERE conversation_id = ";
    let sql = match before {
        Some(_) => format!(
            "{COLUMNS}?1 AND (created_at_ms < ?2 OR (created_at_ms = ?2 AND id < ?3))
             ORDER BY created_at_ms DESC, id DESC LIMIT ?4"
        ),
        None => format!("{COLUMNS}?1 ORDER BY created_at_ms DESC, id DESC LIMIT ?2"),
    };
    let mut statement = connection.prepare(&sql).map_err(read_error)?;
    let mapped = match (before, cursor_time) {
        (Some(cursor), Some(time)) => statement.query_map(
            params![conversation_id, time, cursor.prompt_id, limit],
            prompt_page_metadata_row,
        ),
        (None, None) => {
            statement.query_map(params![conversation_id, limit], prompt_page_metadata_row)
        }
        _ => return Err(corrupt_prompt_page("Prompt page cursor is incomplete")),
    }
    .map_err(read_error)?;
    mapped.collect::<Result<Vec<_>, _>>().map_err(read_error)
}

#[derive(Clone, Copy)]
struct PromptPageMetadata {
    rowid: i64,
    created_at_ms: u64,
    id_bytes: usize,
    role_bytes: usize,
    content_bytes: usize,
}

fn prompt_page_metadata_row(row: &Row<'_>) -> rusqlite::Result<PromptPageMetadata> {
    let created_at_ms = u64::try_from(row.get::<_, i64>(1)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            1,
            rusqlite::types::Type::Integer,
            Box::new(error),
        )
    })?;
    Ok(PromptPageMetadata {
        rowid: row.get(0)?,
        created_at_ms,
        id_bytes: metadata_bytes(row, 2)?,
        role_bytes: metadata_bytes(row, 3)?,
        content_bytes: metadata_bytes(row, 4)?,
    })
}

fn metadata_bytes(row: &Row<'_>, index: usize) -> rusqlite::Result<usize> {
    usize::try_from(row.get::<_, i64>(index)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Integer,
            Box::new(error),
        )
    })
}

fn validate_prompt_page_metadata(row: &PromptPageMetadata) -> Result<(), HubStoreError> {
    if !(1..=MAX_HUB_ENTITY_ID_BYTES).contains(&row.id_bytes)
        || !(1..=MAX_HUB_ROLE_BYTES).contains(&row.role_bytes)
        || row.content_bytes > MAX_PROMPT_CONTENT_BYTES
    {
        return Err(corrupt_prompt_page("Prompt row exceeds stored Hub bounds"));
    }
    Ok(())
}

fn corrupt_prompt_page(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}

pub(super) fn list_prompts_before(
    connection: &mut Connection,
    conversation_id: &str,
    boundary_prompt_id: &str,
    limit: usize,
) -> Result<Vec<PromptRecord>, HubStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    ensure_conversation_exists(&transaction, conversation_id)?;
    validate_causal_associations(&transaction, conversation_id)?;
    let (boundary_rowid, role) =
        prompt_boundary(&transaction, conversation_id, boundary_prompt_id)?;
    if role != "user" {
        return Err(HubStoreError::Conflict {
            entity: HubEntity::Prompt,
            message: "history boundary must be a user prompt".into(),
        });
    }
    let records = query_prompts_before(
        &transaction,
        conversation_id,
        boundary_prompt_id,
        boundary_rowid,
        limit,
    )?;
    transaction.commit().map_err(read_error)?;
    Ok(records)
}

pub(super) fn validate_causal_associations(
    connection: &Connection,
    conversation_id: &str,
) -> Result<(), HubStoreError> {
    let invalid = connection
        .query_row(
            "SELECT COALESCE(p.id, w.prompt_id)
             FROM run_assistant_prompts w
             LEFT JOIN prompts p ON p.id = w.prompt_id
             LEFT JOIN runs r ON r.id = w.run_id
             LEFT JOIN prompts source ON source.id = r.prompt_id
             WHERE (
               p.conversation_id = ?1 OR r.conversation_id = ?1
               OR source.conversation_id = ?1
             ) AND (
               p.id IS NULL OR r.id IS NULL OR source.id IS NULL
               OR p.role <> 'assistant' OR source.role <> 'user'
               OR p.conversation_id <> r.conversation_id
               OR r.conversation_id <> source.conversation_id
             )
             LIMIT 1",
            [conversation_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(read_error)?;
    invalid.map_or(Ok(()), |prompt_id| {
        Err(HubStoreError::Corrupt {
            message: format!("invalid Run assistant association for Prompt '{prompt_id}'"),
        })
    })
}

fn prompt_boundary(
    connection: &Connection,
    conversation_id: &str,
    prompt_id: &str,
) -> Result<(i64, String), HubStoreError> {
    connection
        .query_row(
            "SELECT rowid, role FROM prompts
             WHERE id = ?1 AND conversation_id = ?2",
            params![prompt_id, conversation_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(read_error)?
        .ok_or_else(|| not_found(HubEntity::Prompt, prompt_id))
}

fn query_prompts_before(
    connection: &Connection,
    conversation_id: &str,
    boundary_prompt_id: &str,
    boundary_rowid: i64,
    limit: usize,
) -> Result<Vec<PromptRecord>, HubStoreError> {
    let limit = i64::try_from(limit).map_err(|error| HubStoreError::Conflict {
        entity: HubEntity::Prompt,
        message: error.to_string(),
    })?;
    let mut statement = connection.prepare(CAUSAL_HISTORY_SQL).map_err(read_error)?;
    statement
        .query_map(
            params![conversation_id, boundary_prompt_id, boundary_rowid, limit],
            rows::prompt,
        )
        .map_err(read_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(read_error)
}
