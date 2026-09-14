#![allow(dead_code)]

use std::{
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
};

use forge_runtime_domain::{
    Conversation, ConversationBootstrapCursor, ConversationBootstrapEntry,
    ConversationBootstrapPage, ConversationBootstrapPhase, ConversationChange,
    ConversationChangeKind, ConversationChangePage, ConversationPrompt, ConversationPromptCursor,
    ConversationPromptPage, ConversationScope, GroupContextPolicy, GroupContextSlice,
    GroupProjectMember, HubEntity, HubSnapshot, HubSnapshotAtCursor, HubStore, HubStoreError,
    MAX_CONVERSATION_BOOTSTRAP_PAGE_LIMIT, MAX_CONVERSATION_CHANGE_PAGE_LIMIT,
    MAX_CONVERSATION_PROMPT_PAGE_CONTENT_BYTES, MAX_CONVERSATION_PROMPT_PAGE_LIMIT, Project,
    PromptRecord, SessionGroup,
};

mod atomic_memory;
mod bootstrap;
mod memory_queries;

use memory_queries::*;

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
    conversation_changes: Vec<ConversationChange>,
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

    pub fn seed_prompt(&self, prompt: PromptRecord) {
        self.state
            .lock()
            .expect("memory store fixture lock")
            .prompts
            .push(prompt);
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

    fn snapshot_at_cursor(&self) -> Result<HubSnapshotAtCursor, HubStoreError> {
        let state = self.state()?;
        Ok(HubSnapshotAtCursor {
            snapshot: snapshot_from(&state, &ConversationScope::Global),
            cursor: u64::try_from(state.conversation_changes.len())
                .expect("test change count fits u64"),
        })
    }

    fn conversation_changes_after(
        &self,
        after_cursor: u64,
        limit: usize,
    ) -> Result<ConversationChangePage, HubStoreError> {
        if !(1..=MAX_CONVERSATION_CHANGE_PAGE_LIMIT).contains(&limit) {
            return Err(HubStoreError::Conflict {
                entity: HubEntity::Conversation,
                message: "invalid in-memory Hub change page limit".into(),
            });
        }
        let state = self.state()?;
        let head_cursor =
            u64::try_from(state.conversation_changes.len()).expect("test change count fits u64");
        if after_cursor > head_cursor {
            return Err(HubStoreError::Conflict {
                entity: HubEntity::Conversation,
                message: "change cursor is beyond the in-memory Hub head".into(),
            });
        }
        let changes = state
            .conversation_changes
            .iter()
            .filter(|change| change.cursor > after_cursor)
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();
        let next_cursor = changes.last().map_or(after_cursor, |change| change.cursor);
        Ok(ConversationChangePage {
            after_cursor,
            next_cursor,
            head_cursor,
            has_more: next_cursor < head_cursor,
            changes,
        })
    }

    fn conversation_bootstrap_page(
        &self,
        cursor: Option<&ConversationBootstrapCursor>,
        limit: usize,
    ) -> Result<ConversationBootstrapPage, HubStoreError> {
        let state = self.state()?;
        bootstrap::conversation_bootstrap_page(&state, cursor, limit)
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
        let cursor = u64::try_from(state.conversation_changes.len())
            .expect("test change count fits u64")
            + 1;
        state.conversation_changes.push(ConversationChange {
            cursor,
            schema_version: 1,
            conversation_id: conversation.id.clone(),
            entity_id: conversation.id.clone(),
            aggregate_version: 1,
            kind: ConversationChangeKind::ConversationCreated,
            created_at_ms,
        });
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
        let aggregate_version = state
            .conversation_changes
            .iter()
            .filter(|change| change.conversation_id == conversation_id)
            .count();
        let aggregate_version =
            u64::try_from(aggregate_version).expect("test aggregate version fits u64") + 1;
        let cursor = u64::try_from(state.conversation_changes.len())
            .expect("test change count fits u64")
            + 1;
        state.conversation_changes.push(ConversationChange {
            cursor,
            schema_version: 1,
            conversation_id: conversation_id.into(),
            entity_id: prompt.id.clone(),
            aggregate_version,
            kind: ConversationChangeKind::PromptAppended,
            created_at_ms,
        });
        Ok(prompt)
    }

    fn list_prompts(
        &self,
        conversation_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<PromptRecord>, HubStoreError> {
        let state = self.state()?;
        if let Some(id) = conversation_id {
            require_conversation(&state, id)?;
        }
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

    fn conversation_prompt_page(
        &self,
        conversation_id: &str,
        before: Option<&ConversationPromptCursor>,
        limit: usize,
    ) -> Result<ConversationPromptPage, HubStoreError> {
        if !(1..=MAX_CONVERSATION_PROMPT_PAGE_LIMIT).contains(&limit) {
            return Err(HubStoreError::Conflict {
                entity: HubEntity::Prompt,
                message: "invalid in-memory Prompt page limit".into(),
            });
        }
        let state = self.state()?;
        require_conversation(&state, conversation_id)?;
        let mut records: Vec<_> = state
            .prompts
            .iter()
            .filter(|record| record.conversation_id == conversation_id)
            .filter(|record| {
                before.is_none_or(|cursor| {
                    (record.created_at_ms, record.id.as_str())
                        < (cursor.created_at_ms, cursor.prompt_id.as_str())
                })
            })
            .collect();
        records.sort_by(|left, right| {
            right
                .created_at_ms
                .cmp(&left.created_at_ms)
                .then_with(|| right.id.cmp(&left.id))
        });
        let (prompts, has_more) = take_memory_prompt_page(records, limit)?;
        Ok(memory_prompt_page(conversation_id, prompts, has_more))
    }

    fn list_prompts_before(
        &self,
        conversation_id: &str,
        boundary_prompt_id: &str,
        limit: usize,
    ) -> Result<Vec<PromptRecord>, HubStoreError> {
        let state = self.state()?;
        require_conversation(&state, conversation_id)?;
        let boundary = history_boundary_index(&state, conversation_id, boundary_prompt_id)?;
        require_user_boundary(&state.prompts[boundary])?;
        let prompts = state.prompts[..boundary]
            .iter()
            .rev()
            .filter(|item| item.conversation_id == conversation_id)
            .take(limit)
            .cloned()
            .collect();
        Ok(prompts)
    }

    fn load_group_context(
        &self,
        _group_id: &str,
        _policy: &GroupContextPolicy,
    ) -> Result<GroupContextSlice, HubStoreError> {
        Err(HubStoreError::Unavailable {
            message: "the in-memory Hub fixture does not implement Group context".into(),
        })
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

fn take_memory_prompt_page(
    records: Vec<&PromptRecord>,
    limit: usize,
) -> Result<(Vec<ConversationPrompt>, bool), HubStoreError> {
    let mut prompts = Vec::with_capacity(limit.min(records.len()));
    let mut content_bytes = 0_usize;
    for record in records {
        if prompts.len() == limit {
            return Ok((prompts, true));
        }
        if record.content.len() > MAX_CONVERSATION_PROMPT_PAGE_CONTENT_BYTES {
            return Err(HubStoreError::Corrupt {
                message: "Prompt exceeds the history page content budget".into(),
            });
        }
        let next_bytes = content_bytes.saturating_add(record.content.len());
        if next_bytes > MAX_CONVERSATION_PROMPT_PAGE_CONTENT_BYTES {
            return Ok((prompts, true));
        }
        content_bytes = next_bytes;
        prompts.push(ConversationPrompt {
            id: record.id.clone(),
            conversation_id: record.conversation_id.clone(),
            role: record.role.clone(),
            content: record.content.clone(),
            created_at_ms: record.created_at_ms,
        });
    }
    Ok((prompts, false))
}

fn memory_prompt_page(
    conversation_id: &str,
    prompts: Vec<ConversationPrompt>,
    has_more: bool,
) -> ConversationPromptPage {
    let next_cursor = if has_more {
        prompts.last().map(|prompt| ConversationPromptCursor {
            created_at_ms: prompt.created_at_ms,
            prompt_id: prompt.id.clone(),
        })
    } else {
        None
    };
    ConversationPromptPage {
        conversation_id: conversation_id.into(),
        prompts,
        next_cursor,
        has_more,
    }
}
