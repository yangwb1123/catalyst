use serde_json::Value;

use super::{RemoteClient, RemoteError, validate_conversation_id, validate_entity_id};

impl RemoteClient {
    /// Posts one explicit local Runner preview request exactly once. The
    /// candidate may invoke an injected executor, so it deliberately uses the
    /// non-retrying write path.
    pub(crate) async fn preview_local_runner_execution_readiness(
        &self,
        conversation_id: &str,
        intent_id: &str,
        request: &Value,
    ) -> Result<Value, RemoteError> {
        validate_conversation_id(conversation_id)?;
        validate_entity_id(intent_id, "Run-intent")?;
        self.send_json(
            self.http
                .post(self.local_runner_preview_url(conversation_id, intent_id)?)
                .json(request),
        )
        .await
    }
}
