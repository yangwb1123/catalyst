use super::{RemoteClient, RemoteError, Value, validate_conversation_id, validate_entity_id};

impl RemoteClient {
    pub(in crate::remote_command) async fn preview_device_placement(
        &self,
        request: &Value,
    ) -> Result<Value, RemoteError> {
        super::super::placement::validate_request(request)?;
        let response = self
            .send_json(
                self.http
                    .post(self.endpoint("/api/v1/device-placement/preview")?)
                    .json(request),
            )
            .await?;
        super::super::placement::validate_response(&response, request)?;
        Ok(response)
    }

    /// Posts one explicit owner-bound registry placement preview. The
    /// candidate returns only a deterministic v2 observation; it never
    /// selects, reserves, schedules, dispatches, or executes work.
    pub(in crate::remote_command) async fn preview_device_placement_registry(
        &self,
        request: &Value,
    ) -> Result<Value, RemoteError> {
        super::super::placement_registry::validate_request(request)?;
        let response = self
            .send_json(
                self.http
                    .post(self.endpoint("/api/v1/device-placement/registry-preview")?)
                    .json(request),
            )
            .await?;
        super::super::placement_registry::validate_response(&response)?;
        Ok(response)
    }

    /// Posts one owner-bound scheduler selection comparison. The returned
    /// selected IDs remain display-only; this client never treats them as a
    /// reservation, lease, or dispatch authorization.
    pub(in crate::remote_command) async fn preview_scheduler_selection(
        &self,
        request: &Value,
    ) -> Result<Value, RemoteError> {
        super::super::scheduler_selection::validate_request(request)?;
        let response = self
            .send_json(
                self.http
                    .post(self.scheduler_selection_preview_url()?)
                    .json(request),
            )
            .await?;
        super::super::scheduler_selection::validate_response_for_request(&response, request)?;
        Ok(response)
    }

    /// Claims one owner-bound fenced scheduler lease. This is an effectful
    /// POST, so the caller supplies the explicit idempotency key and the
    /// client performs no automatic retry.
    pub(in crate::remote_command) async fn claim_scheduler_selection_lease(
        &self,
        request: &Value,
        idempotency_key: &str,
    ) -> Result<Value, RemoteError> {
        super::super::scheduler_lease::validate_request(request)?;
        let response = self
            .send_json(
                self.http
                    .post(self.scheduler_selection_lease_url()?)
                    .header("Idempotency-Key", idempotency_key)
                    .json(request),
            )
            .await?;
        super::super::scheduler_lease::validate_response_for_request(&response, request)?;
        Ok(response)
    }

    /// Renews one owner-bound fenced lease proof. The effectful POST is sent
    /// once; callers must reuse the explicit key when retrying an uncertain
    /// response.
    pub(in crate::remote_command) async fn renew_scheduler_selection_lease(
        &self,
        request: &Value,
        idempotency_key: &str,
    ) -> Result<Value, RemoteError> {
        super::super::scheduler_lease_renew::validate_request(request)?;
        let response = self
            .send_json(
                self.http
                    .post(self.scheduler_selection_lease_renewal_url()?)
                    .header("Idempotency-Key", idempotency_key)
                    .json(request),
            )
            .await?;
        super::super::scheduler_lease_renew::validate_response_for_request(&response, request)?;
        Ok(response)
    }

    /// Releases one owner-bound fenced lease proof. The effectful POST is
    /// sent once; callers must reuse the explicit key when retrying an
    /// uncertain response.
    pub(in crate::remote_command) async fn release_scheduler_selection_lease(
        &self,
        request: &Value,
        idempotency_key: &str,
    ) -> Result<Value, RemoteError> {
        super::super::scheduler_lease_release::validate_request(request)?;
        let response = self
            .send_json(
                self.http
                    .post(self.scheduler_selection_lease_release_url()?)
                    .header("Idempotency-Key", idempotency_key)
                    .json(request),
            )
            .await?;
        super::super::scheduler_lease_release::validate_response_for_request(&response, request)?;
        Ok(response)
    }

    /// Posts one authenticated Run/Attempt/lease preflight declaration. The
    /// candidate is stateless and metadata-only, so this uses the one-shot
    /// POST path and never retries an uncertain write.
    pub(in crate::remote_command) async fn preview_run_attempt_lease_dispatch_preflight(
        &self,
        conversation_id: &str,
        run_id: &str,
        request: &Value,
    ) -> Result<Value, RemoteError> {
        validate_conversation_id(conversation_id)?;
        validate_entity_id(run_id, "Run")?;
        super::super::run_attempt_lease_dispatch_preflight::validate_request(request)?;
        let (request_conversation_id, request_run_id) =
            super::super::run_attempt_lease_dispatch_preflight::conversation_and_run(request)?;
        if request_conversation_id != conversation_id || request_run_id != run_id {
            return Err(RemoteError(
                "remote Run/Attempt/lease preflight request does not match the URL path".into(),
            ));
        }
        let response = self
            .send_json(
                self.http
                    .post(self.run_attempt_lease_dispatch_preflight_url(conversation_id, run_id)?)
                    .json(request),
            )
            .await?;
        super::super::run_attempt_lease_dispatch_preflight::validate_response(
            &response,
            request,
            conversation_id,
            run_id,
        )?;
        Ok(response)
    }

    /// Posts one caller-supplied restart image to the authenticated
    /// reconciliation candidate. The request is sent exactly once; the
    /// caller must treat the response as metadata-only classification.
    pub(in crate::remote_command) async fn preview_execution_reconciliation(
        &self,
        conversation_id: &str,
        run_id: &str,
        request: &Value,
    ) -> Result<Value, RemoteError> {
        validate_conversation_id(conversation_id)?;
        validate_entity_id(run_id, "Run")?;
        super::super::execution_reconciliation::validate_request(request)?;
        let response = self
            .send_json(
                self.http
                    .post(self.execution_reconciliation_preview_url(conversation_id, run_id)?)
                    .json(request),
            )
            .await?;
        super::super::execution_reconciliation::validate_response(
            &response,
            request,
            conversation_id,
            run_id,
        )?;
        Ok(response)
    }
}
