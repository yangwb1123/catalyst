use std::path::Path;

use forge_runtime_domain::{
    Conversation, ConversationScope, GroupProjectMember, HubEntity, HubSnapshot, HubStoreError,
    Project, PromptRecord, SessionGroup,
};

use super::MemoryState;

pub(super) fn snapshot_from(state: &MemoryState, scope: &ConversationScope) -> HubSnapshot {
    let projects = visible_projects(state, scope);
    let conversations = match scope {
        ConversationScope::Global => state.conversations.clone(),
        _ => state
            .conversations
            .iter()
            .filter(|item| &item.scope == scope)
            .cloned()
            .collect(),
    };
    HubSnapshot {
        scope: scope.clone(),
        projects,
        conversations,
        groups: state.groups.clone(),
        group_project_members: state.members.clone(),
    }
}

fn visible_projects(state: &MemoryState, scope: &ConversationScope) -> Vec<Project> {
    match scope {
        ConversationScope::Global => state.projects.clone(),
        ConversationScope::Project(id) => state
            .projects
            .iter()
            .filter(|item| &item.id == id)
            .cloned()
            .collect(),
        ConversationScope::Group(id) => state
            .projects
            .iter()
            .filter(|project| {
                state
                    .members
                    .iter()
                    .any(|member| &member.group_id == id && member.project_id == project.id)
            })
            .cloned()
            .collect(),
    }
}

pub(super) fn find_conversation_by_key(state: &MemoryState, key: &str) -> Option<Conversation> {
    let id = &state
        .conversation_keys
        .iter()
        .find(|(candidate, _)| candidate == key)?
        .1;
    state
        .conversations
        .iter()
        .find(|item| &item.id == id)
        .cloned()
}

pub(super) fn find_group_by_key(state: &MemoryState, key: &str) -> Option<SessionGroup> {
    let id = &state
        .group_keys
        .iter()
        .find(|(candidate, _)| candidate == key)?
        .1;
    state.groups.iter().find(|item| &item.id == id).cloned()
}

pub(super) fn same_conversation(
    existing: Conversation,
    scope: &ConversationScope,
    title: &str,
) -> Result<Conversation, HubStoreError> {
    if existing.scope == *scope && existing.title == title {
        return Ok(existing);
    }
    Err(conflict(HubEntity::Conversation))
}

pub(super) fn same_prompt(
    existing: &PromptRecord,
    conversation_id: &str,
    role: &str,
    content: &str,
) -> Result<PromptRecord, HubStoreError> {
    if existing.conversation_id == conversation_id
        && existing.role == role
        && existing.content == content
    {
        return Ok(existing.clone());
    }
    Err(conflict(HubEntity::Prompt))
}

pub(super) fn same_group(
    existing: SessionGroup,
    name: &str,
) -> Result<SessionGroup, HubStoreError> {
    if existing.name == name {
        return Ok(existing);
    }
    Err(conflict(HubEntity::Group))
}

pub(super) fn same_member(
    existing: &GroupProjectMember,
    group_id: &str,
    project_id: &str,
    role: &str,
) -> Result<GroupProjectMember, HubStoreError> {
    if existing.group_id == group_id && existing.project_id == project_id && existing.role == role {
        return Ok(existing.clone());
    }
    Err(conflict(HubEntity::GroupProjectMember))
}

pub(super) fn ensure_scope_exists(
    state: &MemoryState,
    scope: &ConversationScope,
) -> Result<(), HubStoreError> {
    match scope {
        ConversationScope::Global => Ok(()),
        ConversationScope::Project(id) => require_project(state, id),
        ConversationScope::Group(id) => require_group(state, id),
    }
}

pub(super) fn require_group_and_project(
    state: &MemoryState,
    group_id: &str,
    project_id: &str,
) -> Result<(), HubStoreError> {
    require_group(state, group_id)?;
    require_project(state, project_id)
}

pub(super) fn require_conversation(state: &MemoryState, id: &str) -> Result<(), HubStoreError> {
    require_entity(
        state.conversations.iter().any(|item| item.id == id),
        HubEntity::Conversation,
        id,
    )
}

fn require_project(state: &MemoryState, id: &str) -> Result<(), HubStoreError> {
    require_entity(
        state.projects.iter().any(|item| item.id == id),
        HubEntity::Project,
        id,
    )
}

pub(super) fn require_group(state: &MemoryState, id: &str) -> Result<(), HubStoreError> {
    require_entity(
        state.groups.iter().any(|item| item.id == id),
        HubEntity::Group,
        id,
    )
}

fn require_entity(found: bool, entity: HubEntity, id: &str) -> Result<(), HubStoreError> {
    if found {
        return Ok(());
    }
    Err(HubStoreError::NotFound {
        entity,
        id: id.into(),
    })
}

pub(super) fn conflict(entity: HubEntity) -> HubStoreError {
    HubStoreError::Conflict {
        entity,
        message: "idempotency key was reused with different request data".into(),
    }
}

pub(super) fn touch_conversation(state: &mut MemoryState, id: &str, updated_at_ms: u64) {
    if let Some(conversation) = state.conversations.iter_mut().find(|item| item.id == id) {
        conversation.updated_at_ms = updated_at_ms;
    }
}

pub(super) fn history_boundary_index(
    state: &MemoryState,
    conversation_id: &str,
    prompt_id: &str,
) -> Result<usize, HubStoreError> {
    state
        .prompts
        .iter()
        .position(|item| item.id == prompt_id && item.conversation_id == conversation_id)
        .ok_or_else(|| HubStoreError::NotFound {
            entity: HubEntity::Prompt,
            id: prompt_id.into(),
        })
}

pub(super) fn require_user_boundary(boundary: &PromptRecord) -> Result<(), HubStoreError> {
    if boundary.role != "user" {
        return Err(HubStoreError::Conflict {
            entity: HubEntity::Prompt,
            message: "history boundary must be a user prompt".into(),
        });
    }
    Ok(())
}

pub(super) fn project_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project")
        .to_owned()
}
