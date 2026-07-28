use crate::runtime_domain::{
    GroupContextMember, GroupContextPolicy, GroupContextProvenance, GroupContextSlice, HubEntity,
    HubStoreError, MAX_GROUP_CONTEXT_CONTENT_BYTES, MAX_GROUP_CONTEXT_GROUP_CONVERSATIONS,
    MAX_GROUP_CONTEXT_MEMBERS, MAX_GROUP_CONTEXT_PROJECT_CONVERSATIONS,
    MAX_GROUP_CONTEXT_PROMPT_EXCERPT_BYTES, MAX_GROUP_CONTEXT_PROMPTS_PER_CONVERSATION,
    SessionGroup,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use super::{
    Conversation,
    group_context_build::{LoadedConversation, LoadedPrompt, build_slice, digest_bytes},
    read_error, rows,
};

const CONVERSATION_COLUMNS: &str = "id, scope_kind, scope_id, title, created_at_ms, updated_at_ms";
const CONTEXT_PROMPTS_SQL: &str = "WITH causal AS (
  SELECT p.id, p.role, p.content, p.created_at_ms,
         p.rowid AS prompt_rowid,
         COALESCE(source.rowid, p.rowid) AS anchor_rowid,
         CASE WHEN w.run_id IS NULL THEN 0 ELSE 1 END AS run_answer,
         r.rowid AS run_rowid
  FROM prompts p
  LEFT JOIN run_assistant_prompts w ON w.prompt_id = p.id
  LEFT JOIN runs r ON r.id = w.run_id
  LEFT JOIN prompts source ON source.id = r.prompt_id
  WHERE p.conversation_id = ?1
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
), budgets AS (
  SELECT anchor_rowid, COALESCE(SUM(group_size) OVER (
    ORDER BY anchor_rowid DESC
    ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING
  ), 0) AS newer_size
  FROM anchor_groups
)
SELECT r.id, r.role, r.content, r.created_at_ms, r.anchor_rowid
FROM ranked r
JOIN budgets b ON b.anchor_rowid = r.anchor_rowid
WHERE b.newer_size < ?2
  AND (r.run_answer = 0 OR r.anchor_rank <= ?2 - b.newer_size - 1)
ORDER BY r.anchor_rowid DESC, r.run_answer DESC,
         r.run_rowid DESC, r.prompt_rowid DESC
LIMIT ?2";

pub(super) fn load(
    connection: &mut Connection,
    group_id: &str,
    policy: &GroupContextPolicy,
) -> Result<GroupContextSlice, HubStoreError> {
    load_with_hook(connection, group_id, policy, || {})
}

#[cfg(test)]
pub(super) fn load_after_group(
    connection: &mut Connection,
    group_id: &str,
    policy: &GroupContextPolicy,
    after_group: impl FnOnce(),
) -> Result<GroupContextSlice, HubStoreError> {
    load_with_hook(connection, group_id, policy, after_group)
}

fn load_with_hook(
    connection: &mut Connection,
    group_id: &str,
    policy: &GroupContextPolicy,
    after_group: impl FnOnce(),
) -> Result<GroupContextSlice, HubStoreError> {
    validate_policy(policy)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let slice = load_with_group_hook(&transaction, group_id, policy, after_group)?;
    transaction.commit().map_err(read_error)?;
    Ok(slice)
}

fn load_with_group_hook(
    connection: &Connection,
    group_id: &str,
    policy: &GroupContextPolicy,
    after_group: impl FnOnce(),
) -> Result<GroupContextSlice, HubStoreError> {
    let group = load_group(connection, group_id)?;
    after_group();
    load_locked(connection, group, group_id, policy)
}

pub(super) fn load_in_snapshot(
    connection: &Connection,
    group_id: &str,
    policy: &GroupContextPolicy,
) -> Result<GroupContextSlice, HubStoreError> {
    validate_policy(policy)?;
    load_with_group_hook(connection, group_id, policy, || {})
}

fn load_locked(
    connection: &Connection,
    group: SessionGroup,
    group_id: &str,
    policy: &GroupContextPolicy,
) -> Result<GroupContextSlice, HubStoreError> {
    let members = load_members(connection, group_id, policy.max_members)?;
    let (conversations, omitted_conversations) =
        load_all_conversations(connection, group_id, &members, policy)?;
    build_slice(
        policy.clone(),
        group,
        members,
        conversations,
        omitted_conversations,
    )
}

pub(super) fn validate_policy(policy: &GroupContextPolicy) -> Result<(), HubStoreError> {
    let valid = (1..=MAX_GROUP_CONTEXT_MEMBERS).contains(&policy.max_members)
        && (1..=MAX_GROUP_CONTEXT_GROUP_CONVERSATIONS).contains(&policy.max_group_conversations)
        && (1..=MAX_GROUP_CONTEXT_PROJECT_CONVERSATIONS)
            .contains(&policy.max_project_conversations_per_member)
        && (1..=MAX_GROUP_CONTEXT_PROMPTS_PER_CONVERSATION)
            .contains(&policy.max_prompts_per_conversation)
        && (1..=MAX_GROUP_CONTEXT_PROMPT_EXCERPT_BYTES).contains(&policy.max_prompt_excerpt_bytes)
        && (1..=MAX_GROUP_CONTEXT_CONTENT_BYTES).contains(&policy.max_total_content_bytes);
    if valid {
        return Ok(());
    }
    Err(HubStoreError::Conflict {
        entity: HubEntity::Group,
        message: "Group context policy is outside its supported bounds".into(),
    })
}

fn load_group(connection: &Connection, group_id: &str) -> Result<SessionGroup, HubStoreError> {
    connection
        .query_row(
            "SELECT id, name, created_at_ms FROM groups WHERE id = ?1",
            [group_id],
            rows::group,
        )
        .optional()
        .map_err(read_error)?
        .ok_or_else(|| HubStoreError::NotFound {
            entity: HubEntity::Group,
            id: group_id.into(),
        })
}

fn load_members(
    connection: &Connection,
    group_id: &str,
    max_members: usize,
) -> Result<Vec<GroupContextMember>, HubStoreError> {
    let mut statement = connection
        .prepare(
            "SELECT gp.project_id, p.name, gp.role
             FROM group_projects gp
             LEFT JOIN projects p ON p.id = gp.project_id
             WHERE gp.group_id = ?1
             ORDER BY gp.role COLLATE NOCASE, gp.project_id
             LIMIT ?2",
        )
        .map_err(read_error)?;
    let row_limit =
        i64::try_from(max_members.saturating_add(1)).map_err(|error| HubStoreError::Corrupt {
            message: format!("invalid Group context member limit: {error}"),
        })?;
    let rows = statement
        .query_map(params![group_id, row_limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(read_error)?;
    let raw = rows.collect::<Result<Vec<_>, _>>().map_err(read_error)?;
    validate_member_count(raw.len(), max_members)?;
    raw.into_iter().map(map_member).collect()
}

fn validate_member_count(count: usize, max_members: usize) -> Result<(), HubStoreError> {
    if count <= max_members {
        return Ok(());
    }
    Err(HubStoreError::Conflict {
        entity: HubEntity::GroupProjectMember,
        message: format!("Group has more than {max_members} members; context cannot include all"),
    })
}

fn map_member(raw: (String, Option<String>, String)) -> Result<GroupContextMember, HubStoreError> {
    let (project_id, project_name, role) = raw;
    let project_name = project_name.ok_or_else(|| HubStoreError::Corrupt {
        message: format!("Group member Project '{project_id}' is missing"),
    })?;
    Ok(GroupContextMember {
        project_id,
        project_name,
        role,
    })
}

fn load_all_conversations(
    connection: &Connection,
    group_id: &str,
    members: &[GroupContextMember],
    policy: &GroupContextPolicy,
) -> Result<(Vec<LoadedConversation>, usize), HubStoreError> {
    let provenance = GroupContextProvenance::Group {
        group_id: group_id.into(),
    };
    let (mut conversations, mut omitted) = load_scope_conversations(
        connection,
        "group",
        group_id,
        policy.max_group_conversations,
        &provenance,
        policy.max_prompts_per_conversation,
    )?;
    for member in members {
        let (mut project, project_omitted) =
            load_project_conversations(connection, member, policy)?;
        conversations.append(&mut project);
        omitted = omitted.saturating_add(project_omitted);
    }
    Ok((conversations, omitted))
}

fn load_project_conversations(
    connection: &Connection,
    member: &GroupContextMember,
    policy: &GroupContextPolicy,
) -> Result<(Vec<LoadedConversation>, usize), HubStoreError> {
    let provenance = GroupContextProvenance::Project {
        project_id: member.project_id.clone(),
        role: member.role.clone(),
    };
    load_scope_conversations(
        connection,
        "project",
        &member.project_id,
        policy.max_project_conversations_per_member,
        &provenance,
        policy.max_prompts_per_conversation,
    )
}

fn load_scope_conversations(
    connection: &Connection,
    scope_kind: &str,
    scope_id: &str,
    limit: usize,
    provenance: &GroupContextProvenance,
    prompt_limit: usize,
) -> Result<(Vec<LoadedConversation>, usize), HubStoreError> {
    let total = count_nonempty_conversations(connection, scope_kind, scope_id)?;
    let selected = select_conversations(connection, scope_kind, scope_id, limit)?;
    let mut loaded = Vec::with_capacity(selected.len());
    for conversation in selected {
        loaded.push(load_conversation(
            connection,
            conversation,
            provenance.clone(),
            prompt_limit,
        )?);
    }
    Ok((loaded, total.saturating_sub(limit.min(total))))
}

fn count_nonempty_conversations(
    connection: &Connection,
    scope_kind: &str,
    scope_id: &str,
) -> Result<usize, HubStoreError> {
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM conversations c
             WHERE c.scope_kind = ?1 AND c.scope_id = ?2
               AND EXISTS(SELECT 1 FROM prompts p WHERE p.conversation_id = c.id)",
            params![scope_kind, scope_id],
            |row| row.get(0),
        )
        .map_err(read_error)?;
    usize::try_from(count).map_err(|error| HubStoreError::Corrupt {
        message: format!("invalid Group context Conversation count: {error}"),
    })
}

fn select_conversations(
    connection: &Connection,
    scope_kind: &str,
    scope_id: &str,
    limit: usize,
) -> Result<Vec<Conversation>, HubStoreError> {
    let sql = format!(
        "SELECT {CONVERSATION_COLUMNS} FROM conversations c
         WHERE c.scope_kind = ?1 AND c.scope_id = ?2
           AND EXISTS(SELECT 1 FROM prompts p WHERE p.conversation_id = c.id)
         ORDER BY c.updated_at_ms DESC, c.id DESC LIMIT ?3"
    );
    let limit = i64::try_from(limit).map_err(|error| HubStoreError::Corrupt {
        message: error.to_string(),
    })?;
    let mut statement = connection.prepare(&sql).map_err(read_error)?;
    statement
        .query_map(params![scope_kind, scope_id, limit], rows::conversation)
        .map_err(read_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(read_error)
}

fn load_conversation(
    connection: &Connection,
    conversation: Conversation,
    provenance: GroupContextProvenance,
    limit: usize,
) -> Result<LoadedConversation, HubStoreError> {
    super::read::validate_causal_associations(connection, &conversation.id)?;
    validate_prompt_roles(connection, &conversation.id)?;
    let total = count_prompts(connection, &conversation.id)?;
    let prompts = select_prompts(connection, &conversation.id, limit)?;
    Ok(LoadedConversation {
        conversation,
        provenance,
        omitted_prompt_count: total.saturating_sub(prompts.len()),
        prompts,
    })
}

fn validate_prompt_roles(
    connection: &Connection,
    conversation_id: &str,
) -> Result<(), HubStoreError> {
    let invalid = connection
        .query_row(
            "SELECT id, role FROM prompts
             WHERE conversation_id = ?1 AND role NOT IN ('user', 'assistant')
             ORDER BY rowid LIMIT 1",
            [conversation_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(read_error)?;
    if let Some((id, role)) = invalid {
        return Err(HubStoreError::Corrupt {
            message: format!("Prompt '{id}' has unsupported Group context role '{role}'"),
        });
    }
    Ok(())
}

fn count_prompts(connection: &Connection, conversation_id: &str) -> Result<usize, HubStoreError> {
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM prompts WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get(0),
        )
        .map_err(read_error)?;
    usize::try_from(count).map_err(|error| HubStoreError::Corrupt {
        message: format!("invalid Group context Prompt count: {error}"),
    })
}

fn select_prompts(
    connection: &Connection,
    conversation_id: &str,
    limit: usize,
) -> Result<Vec<LoadedPrompt>, HubStoreError> {
    let limit = i64::try_from(limit).map_err(|error| HubStoreError::Corrupt {
        message: error.to_string(),
    })?;
    let mut statement = connection
        .prepare(CONTEXT_PROMPTS_SQL)
        .map_err(read_error)?;
    let rows = statement
        .query_map(params![conversation_id, limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })
        .map_err(read_error)?;
    rows.map(|row| map_prompt(row.map_err(read_error)?))
        .collect()
}

fn map_prompt(raw: (String, String, String, i64, i64)) -> Result<LoadedPrompt, HubStoreError> {
    let (id, role, content, created_at_ms, anchor_rowid) = raw;
    if !matches!(role.as_str(), "user" | "assistant") {
        return Err(HubStoreError::Corrupt {
            message: format!("Prompt '{id}' has unsupported Group context role '{role}'"),
        });
    }
    let created_at_ms = u64::try_from(created_at_ms).map_err(|error| HubStoreError::Corrupt {
        message: format!("Prompt '{id}' has invalid creation time: {error}"),
    })?;
    let content_sha256 = digest_bytes(content.as_bytes());
    Ok(LoadedPrompt {
        id,
        role,
        content,
        created_at_ms,
        anchor_rowid,
        content_sha256,
        excerpt: String::new(),
    })
}
