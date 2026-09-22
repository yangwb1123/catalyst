use serde_json::Value;

use super::{RemoteClient, RemoteError, validate_conversation_id, validate_entity_id};

impl RemoteClient {
    pub(crate) async fn preview_session_runner_receipt_observation(
        &self,
        conversation_id: &str,
        run_id: &str,
        observation: &Value,
    ) -> Result<Value, RemoteError> {
        validate_conversation_id(conversation_id)?;
        validate_entity_id(run_id, "Run")?;
        self.send_json(
            self.http
                .post(self.session_runner_receipt_observation_url(conversation_id, run_id)?)
                .json(observation),
        )
        .await
    }
}
