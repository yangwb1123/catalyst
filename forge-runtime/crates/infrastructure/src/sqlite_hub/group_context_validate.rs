use std::collections::{BTreeMap, BTreeSet};

use crate::runtime_domain::{
    ConversationScope, GroupContextPayload, GroupContextProvenance, GroupContextSlice,
    GroupContextStats, HubStoreError,
};

use super::{group_context_build::digest_bytes, group_context_read::validate_policy};

const MAX_ENTITY_ID_BYTES: usize = 128;

pub(super) fn validate_context(slice: &GroupContextSlice) -> Result<(), HubStoreError> {
    let payload = &slice.payload;
    validate_policy(&payload.policy)
        .map_err(|_| corrupt("stored Group context policy is outside its bounds"))?;
    require_id(&payload.group.id, "Group ID")?;
    let members = validate_members(payload)?;
    let stats = validate_conversations(payload, &members)?;
    validate_stats(&payload.stats, &stats)?;
    if stats.content_bytes > payload.policy.max_total_content_bytes {
        return Err(corrupt("Group context exceeds its total content budget"));
    }
    Ok(())
}

fn validate_members(payload: &GroupContextPayload) -> Result<BTreeMap<&str, &str>, HubStoreError> {
    if payload.members.len() > payload.policy.max_members {
        return Err(corrupt("Group context contains too many members"));
    }
    let mut members = BTreeMap::new();
    for member in &payload.members {
        require_id(&member.project_id, "member Project ID")?;
        require_text(&member.project_name, "member Project name")?;
        require_text(&member.role, "member role")?;
        if members
            .insert(member.project_id.as_str(), member.role.as_str())
            .is_some()
        {
            return Err(corrupt("Group context contains a duplicate member"));
        }
    }
    Ok(members)
}

fn validate_conversations(
    payload: &GroupContextPayload,
    members: &BTreeMap<&str, &str>,
) -> Result<GroupContextStats, HubStoreError> {
    let mut stats = GroupContextStats {
        member_count: payload.members.len(),
        conversation_count: payload.conversations.len(),
        omitted_conversation_count: payload.stats.omitted_conversation_count,
        ..GroupContextStats::default()
    };
    let mut conversation_ids = BTreeSet::new();
    let mut prompt_ids = BTreeSet::new();
    let mut source_counts = BTreeMap::new();
    for item in &payload.conversations {
        require_id(&item.conversation.id, "Conversation ID")?;
        if !conversation_ids.insert(item.conversation.id.as_str()) {
            return Err(corrupt("Group context contains a duplicate Conversation"));
        }
        validate_source(payload, members, item, &mut source_counts)?;
        validate_prompts(payload, item, &mut prompt_ids, &mut stats)?;
    }
    validate_source_limits(payload, &source_counts)?;
    Ok(stats)
}

fn validate_source<'a>(
    payload: &'a GroupContextPayload,
    members: &BTreeMap<&str, &str>,
    item: &'a crate::runtime_domain::GroupContextConversation,
    counts: &mut BTreeMap<(&'a str, &'a str), usize>,
) -> Result<(), HubStoreError> {
    if item.conversation.updated_at_ms < item.conversation.created_at_ms {
        return Err(corrupt("Conversation timestamps are inconsistent"));
    }
    let source = match (&item.provenance, &item.conversation.scope) {
        (GroupContextProvenance::Group { group_id }, ConversationScope::Group(scope_id))
            if group_id == &payload.group.id && scope_id == group_id =>
        {
            ("group", group_id.as_str())
        }
        (
            GroupContextProvenance::Project { project_id, role },
            ConversationScope::Project(scope_id),
        ) if scope_id == project_id && members.get(project_id.as_str()) == Some(&role.as_str()) => {
            ("project", project_id.as_str())
        }
        _ => {
            return Err(corrupt(
                "Conversation provenance is outside the Group scope",
            ));
        }
    };
    *counts.entry(source).or_default() += 1;
    Ok(())
}

fn validate_source_limits(
    payload: &GroupContextPayload,
    counts: &BTreeMap<(&str, &str), usize>,
) -> Result<(), HubStoreError> {
    for ((kind, _), count) in counts {
        let limit = if *kind == "group" {
            payload.policy.max_group_conversations
        } else {
            payload.policy.max_project_conversations_per_member
        };
        if *count > limit {
            return Err(corrupt("Group context exceeds a Conversation source limit"));
        }
    }
    Ok(())
}

fn validate_prompts<'a>(
    payload: &GroupContextPayload,
    item: &'a crate::runtime_domain::GroupContextConversation,
    prompt_ids: &mut BTreeSet<&'a str>,
    stats: &mut GroupContextStats,
) -> Result<(), HubStoreError> {
    if item.prompts.len() > payload.policy.max_prompts_per_conversation {
        return Err(corrupt("Group context contains too many Prompts"));
    }
    stats.omitted_prompt_count = stats
        .omitted_prompt_count
        .checked_add(item.omitted_prompt_count)
        .ok_or_else(|| corrupt("Group context omission statistics overflow"))?;
    for prompt in &item.prompts {
        require_id(&prompt.id, "Prompt ID")?;
        if !prompt_ids.insert(prompt.id.as_str()) {
            return Err(corrupt("Group context contains a duplicate Prompt"));
        }
        validate_prompt(payload, prompt)?;
        stats.prompt_count += 1;
        stats.content_bytes += prompt.excerpt.len();
        stats.truncated_prompt_count += usize::from(prompt.truncated);
    }
    Ok(())
}

fn validate_prompt(
    payload: &GroupContextPayload,
    prompt: &crate::runtime_domain::GroupContextPrompt,
) -> Result<(), HubStoreError> {
    let valid_role = matches!(prompt.role.as_str(), "user" | "assistant");
    let valid_lengths = prompt.excerpt.len() <= payload.policy.max_prompt_excerpt_bytes
        && prompt.excerpt.len() <= prompt.original_bytes;
    let valid_truncation = prompt.truncated == (prompt.excerpt.len() < prompt.original_bytes);
    if !valid_role || !valid_lengths || !valid_truncation {
        return Err(corrupt("Group context Prompt metadata is inconsistent"));
    }
    if !is_lower_hex_digest(&prompt.content_sha256) {
        return Err(corrupt("Group context Prompt digest is invalid"));
    }
    if !prompt.truncated && digest_bytes(prompt.excerpt.as_bytes()) != prompt.content_sha256 {
        return Err(corrupt(
            "complete Group context Prompt digest does not match",
        ));
    }
    Ok(())
}

fn validate_stats(
    stored: &GroupContextStats,
    derived: &GroupContextStats,
) -> Result<(), HubStoreError> {
    let matches = stored.member_count == derived.member_count
        && stored.conversation_count == derived.conversation_count
        && stored.prompt_count == derived.prompt_count
        && stored.content_bytes == derived.content_bytes
        && stored.omitted_prompt_count == derived.omitted_prompt_count
        && stored.truncated_prompt_count == derived.truncated_prompt_count;
    if matches {
        Ok(())
    } else {
        Err(corrupt(
            "Group context statistics do not match its contents",
        ))
    }
}

fn require_text(value: &str, subject: &str) -> Result<(), HubStoreError> {
    if value.trim().is_empty() {
        return Err(corrupt(&format!("{subject} is empty")));
    }
    Ok(())
}

fn require_id(value: &str, subject: &str) -> Result<(), HubStoreError> {
    require_text(value, subject)?;
    if value.len() > MAX_ENTITY_ID_BYTES {
        return Err(corrupt(&format!("{subject} exceeds its byte limit")));
    }
    Ok(())
}

fn is_lower_hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}
