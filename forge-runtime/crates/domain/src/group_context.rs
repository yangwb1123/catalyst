use serde::{Deserialize, Serialize};

pub const GROUP_CONTEXT_VERSION: u16 = 1;
pub const GROUP_CONTEXT_DIGEST_DOMAIN: &[u8] = b"forge.group-context.v1\0";
pub const DEFAULT_GROUP_CONTEXT_CONTENT_BYTES: usize = 256 * 1024;
pub const MAX_GROUP_CONTEXT_CONTENT_BYTES: usize = 512 * 1024;
pub const MAX_GROUP_CONTEXT_MEMBERS: usize = 16;
pub const MAX_GROUP_CONTEXT_GROUP_CONVERSATIONS: usize = 4;
pub const MAX_GROUP_CONTEXT_PROJECT_CONVERSATIONS: usize = 2;
pub const MAX_GROUP_CONTEXT_PROMPTS_PER_CONVERSATION: usize = 8;
pub const MAX_GROUP_CONTEXT_PROMPT_EXCERPT_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupContextPolicy {
    pub max_members: usize,
    pub max_group_conversations: usize,
    pub max_project_conversations_per_member: usize,
    pub max_prompts_per_conversation: usize,
    pub max_prompt_excerpt_bytes: usize,
    pub max_total_content_bytes: usize,
}

impl Default for GroupContextPolicy {
    fn default() -> Self {
        Self {
            max_members: MAX_GROUP_CONTEXT_MEMBERS,
            max_group_conversations: MAX_GROUP_CONTEXT_GROUP_CONVERSATIONS,
            max_project_conversations_per_member: MAX_GROUP_CONTEXT_PROJECT_CONVERSATIONS,
            max_prompts_per_conversation: MAX_GROUP_CONTEXT_PROMPTS_PER_CONVERSATION,
            max_prompt_excerpt_bytes: MAX_GROUP_CONTEXT_PROMPT_EXCERPT_BYTES,
            max_total_content_bytes: DEFAULT_GROUP_CONTEXT_CONTENT_BYTES,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupContextSlice {
    pub v: u16,
    pub payload: GroupContextPayload,
    pub slice_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupContextPayload {
    pub policy: GroupContextPolicy,
    pub group: crate::SessionGroup,
    pub members: Vec<GroupContextMember>,
    pub conversations: Vec<GroupContextConversation>,
    pub stats: GroupContextStats,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupContextMember {
    pub project_id: String,
    pub project_name: String,
    pub role: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GroupContextProvenance {
    Group { group_id: String },
    Project { project_id: String, role: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupContextConversation {
    pub conversation: crate::Conversation,
    pub provenance: GroupContextProvenance,
    pub prompts: Vec<GroupContextPrompt>,
    pub omitted_prompt_count: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupContextPrompt {
    pub id: String,
    pub role: String,
    pub created_at_ms: u64,
    pub excerpt: String,
    pub original_bytes: usize,
    pub content_sha256: String,
    pub truncated: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupContextStats {
    pub member_count: usize,
    pub conversation_count: usize,
    pub prompt_count: usize,
    pub content_bytes: usize,
    pub omitted_conversation_count: usize,
    pub omitted_prompt_count: usize,
    pub truncated_prompt_count: usize,
}
