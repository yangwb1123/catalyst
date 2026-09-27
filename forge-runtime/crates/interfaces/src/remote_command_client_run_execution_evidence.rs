use serde_json::Value;

use super::{RemoteClient, RemoteError, validate_conversation_id, validate_entity_id};

impl RemoteClient {
    pub(crate) async fn preview_run_execution_evidence(
        &self,
        conversation_id: &str,
        run_id: &str,
        request: &Value,
    ) -> Result<Value, RemoteError> {
        validate_conversation_id(conversation_id)?;
        validate_entity_id(run_id, "Run")?;
        let (request_conversation_id, request_run_id) =
            super::super::run_execution_evidence::conversation_and_run(request)?;
        if request_conversation_id != conversation_id || request_run_id != run_id {
            return Err(RemoteError(
                "Run execution evidence request does not match its URL".into(),
            ));
        }
        let response = self
            .send_json(
                self.http
                    .post(self.run_execution_evidence_url(conversation_id, run_id)?)
                    .json(request),
            )
            .await?;
        super::super::run_execution_evidence::validate_response(
            &response,
            request,
            conversation_id,
            run_id,
        )?;
        Ok(response)
    }
}
