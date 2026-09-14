use crate::runtime_domain::{GroupProjectMember, SessionGroup};

use rusqlite::{Connection, OptionalExtension, Transaction, params};

use super::{
    HubEntity, HubStoreError, begin, conflict, ensure_exists, read_error, rows, to_i64, write_error,
};

const GROUP_COLUMNS: &str = "id, name, created_at_ms";
const MEMBER_COLUMNS: &str = "group_id, project_id, role, added_at_ms";

pub(in crate::sqlite_hub) fn create_group(
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

pub(in crate::sqlite_hub) fn add_project_to_group(
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
