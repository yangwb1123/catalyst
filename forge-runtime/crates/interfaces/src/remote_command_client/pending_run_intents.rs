use super::{
    MAX_SAFE_JSON_INTEGER, RemoteClient, RemoteError, Value, json, pending_intent,
    validate_conversation_id, validate_entity_id,
};

impl RemoteClient {
    pub(in crate::remote_command) async fn list_pending_run_intents(
        &self,
        conversation_id: &str,
        limit: usize,
        before_submitted_at_ms: Option<u64>,
        before_intent_id: Option<&str>,
    ) -> Result<Value, RemoteError> {
        validate_conversation_id(conversation_id)?;
        if limit == 0 || limit > pending_intent::PAGE_SIZE {
            return Err(RemoteError(
                "pending Run-intent page request is invalid".into(),
            ));
        }
        if before_submitted_at_ms.is_some() != before_intent_id.is_some()
            || before_submitted_at_ms.is_some_and(|value| value > MAX_SAFE_JSON_INTEGER)
        {
            return Err(RemoteError("pending Run-intent cursor is invalid".into()));
        }
        if let Some(intent_id) = before_intent_id {
            validate_entity_id(intent_id, "pending Run-intent")?;
        }
        let mut request = self
            .http
            .get(self.pending_run_intents_url(conversation_id)?)
            .query(&[("limit", limit.to_string())]);
        if let (Some(submitted_at_ms), Some(intent_id)) = (before_submitted_at_ms, before_intent_id)
        {
            request = request.query(&[
                ("before_submitted_at_ms", submitted_at_ms.to_string()),
                ("before_intent_id", intent_id.to_owned()),
            ]);
        }
        let response = self.send_read_json(request).await?;
        let page: pending_intent::Page = serde_json::from_value(response).map_err(|_| {
            RemoteError("Forge API returned an invalid pending Run-intent page".into())
        })?;
        let before =
            before_submitted_at_ms
                .zip(before_intent_id)
                .map(|(submitted_at_ms, intent_id)| pending_intent::Cursor {
                    submitted_at_ms,
                    intent_id: intent_id.to_owned(),
                });
        pending_intent::validate_page(&page, conversation_id, before.as_ref(), limit)
            .map_err(RemoteError)?;
        serde_json::to_value(page)
            .map_err(|_| RemoteError("pending Run-intent page could not be encoded".into()))
    }

    /// Submits a Prompt to the private inert pending Run-intent candidate.
    /// The response is an immutable receipt only; it does not create an
    /// ordinary Run, select a device, or authorize execution.
    pub(in crate::remote_command) async fn submit_pending_run_intent(
        &self,
        conversation_id: &str,
        expected_version: u64,
        content: &str,
        idempotency_key: &str,
    ) -> Result<Value, RemoteError> {
        validate_conversation_id(conversation_id)?;
        if content.trim().is_empty()
            || content.len() > 256 * 1024
            || expected_version == 0
            || expected_version > MAX_SAFE_JSON_INTEGER
        {
            return Err(RemoteError("pending Run-intent content is invalid".into()));
        }
        let response = self
            .send_json(
                self.http
                    .post(self.pending_run_intents_url(conversation_id)?)
                    .header("Idempotency-Key", idempotency_key)
                    .json(&json!({
                        "content": content,
                        "expected_version": expected_version,
                    })),
            )
            .await?;
        let submission: pending_intent::Submission = serde_json::from_value(response.clone())
            .map_err(|_| {
                RemoteError("Forge API returned an invalid pending Run-intent submission".into())
            })?;
        pending_intent::validate_submission_for_version(
            &submission,
            conversation_id,
            content,
            expected_version,
        )
        .map_err(RemoteError)?;
        serde_json::to_value(submission)
            .map_err(|_| RemoteError("pending Run-intent submission could not be encoded".into()))
    }

    pub(in crate::remote_command) async fn pending_run_intent_timeline(
        &self,
        conversation_id: &str,
        intent_id: &str,
        after_sequence: u64,
        limit: usize,
    ) -> Result<Value, RemoteError> {
        validate_conversation_id(conversation_id)?;
        validate_entity_id(intent_id, "pending Run-intent")?;
        if after_sequence > MAX_SAFE_JSON_INTEGER || limit == 0 || limit > pending_intent::PAGE_SIZE
        {
            return Err(RemoteError(
                "pending Run-intent timeline request is invalid".into(),
            ));
        }
        let url = self.pending_run_intent_timeline_url(conversation_id, intent_id)?;
        let request = self.http.get(url).query(&[
            ("after_sequence", after_sequence.to_string()),
            ("limit", limit.to_string()),
        ]);
        let response = self.send_read_json(request).await?;
        let page: pending_intent::TimelinePage =
            serde_json::from_value(response).map_err(|_| {
                RemoteError("Forge API returned an invalid pending Run-intent timeline".into())
            })?;
        pending_intent::validate_timeline(&page, conversation_id, intent_id, after_sequence, limit)
            .map_err(RemoteError)?;
        serde_json::to_value(page)
            .map_err(|_| RemoteError("pending Run-intent timeline could not be encoded".into()))
    }
}
