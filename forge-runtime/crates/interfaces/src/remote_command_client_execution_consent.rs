use serde_json::Value;

use super::super::{RemoteError, execution_consent_preview, validation};

impl super::RemoteClient {
    /// Reads the owner-bound execution profile preview exactly as a GET. The
    /// response is validated against the URL Conversation and remains a
    /// display-only candidate; this method never grants consent or starts a
    /// Run.
    pub(crate) async fn preview_execution_consent(
        &self,
        conversation_id: &str,
    ) -> Result<Value, RemoteError> {
        validation::validate_conversation_id(conversation_id)?;
        let response = self
            .send_read_json(
                self.http
                    .get(self.execution_consent_preview_url(conversation_id)?),
            )
            .await?;
        execution_consent_preview::validate_response(&response, conversation_id)?;
        Ok(response)
    }
}
