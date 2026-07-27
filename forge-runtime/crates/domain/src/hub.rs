use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum ConversationScope {
    Global,
    Project(String),
    Group(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Conversation {
    pub id: String,
    pub scope: ConversationScope,
    pub title: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PromptRecord {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub idempotency_key: String,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SessionGroup {
    pub id: String,
    pub name: String,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupProjectMember {
    pub group_id: String,
    pub project_id: String,
    pub role: String,
    pub added_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HubSnapshot {
    pub scope: ConversationScope,
    pub projects: Vec<Project>,
    pub conversations: Vec<Conversation>,
    pub groups: Vec<SessionGroup>,
    pub group_project_members: Vec<GroupProjectMember>,
}
