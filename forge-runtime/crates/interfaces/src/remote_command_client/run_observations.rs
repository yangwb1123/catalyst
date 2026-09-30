use super::{
    OwnedRunPageResponse, OwnedRunTimelinePageResponse, RemoteClient, RemoteError, RunObserved,
    Value, validate_conversation_id, validate_entity_id, validate_run_page,
    validate_run_page_request, validate_run_timeline, validate_timeline_request,
};

impl RemoteClient {
    pub(in crate::remote_command) async fn list_runs(
        &self,
        conversation_id: &str,
        limit: usize,
        before_created_at_ms: Option<u64>,
        before_run_id: Option<&str>,
    ) -> Result<Value, RemoteError> {
        validate_conversation_id(conversation_id)?;
        validate_run_page_request(limit, before_created_at_ms, before_run_id)?;
        let mut request = self
            .http
            .get(self.conversation_runs_url(conversation_id)?)
            .query(&[("limit", limit.to_string())]);
        if let (Some(created_at_ms), Some(run_id)) = (before_created_at_ms, before_run_id) {
            request = request.query(&[
                ("before_created_at_ms", created_at_ms.to_string()),
                ("before_run_id", run_id.to_owned()),
            ]);
        }
        let response = self.send_read_json(request).await?;
        let page: OwnedRunPageResponse = serde_json::from_value(response)
            .map_err(|_| RemoteError("Forge API returned an invalid Run page".into()))?;
        validate_run_page(
            &page,
            conversation_id,
            limit,
            before_created_at_ms,
            before_run_id,
        )
        .map_err(RemoteError)?;
        serde_json::to_value(page).map_err(|_| RemoteError("Run page could not be encoded".into()))
    }

    /// Reads one owner-bound, content-free Run observation from the explicit
    /// authenticated candidate. The server derives the owner from the bearer
    /// claims; the client binds the returned projection to the requested
    /// Conversation and Run and accepts no execution authority.
    pub(in crate::remote_command) async fn read_run_observation(
        &self,
        conversation_id: &str,
        run_id: &str,
    ) -> Result<RunObserved, RemoteError> {
        validate_conversation_id(conversation_id)?;
        validate_entity_id(run_id, "Run")?;
        let response = self
            .send_read_json(
                self.http
                    .get(self.run_observation_url(conversation_id, run_id)?),
            )
            .await?;
        let observation: RunObserved = serde_json::from_value(response)
            .map_err(|_| RemoteError("Forge API returned an invalid Run observation".into()))?;
        if observation.conversation_id != conversation_id || observation.run_id != run_id {
            return Err(RemoteError(
                "Forge API returned a Run observation with mismatched binding".into(),
            ));
        }
        observation
            .validate()
            .map_err(|_| RemoteError("Forge API returned an invalid Run observation".into()))?;
        Ok(observation)
    }

    pub(in crate::remote_command) async fn run_timeline(
        &self,
        conversation_id: &str,
        run_id: &str,
        after_sequence: u64,
        limit: usize,
    ) -> Result<Value, RemoteError> {
        validate_conversation_id(conversation_id)?;
        validate_entity_id(run_id, "Run")?;
        validate_timeline_request(after_sequence, limit)?;
        let response = self
            .send_read_json(
                self.http
                    .get(self.run_timeline_url(conversation_id, run_id)?)
                    .query(&[
                        ("after_sequence", after_sequence.to_string()),
                        ("limit", limit.to_string()),
                    ]),
            )
            .await?;
        let page: OwnedRunTimelinePageResponse = serde_json::from_value(response)
            .map_err(|_| RemoteError("Forge API returned an invalid Run timeline".into()))?;
        validate_run_timeline(&page, conversation_id, run_id, after_sequence, limit)
            .map_err(RemoteError)?;
        serde_json::to_value(page)
            .map_err(|_| RemoteError("Run timeline could not be encoded".into()))
    }
}
