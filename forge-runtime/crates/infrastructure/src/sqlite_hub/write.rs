use std::path::Path;

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use super::{
    Conversation, ConversationScope, GroupProjectMember, HubEntity, HubStoreError, Project,
    PromptRecord, SessionGroup, read_error, rows, write_error,
};

const PROJECT_COLUMNS: &str = "id, name, canonical_path, created_at_ms";
const CONVERSATION_COLUMNS: &str = "id, scope_kind, scope_id, title, created_at_ms, updated_at_ms";
const PROMPT_COLUMNS: &str = "id, conversation_id, role, content, idempotency_key, created_at_ms";
const GROUP_COLUMNS: &str = "id, name, created_at_ms";
const MEMBER_COLUMNS: &str = "group_id, project_id, role, added_at_ms";

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

pub(super) fn create_group(
    connection: &mut Connection,
    name: &str,
    idempotency_key: &str,
) -> Result<SessionGroup, HubStoreError> {
    let transaction = begin(connection)?;
    if let Some(existing) = group_by_key(&transaction, idempotency_key)? {
        if existing.name != name {
            return Err(conflict(
                HubEntity::Group,
                "idempotency key was reused with a different group name",
            ));
        }
        transaction
            .commit()
            .map_err(|error| write_error(HubEntity::Group, error))?;
        return Ok(existing);
    }
    let group = SessionGroup {
        id: rows::new_id(&transaction, "group")?,
        name: name.into(),
        created_at_ms: rows::now_ms()?,
    };
    transaction
        .execute(
            "INSERT INTO groups(id,name,idempotency_key,created_at_ms)
             VALUES(?1,?2,?3,?4)",
            params![
                group.id,
                group.name,
                idempotency_key,
                to_i64(group.created_at_ms)?
            ],
        )
        .map_err(|error| write_error(HubEntity::Group, error))?;
    transaction
        .commit()
        .map_err(|error| write_error(HubEntity::Group, error))?;
    Ok(group)
}

pub(super) fn add_project_to_group(
    connection: &mut Connection,
    group_id: &str,
    project_id: &str,
    role: &str,
    idempotency_key: &str,
) -> Result<GroupProjectMember, HubStoreError> {
    let transaction = begin(connection)?;
    ensure_exists(&transaction, "groups", group_id, HubEntity::Group)?;
    ensure_exists(&transaction, "projects", project_id, HubEntity::Project)?;
    if let Some(existing) = member_by_key(&transaction, idempotency_key)? {
        ensure_same_member(&existing, group_id, project_id, role)?;
        transaction
            .commit()
            .map_err(|error| write_error(HubEntity::GroupProjectMember, error))?;
        return Ok(existing);
    }
    if let Some(existing) = member_by_pair(&transaction, group_id, project_id)? {
        ensure_same_member(&existing, group_id, project_id, role)?;
        return Err(conflict(
            HubEntity::GroupProjectMember,
            "project link already exists under a different idempotency key",
        ));
    }
    let member = GroupProjectMember {
        group_id: group_id.into(),
        project_id: project_id.into(),
        role: role.into(),
        added_at_ms: rows::now_ms()?,
    };
    insert_member(&transaction, &member, idempotency_key)?;
    transaction
        .commit()
        .map_err(|error| write_error(HubEntity::GroupProjectMember, error))?;
    Ok(member)
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

fn group_by_key(
    transaction: &Transaction<'_>,
    key: &str,
) -> Result<Option<SessionGroup>, HubStoreError> {
    transaction
        .query_row(
            &format!("SELECT {GROUP_COLUMNS} FROM groups WHERE idempotency_key = ?1"),
            [key],
            rows::group,
        )
        .optional()
        .map_err(read_error)
}

fn member_by_key(
    transaction: &Transaction<'_>,
    key: &str,
) -> Result<Option<GroupProjectMember>, HubStoreError> {
    transaction
        .query_row(
            &format!("SELECT {MEMBER_COLUMNS} FROM group_projects WHERE idempotency_key = ?1"),
            [key],
            rows::group_member,
        )
        .optional()
        .map_err(read_error)
}

fn member_by_pair(
    transaction: &Transaction<'_>,
    group_id: &str,
    project_id: &str,
) -> Result<Option<GroupProjectMember>, HubStoreError> {
    transaction
        .query_row(
            &format!(
                "SELECT {MEMBER_COLUMNS} FROM group_projects
                 WHERE group_id = ?1 AND project_id = ?2"
            ),
            params![group_id, project_id],
            rows::group_member,
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

fn insert_prompt(
    transaction: &Transaction<'_>,
    prompt: &PromptRecord,
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
    transaction
        .execute(
            "UPDATE conversations SET updated_at_ms = ?1 WHERE id = ?2",
            params![created_at, prompt.conversation_id],
        )
        .map_err(|error| write_error(HubEntity::Conversation, error))?;
    Ok(())
}

fn insert_member(
    transaction: &Transaction<'_>,
    member: &GroupProjectMember,
    key: &str,
) -> Result<(), HubStoreError> {
    transaction
        .execute(
            "INSERT INTO group_projects(
               group_id,project_id,role,idempotency_key,added_at_ms
             ) VALUES(?1,?2,?3,?4,?5)",
            params![
                member.group_id,
                member.project_id,
                member.role,
                key,
                to_i64(member.added_at_ms)?
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupProjectMember, error))?;
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

fn ensure_same_member(
    existing: &GroupProjectMember,
    group_id: &str,
    project_id: &str,
    role: &str,
) -> Result<(), HubStoreError> {
    (existing.group_id == group_id && existing.project_id == project_id && existing.role == role)
        .then_some(())
        .ok_or_else(|| {
            conflict(
                HubEntity::GroupProjectMember,
                "project is already linked with different membership data",
            )
        })
}

fn project_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map_or_else(|| path.display().to_string(), str::to_owned)
}

fn to_i64(value: u64) -> Result<i64, HubStoreError> {
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
