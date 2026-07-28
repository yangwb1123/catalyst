use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{Capability, Message, Usage};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RunLimits {
    pub max_turns: u32,
    pub max_tool_calls: u32,
    pub max_tool_output_bytes: usize,
    pub max_model_output_bytes: usize,
    pub max_model_events: u32,
    pub max_output_tokens_per_turn: u32,
}

impl Default for RunLimits {
    fn default() -> Self {
        Self {
            max_turns: 8,
            max_tool_calls: 16,
            max_tool_output_bytes: 64 * 1024,
            max_model_output_bytes: 64 * 1024,
            max_model_events: 4_096,
            max_output_tokens_per_turn: 4_096,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RunRequest {
    pub session_id: String,
    pub run_id: String,
    pub prompt: String,
    pub system_prompt: String,
    pub workspace: PathBuf,
    pub allowed_capabilities: Vec<Capability>,
    pub limits: RunLimits,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitKind {
    Turns,
    ToolCalls,
    ModelOutput,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RunOutcome {
    Completed { answer: String },
    Cancelled,
    LimitExceeded { kind: LimitKind },
    Failed { code: String, message: String },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RunResult {
    pub outcome: RunOutcome,
    pub messages: Vec<Message>,
    pub usage: Usage,
}
