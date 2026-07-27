use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, Row, types::Type};

use super::{
    Conversation, ConversationScope, GroupProjectMember, HubStoreError, Project, PromptRecord,
    SessionGroup, unavailable,
};

pub(super) fn project(row: &Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get(0)?,
        name: row.get(1)?,
        path: PathBuf::from(row.get::<_, String>(2)?),
        created_at_ms: timestamp(row, 3)?,
    })
}

pub(super) fn conversation(row: &Row<'_>) -> rusqlite::Result<Conversation> {
    let kind: String = row.get(1)?;
    let id: Option<String> = row.get(2)?;
    let scope = decode_scope(&kind, id).map_err(|message| conversion_error(1, message))?;
    Ok(Conversation {
        id: row.get(0)?,
        scope,
        title: row.get(3)?,
        created_at_ms: timestamp(row, 4)?,
        updated_at_ms: timestamp(row, 5)?,
    })
}

pub(super) fn prompt(row: &Row<'_>) -> rusqlite::Result<PromptRecord> {
    Ok(PromptRecord {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        role: row.get(2)?,
        content: row.get(3)?,
        idempotency_key: row.get(4)?,
        created_at_ms: timestamp(row, 5)?,
    })
}

pub(super) fn group(row: &Row<'_>) -> rusqlite::Result<SessionGroup> {
    Ok(SessionGroup {
        id: row.get(0)?,
        name: row.get(1)?,
        created_at_ms: timestamp(row, 2)?,
    })
}

pub(super) fn group_member(row: &Row<'_>) -> rusqlite::Result<GroupProjectMember> {
    Ok(GroupProjectMember {
        group_id: row.get(0)?,
        project_id: row.get(1)?,
        role: row.get(2)?,
        added_at_ms: timestamp(row, 3)?,
    })
}

pub(super) fn scope_parts(scope: &ConversationScope) -> (&'static str, Option<&str>) {
    match scope {
        ConversationScope::Global => ("global", None),
        ConversationScope::Project(id) => ("project", Some(id)),
        ConversationScope::Group(id) => ("group", Some(id)),
    }
}

pub(super) fn path_text(path: &Path) -> Result<&str, HubStoreError> {
    path.to_str().ok_or_else(|| HubStoreError::Unavailable {
        message: format!(
            "Hub phase 1 requires a UTF-8 project path: {}",
            path.display()
        ),
    })
}

pub(super) fn now_ms() -> Result<u64, HubStoreError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(unavailable)?;
    u64::try_from(duration.as_millis()).map_err(unavailable)
}

pub(super) fn new_id(connection: &Connection, prefix: &str) -> Result<String, HubStoreError> {
    let random: String = connection
        .query_row("SELECT lower(hex(randomblob(16)))", [], |row| row.get(0))
        .map_err(unavailable)?;
    Ok(format!("{prefix}-{random}"))
}

fn decode_scope(kind: &str, id: Option<String>) -> Result<ConversationScope, String> {
    match (kind, id) {
        ("global", None) => Ok(ConversationScope::Global),
        ("project", Some(id)) => Ok(ConversationScope::Project(id)),
        ("group", Some(id)) => Ok(ConversationScope::Group(id)),
        _ => Err(format!("invalid conversation scope '{kind}'")),
    }
}

fn timestamp(row: &Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    u64::try_from(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(index, Type::Integer, Box::new(error))
    })
}

fn conversion_error(index: usize, message: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        index,
        Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message,
        )),
    )
}
