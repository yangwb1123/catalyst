use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use super::{
    ConversationImportPrompt, HubEntity, HubStoreError, LocalConversationImportSource,
    MAX_CONVERSATION_IMPORT_PROMPT_COUNT, MAX_PROMPT_CONTENT_BYTES, read_error, rows,
};

const CONVERSATION_COLUMNS: &str = "id, scope_kind, scope_id, title, created_at_ms, updated_at_ms";

struct PromptMetadata {
    id: String,
    role: String,
    content_bytes: usize,
}

/// Reads one legacy/local Conversation in a single bounded read transaction.
/// Only ownerless sources and user/assistant text are eligible for transfer.
// Keep the owner check, bounded metadata scan, body load, and snapshot commit together.
#[allow(clippy::too_many_lines)]
pub(super) fn local_conversation_import_source(
    connection: &mut Connection,
    conversation_id: &str,
) -> Result<LocalConversationImportSource, HubStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let conversation = load_conversation(&transaction, conversation_id)?;
    ensure_ownerless_source(&transaction, conversation_id)?;
    let metadata = prompt_metadata(&transaction, conversation_id)?;
    let content_bytes = bounded_content_bytes(&metadata)?;
    let prompts = load_prompts(&transaction, conversation_id, &metadata)?;
    transaction.commit().map_err(read_error)?;
    Ok(LocalConversationImportSource {
        conversation,
        prompts,
        content_bytes,
    })
}

fn load_conversation(
    transaction: &Transaction<'_>,
    conversation_id: &str,
) -> Result<super::Conversation, HubStoreError> {
    transaction
        .query_row(
            &format!("SELECT {CONVERSATION_COLUMNS} FROM conversations WHERE id = ?1"),
            [conversation_id],
            rows::conversation,
        )
        .optional()
        .map_err(read_error)?
        .ok_or_else(|| HubStoreError::NotFound {
            entity: HubEntity::Conversation,
            id: conversation_id.into(),
        })
}

fn ensure_ownerless_source(
    transaction: &Transaction<'_>,
    conversation_id: &str,
) -> Result<(), HubStoreError> {
    let already_owned: bool = transaction
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM conversation_owners WHERE conversation_id = ?1
             )",
            [conversation_id],
            |row| row.get(0),
        )
        .map_err(read_error)?;
    if already_owned {
        return Err(HubStoreError::Conflict {
            entity: HubEntity::Conversation,
            message: "only an ownerless local Conversation can be imported".into(),
        });
    }
    Ok(())
}

fn prompt_metadata(
    transaction: &Transaction<'_>,
    conversation_id: &str,
) -> Result<Vec<PromptMetadata>, HubStoreError> {
    let scan_limit = i64::try_from(MAX_CONVERSATION_IMPORT_PROMPT_COUNT + 1).map_err(|error| {
        HubStoreError::Conflict {
            entity: HubEntity::Conversation,
            message: error.to_string(),
        }
    })?;
    let mut statement = transaction
        .prepare(
            "SELECT id, role, length(CAST(content AS BLOB))
             FROM prompts
             WHERE conversation_id = ?1 AND role IN ('user', 'assistant')
             ORDER BY created_at_ms DESC, id COLLATE BINARY DESC
             LIMIT ?2",
        )
        .map_err(read_error)?;
    let mapped = statement
        .query_map(params![conversation_id, scan_limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(read_error)?;
    let raw = mapped.collect::<Result<Vec<_>, _>>().map_err(read_error)?;
    if raw.len() > MAX_CONVERSATION_IMPORT_PROMPT_COUNT {
        return Err(import_limit_error(
            "local Conversation exceeds the import Prompt count limit",
        ));
    }
    raw.into_iter()
        .map(|(id, role, byte_count)| {
            let content_bytes =
                usize::try_from(byte_count).map_err(|_| HubStoreError::Corrupt {
                    message: "local Conversation has an invalid Prompt length".into(),
                })?;
            Ok(PromptMetadata {
                id,
                role,
                content_bytes,
            })
        })
        .collect()
}

fn bounded_content_bytes(metadata: &[PromptMetadata]) -> Result<usize, HubStoreError> {
    let mut total = 0_usize;
    for prompt in metadata {
        total = total.saturating_add(prompt.content_bytes);
        if total > MAX_PROMPT_CONTENT_BYTES {
            return Err(import_limit_error(
                "local Conversation exceeds the import content-byte limit",
            ));
        }
    }
    Ok(total)
}

fn load_prompts(
    transaction: &Transaction<'_>,
    conversation_id: &str,
    metadata: &[PromptMetadata],
) -> Result<Vec<ConversationImportPrompt>, HubStoreError> {
    let mut prompts = Vec::with_capacity(metadata.len());
    for item in metadata {
        let (role, content) = transaction
            .query_row(
                "SELECT role, content FROM prompts
                 WHERE id = ?1 AND conversation_id = ?2",
                params![item.id, conversation_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .map_err(read_error)?;
        if role != item.role || content.len() != item.content_bytes {
            return Err(HubStoreError::Corrupt {
                message: "local Conversation changed while its import preview was read".into(),
            });
        }
        if content.trim().is_empty() {
            return Err(import_limit_error(
                "local Conversation contains a blank visible Prompt and cannot be imported",
            ));
        }
        prompts.push(ConversationImportPrompt { role, content });
    }
    prompts.reverse();
    Ok(prompts)
}

fn import_limit_error(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::Conversation,
        message: message.into(),
    }
}
