use std::collections::{BTreeMap, BTreeSet};

use crate::runtime_domain::{
    GROUP_CONTEXT_DIGEST_DOMAIN, GROUP_CONTEXT_VERSION, GroupContextConversation,
    GroupContextMember, GroupContextPayload, GroupContextPolicy, GroupContextPrompt,
    GroupContextProvenance, GroupContextSlice, GroupContextStats, HubStoreError, SessionGroup,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::Conversation;

pub(super) struct LoadedConversation {
    pub conversation: Conversation,
    pub provenance: GroupContextProvenance,
    pub prompts: Vec<LoadedPrompt>,
    pub omitted_prompt_count: usize,
}

pub(super) struct LoadedPrompt {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at_ms: u64,
    pub anchor_rowid: i64,
    pub content_sha256: String,
    pub excerpt: String,
}

pub(super) fn build_slice(
    policy: GroupContextPolicy,
    group: SessionGroup,
    members: Vec<GroupContextMember>,
    mut conversations: Vec<LoadedConversation>,
    omitted_conversation_count: usize,
) -> Result<GroupContextSlice, HubStoreError> {
    allocate_excerpts(&mut conversations, &policy);
    let payload = build_payload(
        policy,
        group,
        members,
        conversations,
        omitted_conversation_count,
    );
    let slice_sha256 = digest_json(&payload)?;
    Ok(GroupContextSlice {
        v: GROUP_CONTEXT_VERSION,
        payload,
        slice_sha256,
    })
}

pub(super) fn digest_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

fn allocate_excerpts(conversations: &mut [LoadedConversation], policy: &GroupContextPolicy) {
    let orders = conversations
        .iter()
        .map(|conversation| causal_allocation_order(&conversation.prompts))
        .collect::<Vec<_>>();
    let depth = orders.iter().map(Vec::len).max().unwrap_or(0);
    let mut blocked_anchors = vec![BTreeSet::new(); conversations.len()];
    let mut remaining = policy.max_total_content_bytes;
    for index in 0..depth {
        allocate_round(
            conversations,
            &orders,
            &mut blocked_anchors,
            index,
            policy.max_prompt_excerpt_bytes,
            &mut remaining,
        );
    }
}

fn allocate_round(
    conversations: &mut [LoadedConversation],
    orders: &[Vec<usize>],
    blocked_anchors: &mut [BTreeSet<i64>],
    index: usize,
    prompt_limit: usize,
    remaining: &mut usize,
) {
    let pairs = conversations.iter_mut().zip(orders).zip(blocked_anchors);
    for ((conversation, order), blocked) in pairs {
        let Some(prompt_index) = order.get(index) else {
            continue;
        };
        let Some(prompt) = conversation.prompts.get_mut(*prompt_index) else {
            continue;
        };
        if blocked.contains(&prompt.anchor_rowid) {
            continue;
        }
        let budget = (*remaining).min(prompt_limit);
        prompt.excerpt = utf8_prefix(&prompt.content, budget).to_owned();
        *remaining = remaining.saturating_sub(prompt.excerpt.len());
        if prompt.role == "user" && !prompt.content.is_empty() && prompt.excerpt.is_empty() {
            blocked.insert(prompt.anchor_rowid);
        }
    }
}

fn causal_allocation_order(prompts: &[LoadedPrompt]) -> Vec<usize> {
    let mut order = Vec::with_capacity(prompts.len());
    let mut start = 0;
    while start < prompts.len() {
        let anchor = prompts[start].anchor_rowid;
        let mut end = start + 1;
        while end < prompts.len() && prompts[end].anchor_rowid == anchor {
            end += 1;
        }
        order.extend((start..end).rev());
        start = end;
    }
    order
}

fn utf8_prefix(value: &str, max_bytes: usize) -> &str {
    let mut end = value.len().min(max_bytes);
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    &value[..end]
}

fn build_payload(
    policy: GroupContextPolicy,
    group: SessionGroup,
    members: Vec<GroupContextMember>,
    conversations: Vec<LoadedConversation>,
    omitted_conversation_count: usize,
) -> GroupContextPayload {
    let conversations = conversations
        .into_iter()
        .map(finalize_conversation)
        .collect::<Vec<_>>();
    let stats = context_stats(&members, &conversations, omitted_conversation_count);
    GroupContextPayload {
        policy,
        group,
        members,
        conversations,
        stats,
    }
}

fn finalize_conversation(mut loaded: LoadedConversation) -> GroupContextConversation {
    loaded.prompts.reverse();
    let prompts = loaded.prompts.into_iter().map(finalize_prompt).collect();
    GroupContextConversation {
        conversation: loaded.conversation,
        provenance: loaded.provenance,
        prompts,
        omitted_prompt_count: loaded.omitted_prompt_count,
    }
}

fn finalize_prompt(loaded: LoadedPrompt) -> GroupContextPrompt {
    let original_bytes = loaded.content.len();
    let truncated = loaded.excerpt.len() < original_bytes;
    GroupContextPrompt {
        id: loaded.id,
        role: loaded.role,
        created_at_ms: loaded.created_at_ms,
        excerpt: loaded.excerpt,
        original_bytes,
        content_sha256: loaded.content_sha256,
        truncated,
    }
}

fn context_stats(
    members: &[GroupContextMember],
    conversations: &[GroupContextConversation],
    omitted_conversation_count: usize,
) -> GroupContextStats {
    let prompts = conversations.iter().flat_map(|item| &item.prompts);
    let prompt_count = prompts.clone().count();
    let content_bytes = prompts.clone().map(|item| item.excerpt.len()).sum();
    let truncated_prompt_count = prompts.filter(|item| item.truncated).count();
    let omitted_prompt_count = conversations
        .iter()
        .map(|item| item.omitted_prompt_count)
        .sum();
    GroupContextStats {
        member_count: members.len(),
        conversation_count: conversations.len(),
        prompt_count,
        content_bytes,
        omitted_conversation_count,
        omitted_prompt_count,
        truncated_prompt_count,
    }
}

fn digest_json(value: &GroupContextPayload) -> Result<String, HubStoreError> {
    let value = serde_json::to_value(value).map_err(|error| HubStoreError::Corrupt {
        message: format!("Group context payload cannot encode: {error}"),
    })?;
    let encoded =
        serde_json::to_vec(&sort_json(value)).map_err(|error| HubStoreError::Corrupt {
            message: format!("Group context payload cannot encode: {error}"),
        })?;
    let mut digest = Sha256::new();
    digest.update(GROUP_CONTEXT_DIGEST_DOMAIN);
    digest.update(encoded);
    Ok(format!("{:x}", digest.finalize()))
}

fn sort_json(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(sort_json).collect()),
        Value::Object(items) => {
            let sorted = items
                .into_iter()
                .map(|(key, value)| (key, sort_json(value)))
                .collect::<BTreeMap<_, _>>();
            Value::Object(sorted.into_iter().collect())
        }
        other => other,
    }
}
