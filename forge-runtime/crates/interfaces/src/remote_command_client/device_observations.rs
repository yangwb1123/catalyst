use super::{
    RemoteClient, RemoteError, Value, lifecycle_registry, validate_conversation_id,
    validate_entity_id,
};

impl RemoteClient {
    /// Reads the private, owner-bound inventory candidate. The server derives
    /// the owner from the verified bearer claims; the client accepts only the
    /// shared unverified observation envelope and never treats it as authority.
    pub(in crate::remote_command) async fn read_device_inventory(
        &self,
    ) -> Result<Value, RemoteError> {
        let response = self
            .send_read_json(self.http.get(self.endpoint("/api/v1/devices")?))
            .await?;
        crate::device_inventory_command::validate_remote_response(&response)
            .map_err(|_| RemoteError("Forge API returned an invalid device inventory".into()))?;
        Ok(response)
    }

    /// Reads the explicitly requested lifecycle-registry candidate. This is
    /// a GET-only value observation; it is never part of startup or `sync`.
    pub(in crate::remote_command) async fn read_lifecycle_registry(
        &self,
    ) -> Result<Value, RemoteError> {
        let response = self
            .send_read_json(self.http.get(self.lifecycle_registry_url()?))
            .await?;
        lifecycle_registry::validate_response(&response)?;
        Ok(response)
    }

    /// Posts one explicitly requested metadata-only credential lifecycle
    /// candidate.  This is an injected, owner-scoped seam: the caller must
    /// provide the reviewed request image, the POST is sent exactly once, and
    /// no credential material is accepted or returned by this client layer.
    pub(in crate::remote_command) async fn preview_device_credential_candidate(
        &self,
        request: &Value,
    ) -> Result<Value, RemoteError> {
        crate::device_credential_candidate_command::validate_remote_request(request)
            .map_err(|error| RemoteError(error.to_string()))?;
        let response = self
            .send_json(
                self.http
                    .post(self.credential_candidate_url()?)
                    .json(request),
            )
            .await?;
        crate::device_credential_candidate_command::validate_remote_response(&response, request)
            .map_err(|error| RemoteError(error.to_string()))?;
        Ok(response)
    }

    /// Reads the private, owner-bound lossless v2 inventory candidate. The
    /// candidate is never mounted by production constructors; this client
    /// method is useful for explicit test/injected sources only.
    pub(in crate::remote_command) async fn read_device_inventory_v2(
        &self,
    ) -> Result<Value, RemoteError> {
        let response = self
            .send_read_json(
                self.http
                    .get(self.endpoint("/api/v1/devices/observations/v2")?),
            )
            .await?;
        crate::device_inventory_observation_v2_command::validate_remote_response(&response)
            .map_err(|_| RemoteError("Forge API returned an invalid v2 device inventory".into()))?;
        Ok(response)
    }

    /// Reads the private owner-bound client-instance/session observation.
    /// Forge mounts this path only behind the accepted device-fabric
    /// activation assembly; the ordinary server constructor remains closed.
    /// The client validates and returns only the strict display-only envelope.
    pub(in crate::remote_command) async fn read_client_instance_session_view(
        &self,
    ) -> Result<Value, RemoteError> {
        let response = self
            .send_read_json(
                self.http
                    .get(self.endpoint("/api/v1/client-instances/session-view")?),
            )
            .await?;
        crate::device_client_session_view_command::validate_remote_response(&response).map_err(
            |_| RemoteError("Forge API returned an invalid client-instance session view".into()),
        )?;
        Ok(response)
    }

    /// Reads the private owner-bound client-instance/resource observation.
    /// The accepted activation assembly may mount it, while the ordinary
    /// server constructor remains closed. It is a strict display-only
    /// composition and carries no inventory, scheduling, reservation, or
    /// execution authority.
    pub(in crate::remote_command) async fn read_client_instance_resource_view(
        &self,
    ) -> Result<Value, RemoteError> {
        let response = self
            .send_read_json(
                self.http
                    .get(self.endpoint("/api/v1/client-instances/resource-view")?),
            )
            .await?;
        crate::device_client_instance_resource_view_command::validate_remote_response(&response)
            .map_err(|_| {
                RemoteError("Forge API returned an invalid client-instance resource view".into())
            })?;
        Ok(response)
    }

    pub(in crate::remote_command) async fn preview_session_device_observation(
        &self,
        conversation_id: &str,
        run_id: &str,
        request: &Value,
    ) -> Result<Value, RemoteError> {
        validate_conversation_id(conversation_id)?;
        validate_entity_id(run_id, "Run")?;
        super::super::session_observation::validate_request(request)?;
        let request_object = request
            .as_object()
            .ok_or_else(|| RemoteError("remote session observation input is invalid".into()))?;
        let request_conversation_id = request_object
            .get("conversation_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RemoteError("remote session observation conversation is invalid".into())
            })?;
        let request_run_id = request_object
            .get("run_id")
            .and_then(Value::as_str)
            .ok_or_else(|| RemoteError("remote session observation Run is invalid".into()))?;
        if request_conversation_id != conversation_id || request_run_id != run_id {
            return Err(RemoteError(
                "Session device observation request does not match its URL".into(),
            ));
        }
        let response = self
            .send_json(
                self.http
                    .post(self.session_device_observation_url(conversation_id, run_id)?)
                    .json(request),
            )
            .await?;
        super::super::session_observation::validate_response(&response, request)?;
        Ok(response)
    }
}
