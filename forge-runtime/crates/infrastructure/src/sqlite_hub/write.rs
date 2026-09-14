use std::path::Path;

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

mod group;
mod owned;
pub(super) use group::{add_project_to_group, create_group};
pub(super) use owned::{append_owned_prompt, create_owned_conversation, import_owned_conversation};

use super::{
    Conversation, ConversationScope, HubEntity, HubStoreError, Project, PromptRecord, read_error,
    rows, write_error,
};

const PROJECT_COLUMNS: &str = "id, name, canonical_path, created_at_ms";
const CONVERSATION_COLUMNS: &str = "id, scope_kind, scope_id, title, created_at_ms, updated_at_ms";
const PROMPT_COLUMNS: &str = "id, conversation_id, role, content, idempotency_key, created_at_ms";

pub(super) fn open_project(
    connection: &mut Connection,
    absolute_path: &Path,
) -> Result<Project, HubStoreError> {
    if !absolute_path.is_absolute() {
        return Err(conflict(
            HubEntity::Project,
            "project path must be canonical and absolute",
        ));
    }
    let path = rows::path_text(absolute_path)?;
    let transaction = begin(connection)?;
    if let Some(project) = project_by_path(&transaction, path)? {
        transaction
            .commit()
            .map_err(|error| write_error(HubEntity::Project, error))?;
        return Ok(project);
    }
    let project = Project {
        id: rows::new_id(&transaction, "project")?,
        name: project_name(absolute_path),
        path: absolute_path.to_path_buf(),
        created_at_ms: rows::now_ms()?,
    };
    transaction
        .execute(
            "INSERT INTO projects(id,name,canonical_path,created_at_ms)
             VALUES(?1,?2,?3,?4)",
            params![
                project.id,
                project.name,
                path,
                to_i64(project.created_at_ms)?
            ],
        )
        .map_err(|error| write_error(HubEntity::Project, error))?;
    transaction
        .commit()
        .map_err(|error| write_error(HubEntity::Project, error))?;
    Ok(project)
}

pub(super) fn create_conversation(
    connection: &mut Connection,
    scope: &ConversationScope,
    title: &str,
    idempotency_key: &str,
) -> Result<Conversation, HubStoreError> {
    let transaction = begin(connection)?;
    ensure_scope_exists(&transaction, scope)?;
    if let Some(existing) = conversation_by_key(&transaction, idempotency_key)? {
        ensure_same_conversation(&existing, scope, title)?;
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
    super::change_write::append_conversation_change(
        &transaction,
        "conversation_created",
        &conversation.id,
        &conversation.id,
        conversation.created_at_ms,
    )?;
    transaction
        .commit()
        .map_err(|error| write_error(HubEntity::Conversation, error))?;
    Ok(conversation)
}

pub(super) fn append_prompt(
    connection: &mut Connection,
    conversation_id: &str,
    role: &str,
    content: &str,
    idempotency_key: &str,
) -> Result<PromptRecord, HubStoreError> {
    let transaction = begin(connection)?;
    ensure_exists(
        &transaction,
        "conversations",
        conversation_id,
        HubEntity::Conversation,
    )?;
    if let Some(existing) = prompt_by_key(&transaction, idempotency_key)? {
        ensure_same_prompt(&existing, conversation_id, role, content)?;
        transaction
            .commit()
            .map_err(|error| write_error(HubEntity::Prompt, error))?;
        return Ok(existing);
    }
    let prompt = PromptRecord {
        id: rows::new_id(&transaction, "prompt")?,
        conversation_id: conversation_id.into(),
        role: role.into(),
        content: content.into(),
        idempotency_key: idempotency_key.into(),
        created_at_ms: rows::now_ms()?,
    };
    insert_prompt(&transaction, &prompt)?;
    transaction
        .commit()
        .map_err(|error| write_error(HubEntity::Prompt, error))?;
    Ok(prompt)
}

fn begin(connection: &mut Connection) -> Result<Transaction<'_>, HubStoreError> {
    connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(super::unavailable)
}

fn project_by_path(
    transaction: &Transaction<'_>,
    path: &str,
) -> Result<Option<Project>, HubStoreError> {
    transaction
        .query_row(
            &format!("SELECT {PROJECT_COLUMNS} FROM projects WHERE canonical_path = ?1"),
            [path],
            rows::project,
        )
        .optional()
        .map_err(read_error)
}

fn conversation_by_key(
    transaction: &Transaction<'_>,
    key: &str,
) -> Result<Option<Conversation>, HubStoreError> {
    transaction
        .query_row(
            &format!(
                "SELECT {CONVERSATION_COLUMNS} FROM conversations
                 WHERE idempotency_key = ?1"
            ),
            [key],
            rows::conversation,
        )
        .optional()
        .map_err(read_error)
}

fn prompt_by_key(
    transaction: &Transaction<'_>,
    key: &str,
) -> Result<Option<PromptRecord>, HubStoreError> {
    transaction
        .query_row(
            &format!("SELECT {PROMPT_COLUMNS} FROM prompts WHERE idempotency_key = ?1"),
            [key],
            rows::prompt,
        )
        .optional()
        .map_err(read_error)
}

fn ensure_scope_exists(
    transaction: &Transaction<'_>,
    scope: &ConversationScope,
) -> Result<(), HubStoreError> {
    match scope {
        ConversationScope::Global => Ok(()),
        ConversationScope::Project(id) => {
            ensure_exists(transaction, "projects", id, HubEntity::Project)
        }
        ConversationScope::Group(id) => ensure_exists(transaction, "groups", id, HubEntity::Group),
    }
}

fn ensure_exists(
    transaction: &Transaction<'_>,
    table: &str,
    id: &str,
    entity: HubEntity,
) -> Result<(), HubStoreError> {
    let sql = match table {
        "projects" => "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
        "groups" => "SELECT EXISTS(SELECT 1 FROM groups WHERE id = ?1)",
        "conversations" => "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ?1)",
        _ => return Err(conflict(entity, "unsupported existence check")),
    };
    let exists: bool = transaction
        .query_row(sql, [id], |row| row.get(0))
        .map_err(read_error)?;
    exists.then_some(()).ok_or_else(|| HubStoreError::NotFound {
        entity,
        id: id.into(),
    })
}

fn insert_conversation(
    transaction: &Transaction<'_>,
    conversation: &Conversation,
    key: &str,
) -> Result<(), HubStoreError> {
    let (kind, scope_id) = rows::scope_parts(&conversation.scope);
    transaction
        .execute(
            "INSERT INTO conversations(
               id,scope_kind,scope_id,title,idempotency_key,created_at_ms,updated_at_ms
             ) VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                conversation.id,
                kind,
                scope_id,
                conversation.title,
                key,
                to_i64(conversation.created_at_ms)?,
                to_i64(conversation.updated_at_ms)?
            ],
        )
        .map_err(|error| write_error(HubEntity::Conversation, error))?;
    Ok(())
}

pub(super) fn insert_prompt(
    transaction: &Transaction<'_>,
    prompt: &PromptRecord,
) -> Result<(), HubStoreError> {
    insert_prompt_with_hook(transaction, prompt, |_| Ok(()))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PromptInsertStage {
    PromptRow,
    ConversationTimestamp,
    ChangeJournal,
}

pub(super) fn insert_prompt_with_hook(
    transaction: &Transaction<'_>,
    prompt: &PromptRecord,
    mut after_stage: impl FnMut(PromptInsertStage) -> Result<(), HubStoreError>,
) -> Result<(), HubStoreError> {
    let created_at = to_i64(prompt.created_at_ms)?;
    transaction
        .execute(
            "INSERT INTO prompts(
               id,conversation_id,role,content,idempotency_key,created_at_ms
             ) VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                prompt.id,
                prompt.conversation_id,
                prompt.role,
                prompt.content,
                prompt.idempotency_key,
                created_at
            ],
        )
        .map_err(|error| write_error(HubEntity::Prompt, error))?;
    after_stage(PromptInsertStage::PromptRow)?;
    transaction
        .execute(
            "UPDATE conversations SET updated_at_ms = ?1 WHERE id = ?2",
            params![created_at, prompt.conversation_id],
        )
        .map_err(|error| write_error(HubEntity::Conversation, error))?;
    after_stage(PromptInsertStage::ConversationTimestamp)?;
    super::change_write::append_conversation_change(
        transaction,
        "prompt_appended",
        &prompt.conversation_id,
        &prompt.id,
        prompt.created_at_ms,
    )?;
    after_stage(PromptInsertStage::ChangeJournal)?;
    Ok(())
}

fn ensure_same_conversation(
    existing: &Conversation,
    scope: &ConversationScope,
    title: &str,
) -> Result<(), HubStoreError> {
    (existing.scope == *scope && existing.title == title)
        .then_some(())
        .ok_or_else(|| {
            conflict(
                HubEntity::Conversation,
                "idempotency key was reused with different conversation data",
            )
        })
}

fn ensure_same_prompt(
    existing: &PromptRecord,
    conversation_id: &str,
    role: &str,
    content: &str,
) -> Result<(), HubStoreError> {
    (existing.conversation_id == conversation_id
        && existing.role == role
        && existing.content == content)
        .then_some(())
        .ok_or_else(|| {
            conflict(
                HubEntity::Prompt,
                "idempotency key was reused with different prompt data",
            )
        })
}

fn project_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map_or_else(|| path.display().to_string(), str::to_owned)
}

pub(super) fn to_i64(value: u64) -> Result<i64, HubStoreError> {
    i64::try_from(value).map_err(|error| HubStoreError::Unavailable {
        message: error.to_string(),
    })
}

fn conflict(entity: HubEntity, message: impl Into<String>) -> HubStoreError {
    HubStoreError::Conflict {
        entity,
        message: message.into(),
    }
}
