use serde_json::Value;

use super::{RemoteClient, RemoteError, validate_conversation_id, validate_entity_id};

impl RemoteClient {
    pub(crate) async fn preview_session_runner_reconciliation_from_history(
        &self,
        conversation_id: &str,
        run_id: &str,
        history: &Value,
    ) -> Result<(Value, Value), RemoteError> {
        let canonical_history = self
            .preview_session_runner_receipt_history(conversation_id, run_id, history)
            .await?;
        super::super::session_runner_receipt_history::validate_response(
            &canonical_history,
            history,
            conversation_id,
            run_id,
        )?;
        let projection = self
            .preview_session_runner_reconciliation(conversation_id, run_id, &canonical_history)
            .await?;
        Ok((canonical_history, projection))
    }

    pub(crate) async fn preview_session_runner_reconciliation(
        &self,
        conversation_id: &str,
        run_id: &str,
        history: &Value,
    ) -> Result<Value, RemoteError> {
        validate_conversation_id(conversation_id)?;
        validate_entity_id(run_id, "Run")?;
        super::super::session_runner_receipt_history::validate_value(history)?;
        let (request_conversation_id, request_run_id) =
            super::super::session_runner_receipt_history::conversation_and_run(history)?;
        if request_conversation_id != conversation_id || request_run_id != run_id {
            return Err(RemoteError(
                "Session Runner reconciliation request does not match its URL".into(),
            ));
        }
        let response = self
            .send_json(
                self.http
                    .post(self.session_runner_reconciliation_preview_url(conversation_id, run_id)?)
                    .json(history),
            )
            .await?;
        super::super::session_runner_reconciliation::validate_remote_response(
            &response,
            history,
            conversation_id,
            run_id,
        )?;
        Ok(response)
    }
}
