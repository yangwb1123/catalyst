use crate::runtime_domain::ConversationImportPrompt;

use super::super::import_payload;
use super::{RemoteClient, RemoteError};

impl RemoteClient {
    pub(in crate::remote_command) async fn import_owned_conversation(
        &self,
        title: &str,
        prompts: &[ConversationImportPrompt],
        idempotency_key: &str,
    ) -> Result<serde_json::Value, RemoteError> {
        if title.trim().is_empty() || title.len() > 256 || prompts.len() > 128 {
            return Err(RemoteError("conversation import is invalid".into()));
        }
        let mut total_bytes = 0_usize;
        for prompt in prompts {
            if !matches!(prompt.role.as_str(), "user" | "assistant")
                || prompt.content.trim().is_empty()
                || prompt.content.len() > 256 * 1024
            {
                return Err(RemoteError("conversation import is invalid".into()));
            }
            total_bytes = total_bytes.saturating_add(prompt.content.len());
            if total_bytes > 256 * 1024 {
                return Err(RemoteError("conversation import is invalid".into()));
            }
        }
        self.send_json(
            self.http
                .post(self.endpoint("/api/v1/conversations/import")?)
                .header("Idempotency-Key", idempotency_key)
                .json(&import_payload(title, prompts)),
        )
        .await
    }
}
