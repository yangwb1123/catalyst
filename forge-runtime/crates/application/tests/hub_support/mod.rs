use std::{
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
};

use forge_runtime_domain::{
    Conversation, ConversationScope, GroupProjectMember, HubEntity, HubSnapshot, HubStore,
    HubStoreError, Project, PromptRecord, SessionGroup,
};

mod atomic_memory;

#[derive(Default)]
pub struct MemoryHubStore {
    state: Mutex<MemoryState>,
}

#[derive(Default)]
struct MemoryState {
    sequence: u64,
    projects: Vec<Project>,
    conversations: Vec<Conversation>,
    conversation_keys: Vec<(String, String)>,
    prompts: Vec<PromptRecord>,
    groups: Vec<SessionGroup>,
    group_keys: Vec<(String, String)>,
    members: Vec<GroupProjectMember>,
    member_keys: Vec<(String, GroupProjectMember)>,
}

impl MemoryState {
    fn identity(&mut self, prefix: &str) -> (String, u64) {
        self.sequence += 1;
        (format!("{prefix}-{}", self.sequence), self.sequence)
    }
}

impl MemoryHubStore {
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn state(&self) -> Result<MutexGuard<'_, MemoryState>, HubStoreError> {
        self.state.lock().map_err(|_| HubStoreError::Unavailable {
            message: "memory store lock poisoned".into(),
        })
    }
}

impl HubStore for MemoryHubStore {
    fn open_project(&self, absolute_path: &Path) -> Result<Project, HubStoreError> {
        let mut state = self.state()?;
        if let Some(project) = state
            .projects
            .iter()
            .find(|item| item.path == absolute_path)
        {
            return Ok(project.clone());
        }
        let (id, created_at_ms) = state.identity("project");
        let name = project_name(absolute_path);
        let project = Project {
            id,
            name,
            path: absolute_path.to_path_buf(),
            created_at_ms,
        };
        state.projects.push(project.clone());
        Ok(project)
    }

    fn snapshot(&self, scope: &ConversationScope) -> Result<HubSnapshot, HubStoreError> {
        let state = self.state()?;
        Ok(snapshot_from(&state, scope))
    }

    fn create_conversation(
        &self,
        scope: &ConversationScope,
        title: &str,
        idempotency_key: &str,
    ) -> Result<Conversation, HubStoreError> {
        let mut state = self.state()?;
        if let Some(item) = find_conversation_by_key(&state, idempotency_key) {
            return same_conversation(item, scope, title);
        }
        ensure_scope_exists(&state, scope)?;
        let (id, created_at_ms) = state.identity("conversation");
        let conversation = Conversation {
            id,
            scope: scope.clone(),
            title: title.into(),
            created_at_ms,
            updated_at_ms: created_at_ms,
        };
        state
            .conversation_keys
            .push((idempotency_key.into(), conversation.id.clone()));
        state.conversations.push(conversation.clone());
        Ok(conversation)
    }

    fn list_conversations(
        &self,
        scope: &ConversationScope,
    ) -> Result<Vec<Conversation>, HubStoreError> {
        let state = self.state()?;
        Ok(state
            .conversations
            .iter()
            .filter(|item| &item.scope == scope)
            .cloned()
            .collect())
    }

    fn append_prompt(
        &self,
        conversation_id: &str,
        role: &str,
        content: &str,
        idempotency_key: &str,
    ) -> Result<PromptRecord, HubStoreError> {
        let mut state = self.state()?;
        if let Some(prompt) = state
            .prompts
            .iter()
            .find(|item| item.idempotency_key == idempotency_key)
        {
            return same_prompt(prompt, conversation_id, role, content);
        }
        require_conversation(&state, conversation_id)?;
        let (id, created_at_ms) = state.identity("prompt");
        let prompt = PromptRecord {
            id,
            conversation_id: conversation_id.into(),
            role: role.into(),
            content: content.into(),
            idempotency_key: idempotency_key.into(),
            created_at_ms,
        };
        state.prompts.push(prompt.clone());
        touch_conversation(&mut state, conversation_id, created_at_ms);
        Ok(prompt)
    }

    fn list_prompts(
        &self,
        conversation_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<PromptRecord>, HubStoreError> {
        let state = self.state()?;
        let mut prompts: Vec<_> = state
            .prompts
            .iter()
            .filter(|item| conversation_id.is_none_or(|id| item.conversation_id == id))
            .cloned()
            .collect();
        prompts.sort_by(|left, right| {
            right
                .created_at_ms
                .cmp(&left.created_at_ms)
                .then_with(|| right.id.cmp(&left.id))
        });
        prompts.truncate(limit);
        Ok(prompts)
    }

    fn create_group(
        &self,
        name: &str,
        idempotency_key: &str,
    ) -> Result<SessionGroup, HubStoreError> {
        let mut state = self.state()?;
        if let Some(group) = find_group_by_key(&state, idempotency_key) {
            return same_group(group, name);
        }
        let (id, created_at_ms) = state.identity("group");
        let group = SessionGroup {
            id,
            name: name.into(),
            created_at_ms,
        };
        state
            .group_keys
            .push((idempotency_key.into(), group.id.clone()));
        state.groups.push(group.clone());
        Ok(group)
    }

    fn list_groups(&self) -> Result<Vec<SessionGroup>, HubStoreError> {
        Ok(self.state()?.groups.clone())
    }

    fn add_project_to_group(
        &self,
        group_id: &str,
        project_id: &str,
        role: &str,
        idempotency_key: &str,
    ) -> Result<GroupProjectMember, HubStoreError> {
        let mut state = self.state()?;
        if let Some((_, member)) = state
            .member_keys
            .iter()
            .find(|(key, _)| key == idempotency_key)
        {
            return same_member(member, group_id, project_id, role);
        }
        if let Some(member) = state
            .members
            .iter()
            .find(|item| item.group_id == group_id && item.project_id == project_id)
        {
            same_member(member, group_id, project_id, role)?;
            return Err(conflict(HubEntity::GroupProjectMember));
        }
        require_group_and_project(&state, group_id, project_id)?;
        let (_, added_at_ms) = state.identity("member");
        let member = GroupProjectMember {
            group_id: group_id.into(),
            project_id: project_id.into(),
            role: role.into(),
            added_at_ms,
        };
        state
            .member_keys
            .push((idempotency_key.into(), member.clone()));
        state.members.push(member.clone());
        Ok(member)
    }

    fn add_project_path_to_group(
        &self,
        group_id: &str,
        absolute_path: &Path,
        role: &str,
        idempotency_key: &str,
    ) -> Result<GroupProjectMember, HubStoreError> {
        let mut state = self.state()?;
        atomic_memory::link_project_path(&mut state, group_id, absolute_path, role, idempotency_key)
    }
}

fn snapshot_from(state: &MemoryState, scope: &ConversationScope) -> HubSnapshot {
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

fn find_conversation_by_key(state: &MemoryState, key: &str) -> Option<Conversation> {
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

fn find_group_by_key(state: &MemoryState, key: &str) -> Option<SessionGroup> {
    let id = &state
        .group_keys
        .iter()
        .find(|(candidate, _)| candidate == key)?
        .1;
    state.groups.iter().find(|item| &item.id == id).cloned()
}

fn same_conversation(
    existing: Conversation,
    scope: &ConversationScope,
    title: &str,
) -> Result<Conversation, HubStoreError> {
    if existing.scope == *scope && existing.title == title {
        return Ok(existing);
    }
    Err(conflict(HubEntity::Conversation))
}

fn same_prompt(
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

fn same_group(existing: SessionGroup, name: &str) -> Result<SessionGroup, HubStoreError> {
    if existing.name == name {
        return Ok(existing);
    }
    Err(conflict(HubEntity::Group))
}

fn same_member(
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

fn ensure_scope_exists(
    state: &MemoryState,
    scope: &ConversationScope,
) -> Result<(), HubStoreError> {
    match scope {
        ConversationScope::Global => Ok(()),
        ConversationScope::Project(id) => require_project(state, id),
        ConversationScope::Group(id) => require_group(state, id),
    }
}

fn require_group_and_project(
    state: &MemoryState,
    group_id: &str,
    project_id: &str,
) -> Result<(), HubStoreError> {
    require_group(state, group_id)?;
    require_project(state, project_id)
}

fn require_conversation(state: &MemoryState, id: &str) -> Result<(), HubStoreError> {
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

fn require_group(state: &MemoryState, id: &str) -> Result<(), HubStoreError> {
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

fn conflict(entity: HubEntity) -> HubStoreError {
    HubStoreError::Conflict {
        entity,
        message: "idempotency key was reused with different request data".into(),
    }
}

fn touch_conversation(state: &mut MemoryState, id: &str, updated_at_ms: u64) {
    if let Some(conversation) = state.conversations.iter_mut().find(|item| item.id == id) {
        conversation.updated_at_ms = updated_at_ms;
    }
}

fn project_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project")
        .to_owned()
}
