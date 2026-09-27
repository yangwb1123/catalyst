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
        super::super::session_runner_receipt::validate_value(observation)?;
        let (request_conversation_id, request_run_id) =
            super::super::session_runner_receipt::conversation_and_run(observation)?;
        if request_conversation_id != conversation_id || request_run_id != run_id {
            return Err(RemoteError(
                "Session Runner receipt observation request does not match its URL".into(),
            ));
        }
        let response = self
            .send_json(
                self.http
                    .post(self.session_runner_receipt_observation_url(conversation_id, run_id)?)
                    .json(observation),
            )
            .await?;
        super::super::session_runner_receipt::validate_response(
            &response,
            observation,
            conversation_id,
            run_id,
        )?;
        Ok(response)
    }
}
