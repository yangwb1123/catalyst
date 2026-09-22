use rusqlite::{Connection, OptionalExtension, Transaction};

use super::super::{
    Conversation, ConversationImportPrompt, ConversationOwner, ConversationPrompt,
    ConversationScope, HubEntity, HubStoreError, OwnedConversationImportResult,
    OwnedPromptAppendResult, PromptRecord, read_error, rows, write_error,
};
use super::{
    begin, conversation_by_key, ensure_same_conversation, ensure_same_prompt, ensure_scope_exists,
    insert_conversation, insert_prompt, prompt_by_key, to_i64,
};

pub(in crate::sqlite_hub) fn create_owned_conversation(
    connection: &mut Connection,
    owner: &ConversationOwner,
    scope: &ConversationScope,
    title: &str,
    idempotency_key: &str,
) -> Result<Conversation, HubStoreError> {
    let transaction = begin(connection)?;
    ensure_scope_exists(&transaction, scope)?;
    if let Some(existing) = conversation_by_key(&transaction, idempotency_key)? {
        ensure_same_conversation(&existing, scope, title)?;
        ensure_conversation_owner(&transaction, owner, &existing.id)?;
        transaction
            .commit()
            .map_err(|error| write_error(HubEntity::Conversation, error))?;
        return Ok(existing);
    }
    let now = rows::now_ms()?;
    let conversation = Conversation {
        id: rows::new_id(&transaction, "session")?,
        scope: scope.clone(),
        title: title.into(),
        created_at_ms: now,
        updated_at_ms: now,
    };
    insert_conversation(&transaction, &conversation, idempotency_key)?;
    add_owner_and_creation_change(&transaction, owner, &conversation, now)?;
    transaction
        .commit()
        .map_err(|error| write_error(HubEntity::Conversation, error))?;
    Ok(conversation)
}

fn add_owner_and_creation_change(
    transaction: &Transaction<'_>,
    owner: &ConversationOwner,
    conversation: &Conversation,
    created_at_ms: u64,
) -> Result<(), HubStoreError> {
    transaction
        .execute(
            "INSERT INTO conversation_owners(
               conversation_id, issuer, subject, tenant_id, created_at_ms
             ) VALUES(?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                conversation.id,
                owner.issuer,
                owner.subject,
                owner.tenant_id,
                to_i64(created_at_ms)?,
            ],
        )
        .map_err(|error| write_error(HubEntity::Conversation, error))?;
    super::super::change_write::append_conversation_change(
        transaction,
        "conversation_created",
        &conversation.id,
        &conversation.id,
        created_at_ms,
    )?;
    Ok(())
}

// Keep idempotency, ownership, Conversation, Prompt, and change writes in one transaction.
pub(in crate::sqlite_hub) fn import_owned_conversation(
    connection: &mut Connection,
    owner: &ConversationOwner,
    title: &str,
    prompts: &[ConversationImportPrompt],
    idempotency_key: &str,
) -> Result<OwnedConversationImportResult, HubStoreError> {
    let transaction = begin(connection)?;
    let scope = ConversationScope::Global;
    if let Some(result) = replay_owned_import(&transaction, owner, title, prompts, idempotency_key)?
    {
        transaction
            .commit()
            .map_err(|error| write_error(HubEntity::Conversation, error))?;
        return Ok(result);
    }
    ensure_scope_exists(&transaction, &scope)?;
    let result = insert_owned_import(&transaction, owner, title, prompts, idempotency_key)?;
    transaction
        .commit()
        .map_err(|error| write_error(HubEntity::Conversation, error))?;
    Ok(result)
}

fn replay_owned_import(
    transaction: &Transaction<'_>,
    owner: &ConversationOwner,
    title: &str,
    prompts: &[ConversationImportPrompt],
    idempotency_key: &str,
) -> Result<Option<OwnedConversationImportResult>, HubStoreError> {
    let Some(existing) = conversation_by_key(transaction, idempotency_key)? else {
        return Ok(None);
    };
    ensure_same_conversation(&existing, &ConversationScope::Global, title)?;
    ensure_conversation_owner(transaction, owner, &existing.id)?;
    let existing_prompts = imported_prompts(transaction, &existing.id, idempotency_key)?;
    if !same_import_prompts(&existing_prompts, prompts) {
        return Err(HubStoreError::Conflict {
            entity: HubEntity::Conversation,
            message: "idempotency key was reused with different import content".into(),
        });
    }
    let aggregate_version = conversation_change_version(transaction, &existing.id)?;
    Ok(Some(OwnedConversationImportResult {
        conversation: existing,
        aggregate_version,
        imported_prompt_count: prompts.len(),
        replayed: true,
    }))
}

fn same_import_prompts(saved: &[PromptRecord], requested: &[ConversationImportPrompt]) -> bool {
    saved.len() == requested.len()
        && saved
            .iter()
            .zip(requested)
            .all(|(left, right)| left.role == right.role && left.content == right.content)
}

fn insert_owned_import(
    transaction: &Transaction<'_>,
    owner: &ConversationOwner,
    title: &str,
    prompts: &[ConversationImportPrompt],
    idempotency_key: &str,
) -> Result<OwnedConversationImportResult, HubStoreError> {
    let prompt_count = u64::try_from(prompts.len()).map_err(|error| HubStoreError::Conflict {
        entity: HubEntity::Prompt,
        message: error.to_string(),
    })?;
    let created_at_ms = rows::now_ms()?.saturating_sub(prompt_count);
    let mut conversation =
        new_import_conversation(transaction, owner, title, idempotency_key, created_at_ms)?;
    insert_import_prompts(
        transaction,
        &conversation,
        prompts,
        idempotency_key,
        created_at_ms,
    )?;
    conversation.updated_at_ms = import_updated_at(created_at_ms, prompt_count)?;
    let aggregate_version = conversation_change_version(transaction, &conversation.id)?;
    Ok(OwnedConversationImportResult {
        conversation,
        aggregate_version,
        imported_prompt_count: prompts.len(),
        replayed: false,
    })
}

fn new_import_conversation(
    transaction: &Transaction<'_>,
    owner: &ConversationOwner,
    title: &str,
    idempotency_key: &str,
    created_at_ms: u64,
) -> Result<Conversation, HubStoreError> {
    let conversation = Conversation {
        id: rows::new_id(transaction, "session")?,
        scope: ConversationScope::Global,
        title: title.into(),
        created_at_ms,
        updated_at_ms: created_at_ms,
    };
    insert_conversation(transaction, &conversation, idempotency_key)?;
    add_owner_and_creation_change(transaction, owner, &conversation, created_at_ms)?;
    Ok(conversation)
}

fn insert_import_prompts(
    transaction: &Transaction<'_>,
    conversation: &Conversation,
    prompts: &[ConversationImportPrompt],
    idempotency_key: &str,
    created_at_ms: u64,
) -> Result<(), HubStoreError> {
    for (index, imported) in prompts.iter().enumerate() {
        let index = u64::try_from(index).map_err(|error| HubStoreError::Conflict {
            entity: HubEntity::Prompt,
            message: error.to_string(),
        })?;
        let prompt = PromptRecord {
            id: rows::new_id(transaction, "prompt")?,
            conversation_id: conversation.id.clone(),
            role: imported.role.clone(),
            content: imported.content.clone(),
            idempotency_key: format!("{idempotency_key}:prompt:{index}"),
            created_at_ms: created_at_ms.checked_add(index).ok_or_else(|| {
                HubStoreError::Corrupt {
                    message: "imported Conversation timestamp overflowed".into(),
                }
            })?,
        };
        insert_prompt(transaction, &prompt)?;
    }
    Ok(())
}

fn import_updated_at(created_at_ms: u64, prompt_count: u64) -> Result<u64, HubStoreError> {
    let Some(last_index) = prompt_count.checked_sub(1) else {
        return Ok(created_at_ms);
    };
    created_at_ms
        .checked_add(last_index)
        .ok_or_else(|| HubStoreError::Corrupt {
            message: "imported Conversation timestamp overflowed".into(),
        })
}

fn imported_prompts(
    transaction: &Transaction<'_>,
    conversation_id: &str,
    idempotency_key: &str,
) -> Result<Vec<PromptRecord>, HubStoreError> {
    let mut statement = transaction
        .prepare(
            "SELECT p.id, p.conversation_id, p.role, p.content,
                    p.idempotency_key, p.created_at_ms
             FROM conversation_changes c
             JOIN prompts p ON p.id = c.entity_id
             WHERE c.conversation_id = ?1 AND c.event_kind = 'prompt_appended'
               AND substr(p.idempotency_key, 1, length(?2)) = ?2
             ORDER BY c.aggregate_version ASC",
        )
        .map_err(read_error)?;
    let mapped = statement
        .query_map(
            rusqlite::params![conversation_id, format!("{idempotency_key}:prompt:")],
            rows::prompt,
        )
        .map_err(read_error)?;
    mapped.collect::<Result<Vec<_>, _>>().map_err(read_error)
}

pub(in crate::sqlite_hub) fn append_owned_prompt(
    connection: &mut Connection,
    owner: &ConversationOwner,
    conversation_id: &str,
    content: &str,
    idempotency_key: &str,
    expected_version: u64,
) -> Result<OwnedPromptAppendResult, HubStoreError> {
    let transaction = begin(connection)?;
    ensure_conversation_owner(&transaction, owner, conversation_id)?;
    if let Some(existing) = prompt_by_key(&transaction, idempotency_key)? {
        ensure_same_prompt(&existing, conversation_id, "user", content)?;
        let aggregate_version = prompt_change_version(&transaction, conversation_id, &existing.id)?;
        let result = owned_prompt_result(existing, aggregate_version, true);
        transaction
            .commit()
            .map_err(|error| write_error(HubEntity::Prompt, error))?;
        return Ok(result);
    }
    let actual_version = conversation_change_version(&transaction, conversation_id)?;
    if actual_version != expected_version {
        return Err(HubStoreError::Conflict {
            entity: HubEntity::Conversation,
            message: format!(
                "Conversation aggregate version changed: expected {expected_version}, found {actual_version}"
            ),
        });
    }
    let prompt = PromptRecord {
        id: rows::new_id(&transaction, "prompt")?,
        conversation_id: conversation_id.into(),
        role: "user".into(),
        content: content.into(),
        idempotency_key: idempotency_key.into(),
        created_at_ms: rows::now_ms()?,
    };
    insert_prompt(&transaction, &prompt)?;
    let aggregate_version = conversation_change_version(&transaction, conversation_id)?;
    let result = owned_prompt_result(prompt, aggregate_version, false);
    transaction
        .commit()
        .map_err(|error| write_error(HubEntity::Prompt, error))?;
    Ok(result)
}

fn ensure_conversation_owner(
    transaction: &Transaction<'_>,
    owner: &ConversationOwner,
    conversation_id: &str,
) -> Result<(), HubStoreError> {
    let owns: bool = transaction
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM conversation_owners
               WHERE conversation_id = ?1 AND issuer = ?2 AND subject = ?3 AND tenant_id = ?4
             )",
            rusqlite::params![
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

fn conversation_change_version(
    transaction: &Transaction<'_>,
    conversation_id: &str,
) -> Result<u64, HubStoreError> {
    let version: Option<i64> = transaction
        .query_row(
            "SELECT last_version FROM conversation_change_heads WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(read_error)?;
    version
        .map(u64::try_from)
        .transpose()
        .map_err(|error| HubStoreError::Corrupt {
            message: format!("invalid Conversation aggregate version: {error}"),
        })?
        .ok_or_else(|| HubStoreError::Corrupt {
            message: format!("owned Conversation '{conversation_id}' has no change head"),
        })
}

fn prompt_change_version(
    transaction: &Transaction<'_>,
    conversation_id: &str,
    prompt_id: &str,
) -> Result<u64, HubStoreError> {
    let version: Option<i64> = transaction
        .query_row(
            "SELECT aggregate_version
             FROM conversation_changes
             WHERE conversation_id = ?1
               AND entity_id = ?2
               AND event_kind = 'prompt_appended'",
            rusqlite::params![conversation_id, prompt_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(read_error)?;
    version
        .map(u64::try_from)
        .transpose()
        .map_err(|error| HubStoreError::Corrupt {
            message: format!("invalid Prompt aggregate version: {error}"),
        })?
        .ok_or_else(|| HubStoreError::Corrupt {
            message: format!(
                "owned Prompt '{prompt_id}' has no prompt_appended change in Conversation '{conversation_id}'"
            ),
        })
}

fn owned_prompt_result(
    prompt: PromptRecord,
    aggregate_version: u64,
    replayed: bool,
) -> OwnedPromptAppendResult {
    OwnedPromptAppendResult {
        prompt: ConversationPrompt {
            id: prompt.id,
            conversation_id: prompt.conversation_id,
            role: prompt.role,
            content: prompt.content,
            created_at_ms: prompt.created_at_ms,
        },
        aggregate_version,
        replayed,
    }
}
