use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use super::{
    Conversation, ConversationScope, GroupProjectMember, HubEntity, HubSnapshot, HubStoreError,
    Project, PromptRecord, SessionGroup, read_error, rows,
};

const PROJECT_COLUMNS: &str = "id, name, canonical_path, created_at_ms";
const CONVERSATION_COLUMNS: &str = "id, scope_kind, scope_id, title, created_at_ms, updated_at_ms";
const PROMPT_COLUMNS: &str = "id, conversation_id, role, content, idempotency_key, created_at_ms";
const GROUP_COLUMNS: &str = "id, name, created_at_ms";
const MEMBER_COLUMNS: &str = "group_id, project_id, role, added_at_ms";
type SnapshotParts = (Vec<Project>, Vec<SessionGroup>, Vec<GroupProjectMember>);
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

pub(super) fn snapshot(
    connection: &mut Connection,
    scope: &ConversationScope,
) -> Result<HubSnapshot, HubStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let snapshot = snapshot_locked(&transaction, scope)?;
    transaction.commit().map_err(read_error)?;
    Ok(snapshot)
}

fn snapshot_locked(
    connection: &Connection,
    scope: &ConversationScope,
) -> Result<HubSnapshot, HubStoreError> {
    let (projects, groups, members) = match scope {
        ConversationScope::Global => (
            list_projects(connection)?,
            list_groups(connection)?,
            list_group_members(connection)?,
        ),
        ConversationScope::Project(id) => project_snapshot_parts(connection, id)?,
        ConversationScope::Group(id) => group_snapshot_parts(connection, id)?,
    };
    Ok(HubSnapshot {
        scope: scope.clone(),
        projects,
        conversations: list_conversations(connection, scope)?,
        groups,
        group_project_members: members,
    })
}

pub(super) fn list_conversations(
    connection: &Connection,
    scope: &ConversationScope,
) -> Result<Vec<Conversation>, HubStoreError> {
    if matches!(scope, ConversationScope::Global) {
        return query_conversations(
            connection,
            &format!(
                "SELECT {CONVERSATION_COLUMNS} FROM conversations
                 ORDER BY updated_at_ms DESC, id DESC"
            ),
            [],
        );
    }
    ensure_scope_exists(connection, scope)?;
    let (kind, id) = rows::scope_parts(scope);
    query_conversations(
        connection,
        &format!(
            "SELECT {CONVERSATION_COLUMNS} FROM conversations
             WHERE scope_kind = ?1 AND scope_id = ?2
             ORDER BY updated_at_ms DESC, id DESC"
        ),
        params![kind, id],
    )
}

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

pub(super) fn list_groups(connection: &Connection) -> Result<Vec<SessionGroup>, HubStoreError> {
    let mut statement = connection
        .prepare(&format!(
            "SELECT {GROUP_COLUMNS} FROM groups ORDER BY name COLLATE NOCASE, id"
        ))
        .map_err(read_error)?;
    statement
        .query_map([], rows::group)
        .map_err(read_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(read_error)
}

fn list_projects(connection: &Connection) -> Result<Vec<Project>, HubStoreError> {
    let mut statement = connection
        .prepare(&format!(
            "SELECT {PROJECT_COLUMNS} FROM projects
             ORDER BY name COLLATE NOCASE, id"
        ))
        .map_err(read_error)?;
    statement
        .query_map([], rows::project)
        .map_err(read_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(read_error)
}

fn project_snapshot_parts(
    connection: &Connection,
    project_id: &str,
) -> Result<SnapshotParts, HubStoreError> {
    let project = find_project(connection, project_id)?
        .ok_or_else(|| not_found(HubEntity::Project, project_id))?;
    let groups = project_groups(connection, project_id)?;
    let members = members_for_groups(connection, &groups)?;
    Ok((vec![project], groups, members))
}

fn group_snapshot_parts(
    connection: &Connection,
    group_id: &str,
) -> Result<SnapshotParts, HubStoreError> {
    let group =
        find_group(connection, group_id)?.ok_or_else(|| not_found(HubEntity::Group, group_id))?;
    let members = members_for_group(connection, group_id)?;
    let projects = projects_for_members(connection, &members)?;
    Ok((projects, vec![group], members))
}

fn find_project(
    connection: &Connection,
    project_id: &str,
) -> Result<Option<Project>, HubStoreError> {
    connection
        .query_row(
            &format!("SELECT {PROJECT_COLUMNS} FROM projects WHERE id = ?1"),
            [project_id],
            rows::project,
        )
        .optional()
        .map_err(read_error)
}

fn find_group(
    connection: &Connection,
    group_id: &str,
) -> Result<Option<SessionGroup>, HubStoreError> {
    connection
        .query_row(
            &format!("SELECT {GROUP_COLUMNS} FROM groups WHERE id = ?1"),
            [group_id],
            rows::group,
        )
        .optional()
        .map_err(read_error)
}

fn ensure_scope_exists(
    connection: &Connection,
    scope: &ConversationScope,
) -> Result<(), HubStoreError> {
    let found = match scope {
        ConversationScope::Global => return Ok(()),
        ConversationScope::Project(id) => find_project(connection, id)?.is_some(),
        ConversationScope::Group(id) => find_group(connection, id)?.is_some(),
    };
    if found {
        return Ok(());
    }
    let (entity, id) = match scope {
        ConversationScope::Project(id) => (HubEntity::Project, id.as_str()),
        ConversationScope::Group(id) => (HubEntity::Group, id.as_str()),
        ConversationScope::Global => unreachable!("global scope returned above"),
    };
    Err(not_found(entity, id))
}

fn ensure_conversation_exists(
    connection: &Connection,
    conversation_id: &str,
) -> Result<(), HubStoreError> {
    let found = connection
        .query_row(
            "SELECT 1 FROM conversations WHERE id = ?1",
            [conversation_id],
            |_| Ok(()),
        )
        .optional()
        .map_err(read_error)?
        .is_some();
    if found {
        Ok(())
    } else {
        Err(not_found(HubEntity::Conversation, conversation_id))
    }
}

fn project_groups(
    connection: &Connection,
    project_id: &str,
) -> Result<Vec<SessionGroup>, HubStoreError> {
    let sql = format!(
        "SELECT g.{GROUP_COLUMNS} FROM groups g
         JOIN group_projects gp ON gp.group_id = g.id
         WHERE gp.project_id = ?1 ORDER BY g.name COLLATE NOCASE, g.id"
    );
    let mut statement = connection.prepare(&sql).map_err(read_error)?;
    statement
        .query_map([project_id], rows::group)
        .map_err(read_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(read_error)
}

fn list_group_members(connection: &Connection) -> Result<Vec<GroupProjectMember>, HubStoreError> {
    query_members(
        connection,
        &format!(
            "SELECT {MEMBER_COLUMNS} FROM group_projects
             ORDER BY group_id, role, project_id"
        ),
        [],
    )
}

fn members_for_group(
    connection: &Connection,
    group_id: &str,
) -> Result<Vec<GroupProjectMember>, HubStoreError> {
    query_members(
        connection,
        &format!(
            "SELECT {MEMBER_COLUMNS} FROM group_projects
             WHERE group_id = ?1 ORDER BY role, project_id"
        ),
        [group_id],
    )
}

fn members_for_groups(
    connection: &Connection,
    groups: &[SessionGroup],
) -> Result<Vec<GroupProjectMember>, HubStoreError> {
    let mut members = Vec::new();
    for group in groups {
        members.extend(members_for_group(connection, &group.id)?);
    }
    Ok(members)
}

fn projects_for_members(
    connection: &Connection,
    members: &[GroupProjectMember],
) -> Result<Vec<Project>, HubStoreError> {
    let mut projects = Vec::with_capacity(members.len());
    for member in members {
        let project = find_project(connection, &member.project_id)?
            .ok_or_else(|| not_found(HubEntity::Project, &member.project_id))?;
        projects.push(project);
    }
    Ok(projects)
}

fn query_conversations<P>(
    connection: &Connection,
    sql: &str,
    parameters: P,
) -> Result<Vec<Conversation>, HubStoreError>
where
    P: rusqlite::Params,
{
    let mut statement = connection.prepare(sql).map_err(read_error)?;
    statement
        .query_map(parameters, rows::conversation)
        .map_err(read_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(read_error)
}

fn query_members<P>(
    connection: &Connection,
    sql: &str,
    parameters: P,
) -> Result<Vec<GroupProjectMember>, HubStoreError>
where
    P: rusqlite::Params,
{
    let mut statement = connection.prepare(sql).map_err(read_error)?;
    statement
        .query_map(parameters, rows::group_member)
        .map_err(read_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(read_error)
}

fn not_found(entity: HubEntity, id: &str) -> HubStoreError {
    HubStoreError::NotFound {
        entity,
        id: id.into(),
    }
}
