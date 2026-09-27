use serde_json::Value;

use super::{RemoteClient, RemoteError, validate_conversation_id, validate_entity_id};

impl RemoteClient {
    pub(crate) async fn preview_runner_execution_boundary(
        &self,
        conversation_id: &str,
        run_id: &str,
        request: &Value,
    ) -> Result<Value, RemoteError> {
        validate_conversation_id(conversation_id)?;
        validate_entity_id(run_id, "Run")?;
        super::super::runner_execution_boundary::validate_request(request)?;
        let (request_conversation_id, request_run_id) =
            super::super::runner_execution_boundary::conversation_and_run(request)?;
        if request_conversation_id != conversation_id || request_run_id != run_id {
            return Err(RemoteError(
                "Runner execution boundary request does not match its URL".into(),
            ));
        }
        let response = self
            .send_json(
                self.http
                    .post(self.runner_execution_boundary_url(conversation_id, run_id)?)
                    .json(request),
            )
            .await?;
        super::super::runner_execution_boundary::validate_response(
            &response,
            request,
            conversation_id,
            run_id,
        )?;
        Ok(response)
    }
}
