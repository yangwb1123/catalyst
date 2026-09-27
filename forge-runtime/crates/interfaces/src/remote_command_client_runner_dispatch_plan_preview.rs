use serde_json::Value;

use super::{RemoteClient, RemoteError, validate_conversation_id, validate_entity_id};

impl RemoteClient {
    pub(crate) async fn preview_runner_dispatch_plan(
        &self,
        conversation_id: &str,
        run_id: &str,
        dispatch_plan: &Value,
    ) -> Result<Value, RemoteError> {
        validate_conversation_id(conversation_id)?;
        validate_entity_id(run_id, "Run")?;
        super::super::runner_dispatch_plan_preview::validate_dispatch_plan_request(
            dispatch_plan,
            conversation_id,
            run_id,
        )?;
        let response = self
            .send_json(
                self.http
                    .post(self.runner_dispatch_plan_preview_url(conversation_id, run_id)?)
                    .json(dispatch_plan),
            )
            .await?;
        super::super::runner_dispatch_plan_preview::validate_response_for_dispatch_plan(
            &response,
            dispatch_plan,
            conversation_id,
            run_id,
        )?;
        Ok(response)
    }
}
