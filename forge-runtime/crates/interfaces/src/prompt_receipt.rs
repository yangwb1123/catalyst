use crate::runtime_domain::PromptRecord;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct PromptReceipt {
    pub id: String,
    pub conversation_id: String,
    pub created_at_ms: u64,
}

impl From<PromptRecord> for PromptReceipt {
    fn from(prompt: PromptRecord) -> Self {
        Self {
            id: prompt.id,
            conversation_id: prompt.conversation_id,
            created_at_ms: prompt.created_at_ms,
        }
    }
}
