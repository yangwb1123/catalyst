use std::sync::Arc;

use crate::{
    HubError, HubField, MAX_ENTITY_ID_BYTES, MAX_IDEMPOTENCY_KEY_BYTES,
    runtime_domain::{
        GROUP_RUN_VERSION, GroupContextPolicy, GroupRunRecord, GroupRunSnapshot, GroupRunStore,
        MAX_GROUP_CONTEXT_CONTENT_BYTES, MAX_GROUP_CONTEXT_GROUP_CONVERSATIONS,
        MAX_GROUP_CONTEXT_MEMBERS, MAX_GROUP_CONTEXT_PROJECT_CONVERSATIONS,
        MAX_GROUP_CONTEXT_PROMPT_EXCERPT_BYTES, MAX_GROUP_CONTEXT_PROMPTS_PER_CONVERSATION,
        MAX_GROUP_RUN_LIST_LIMIT, PrepareGroupRun, PrepareGroupRunResult,
    },
};

pub struct GroupRunService {
    store: Arc<dyn GroupRunStore>,
}

impl GroupRunService {
    #[must_use]
    pub fn new(store: Arc<dyn GroupRunStore>) -> Self {
        Self { store }
    }

    /// Freezes one Group dossier without invoking any provider.
    ///
    /// # Errors
    ///
    /// Returns validation or structured storage errors.
    pub fn prepare(&self, request: &PrepareGroupRun) -> Result<PrepareGroupRunResult, HubError> {
        if request.v != GROUP_RUN_VERSION {
            return Err(HubError::UnsupportedGroupRunVersion {
                actual: request.v,
                expected: GROUP_RUN_VERSION,
            });
        }
        required(&request.run_id, HubField::GroupRunId, MAX_ENTITY_ID_BYTES)?;
        required(&request.group_id, HubField::GroupId, MAX_ENTITY_ID_BYTES)?;
        required(
            &request.idempotency_key,
            HubField::IdempotencyKey,
            MAX_IDEMPOTENCY_KEY_BYTES,
        )?;
        if i64::try_from(request.created_at_ms).is_err() {
            return Err(HubError::GroupRunCreationTimeOutOfRange);
        }
        validate_policy(&request.policy)?;
        Ok(self.store.prepare_group_run(request)?)
    }

    /// Loads and verifies one frozen Group Run.
    ///
    /// # Errors
    ///
    /// Returns validation or structured storage errors.
    pub fn inspect(&self, run_id: &str) -> Result<GroupRunSnapshot, HubError> {
        required(run_id, HubField::GroupRunId, MAX_ENTITY_ID_BYTES)?;
        Ok(self.store.inspect_group_run(run_id)?)
    }

    /// Lists prepared Group Run metadata.
    ///
    /// # Errors
    ///
    /// Returns validation or structured storage errors.
    pub fn list(
        &self,
        group_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupRunRecord>, HubError> {
        if let Some(id) = group_id {
            required(id, HubField::GroupId, MAX_ENTITY_ID_BYTES)?;
        }
        list_limit(limit)?;
        Ok(self.store.list_group_runs(group_id, limit)?)
    }
}

fn required(value: &str, field: HubField, max_bytes: usize) -> Result<(), HubError> {
    if value.trim().is_empty() {
        return Err(HubError::Empty { field });
    }
    if value.len() > max_bytes {
        return Err(HubError::TooLong { field, max_bytes });
    }
    if value.chars().any(unsupported_identifier_character) {
        return Err(HubError::InvalidCharacters { field });
    }
    Ok(())
}

fn unsupported_identifier_character(value: char) -> bool {
    value.is_control()
        || matches!(
            value,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
}

fn validate_policy(policy: &GroupContextPolicy) -> Result<(), HubError> {
    for (value, field, max) in [
        (
            policy.max_members,
            HubField::GroupContextMembers,
            MAX_GROUP_CONTEXT_MEMBERS,
        ),
        (
            policy.max_group_conversations,
            HubField::GroupContextGroupConversations,
            MAX_GROUP_CONTEXT_GROUP_CONVERSATIONS,
        ),
        (
            policy.max_project_conversations_per_member,
            HubField::GroupContextProjectConversations,
            MAX_GROUP_CONTEXT_PROJECT_CONVERSATIONS,
        ),
        (
            policy.max_prompts_per_conversation,
            HubField::GroupContextPrompts,
            MAX_GROUP_CONTEXT_PROMPTS_PER_CONVERSATION,
        ),
        (
            policy.max_prompt_excerpt_bytes,
            HubField::GroupContextPromptExcerptBytes,
            MAX_GROUP_CONTEXT_PROMPT_EXCERPT_BYTES,
        ),
        (
            policy.max_total_content_bytes,
            HubField::GroupContextBytes,
            MAX_GROUP_CONTEXT_CONTENT_BYTES,
        ),
    ] {
        range(value, field, max)?;
    }
    Ok(())
}

fn list_limit(value: usize) -> Result<(), HubError> {
    range(value, HubField::GroupRunLimit, MAX_GROUP_RUN_LIST_LIMIT)
}

fn range(value: usize, field: HubField, max: usize) -> Result<(), HubError> {
    if (1..=max).contains(&value) {
        return Ok(());
    }
    Err(HubError::OutOfRange { field, min: 1, max })
}
