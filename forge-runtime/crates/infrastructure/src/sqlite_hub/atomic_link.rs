use std::path::Path;

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use super::{GroupProjectMember, HubEntity, HubStoreError, Project, read_error, rows, write_error};

const PROJECT_COLUMNS: &str = "id, name, canonical_path, created_at_ms";
const MEMBER_COLUMNS: &str = "group_id, project_id, role, added_at_ms";

pub(super) fn add_project_path_to_group(
    connection: &mut Connection,
    group_id: &str,
    absolute_path: &Path,
    role: &str,
    key: &str,
) -> Result<GroupProjectMember, HubStoreError> {
    if !absolute_path.is_absolute() {
        return Err(HubStoreError::Conflict {
            entity: HubEntity::Project,
            message: "project path must be canonical and absolute".into(),
        });
    }
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(super::unavailable)?;
    ensure_group(&transaction, group_id)?;
    let path = rows::path_text(absolute_path)?;
    if let Some(member) = member_by_key(&transaction, key)? {
        ensure_replay(&transaction, &member, group_id, path, role)?;
        transaction
            .commit()
            .map_err(|error| write_error(HubEntity::GroupProjectMember, error))?;
        return Ok(member);
    }
    let project = find_or_create_project(&transaction, absolute_path, path)?;
    if let Some(member) = member_by_pair(&transaction, group_id, &project.id)? {
        if member.role != role {
            return Err(conflict("project is already linked with a different role"));
        }
        return Err(conflict(
            "project link already exists under a different idempotency key",
        ));
    }
    let member = new_member(group_id, project.id, role)?;
    insert_member(&transaction, &member, key)?;
    transaction
        .commit()
        .map_err(|error| write_error(HubEntity::GroupProjectMember, error))?;
    Ok(member)
}

fn find_or_create_project(
    transaction: &Transaction<'_>,
    absolute_path: &Path,
    path: &str,
) -> Result<Project, HubStoreError> {
    if let Some(project) = project_by_path(transaction, path)? {
        return Ok(project);
    }
    let project = Project {
        id: rows::new_id(transaction, "project")?,
        name: project_name(absolute_path),
        path: absolute_path.to_path_buf(),
        created_at_ms: rows::now_ms()?,
    };
    transaction
        .execute(
            "INSERT INTO projects(id,name,canonical_path,created_at_ms) VALUES(?1,?2,?3,?4)",
            params![
                project.id,
                project.name,
                path,
                to_i64(project.created_at_ms)?
            ],
        )
        .map_err(|error| write_error(HubEntity::Project, error))?;
    Ok(project)
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

fn ensure_replay(
    transaction: &Transaction<'_>,
    member: &GroupProjectMember,
    group_id: &str,
    path: &str,
    role: &str,
) -> Result<(), HubStoreError> {
    if member.group_id != group_id || member.role != role {
        return Err(conflict(
            "idempotency key was reused with different membership data",
        ));
    }
    let existing_path: String = transaction
        .query_row(
            "SELECT canonical_path FROM projects WHERE id = ?1",
            [&member.project_id],
            |row| row.get(0),
        )
        .map_err(read_error)?;
    if existing_path == path {
        Ok(())
    } else {
        Err(conflict(
            "idempotency key was reused with a different project path",
        ))
    }
}

fn ensure_group(transaction: &Transaction<'_>, group_id: &str) -> Result<(), HubStoreError> {
    let found: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM groups WHERE id = ?1)",
            [group_id],
            |row| row.get(0),
        )
        .map_err(read_error)?;
    found.then_some(()).ok_or_else(|| HubStoreError::NotFound {
        entity: HubEntity::Group,
        id: group_id.into(),
    })
}

fn new_member(
    group_id: &str,
    project_id: String,
    role: &str,
) -> Result<GroupProjectMember, HubStoreError> {
    Ok(GroupProjectMember {
        group_id: group_id.into(),
        project_id,
        role: role.into(),
        added_at_ms: rows::now_ms()?,
    })
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

fn conflict(message: impl Into<String>) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupProjectMember,
        message: message.into(),
    }
}
