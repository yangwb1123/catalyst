use reqwest::Url;

use super::{RemoteClient, RemoteError};

impl RemoteClient {
    pub(super) fn credential_candidate_url(&self) -> Result<Url, RemoteError> {
        self.endpoint("/api/v1/device-enrollment-heartbeat/credential-candidate")
    }

    pub(super) fn lifecycle_registry_url(&self) -> Result<Url, RemoteError> {
        self.endpoint("/api/v1/device-enrollment-heartbeat/lifecycle-registry")
    }

    pub(super) fn conversation_prompts_url(
        &self,
        conversation_id: &str,
    ) -> Result<Url, RemoteError> {
        let mut url = self.endpoint("/api/v1/conversations")?;
        url.path_segments_mut()
            .map_err(|()| RemoteError("Forge API endpoint is invalid".into()))?
            .pop_if_empty()
            .push(conversation_id)
            .push("prompts");
        Ok(url)
    }

    pub(super) fn conversation_url(&self, conversation_id: &str) -> Result<Url, RemoteError> {
        let mut url = self.endpoint("/api/v1/conversations")?;
        url.path_segments_mut()
            .map_err(|()| RemoteError("Forge API endpoint is invalid".into()))?
            .pop_if_empty()
            .push(conversation_id);
        Ok(url)
    }

    pub(super) fn conversation_runs_url(&self, conversation_id: &str) -> Result<Url, RemoteError> {
        let mut url = self.endpoint("/api/v1/conversations")?;
        url.path_segments_mut()
            .map_err(|()| RemoteError("Forge API endpoint is invalid".into()))?
            .pop_if_empty()
            .push(conversation_id)
            .push("runs");
        Ok(url)
    }

    pub(super) fn pending_run_intents_url(
        &self,
        conversation_id: &str,
    ) -> Result<Url, RemoteError> {
        let mut url = self.conversation_url(conversation_id)?;
        url.path_segments_mut()
            .map_err(|()| RemoteError("Forge API endpoint is invalid".into()))?
            .push("run-intents");
        Ok(url)
    }

    pub(super) fn pending_run_intent_timeline_url(
        &self,
        conversation_id: &str,
        intent_id: &str,
    ) -> Result<Url, RemoteError> {
        let mut url = self.pending_run_intents_url(conversation_id)?;
        url.path_segments_mut()
            .map_err(|()| RemoteError("Forge API endpoint is invalid".into()))?
            .push(intent_id)
            .push("timeline");
        Ok(url)
    }

    pub(super) fn run_timeline_url(
        &self,
        conversation_id: &str,
        run_id: &str,
    ) -> Result<Url, RemoteError> {
        let mut url = self.conversation_runs_url(conversation_id)?;
        url.path_segments_mut()
            .map_err(|()| RemoteError("Forge API endpoint is invalid".into()))?
            .push(run_id)
            .push("timeline");
        Ok(url)
    }

    pub(super) fn run_observation_url(
        &self,
        conversation_id: &str,
        run_id: &str,
    ) -> Result<Url, RemoteError> {
        let mut url = self.conversation_runs_url(conversation_id)?;
        url.path_segments_mut()
            .map_err(|()| RemoteError("Forge API endpoint is invalid".into()))?
            .push(run_id)
            .push("observation");
        Ok(url)
    }

    pub(super) fn session_device_observation_url(
        &self,
        conversation_id: &str,
        run_id: &str,
    ) -> Result<Url, RemoteError> {
        let mut url = self.conversation_runs_url(conversation_id)?;
        url.path_segments_mut()
            .map_err(|()| RemoteError("Forge API endpoint is invalid".into()))?
            .push(run_id)
            .push("device-observation")
            .push("preview");
        Ok(url)
    }

    pub(super) fn session_runner_receipt_observation_url(
        &self,
        conversation_id: &str,
        run_id: &str,
    ) -> Result<Url, RemoteError> {
        let mut url = self.conversation_runs_url(conversation_id)?;
        url.path_segments_mut()
            .map_err(|()| RemoteError("Forge API endpoint is invalid".into()))?
            .push(run_id)
            .push("runner-receipt-observation")
            .push("preview");
        Ok(url)
    }

    pub(super) fn run_attempt_lease_dispatch_preflight_url(
        &self,
        conversation_id: &str,
        run_id: &str,
    ) -> Result<Url, RemoteError> {
        let mut url = self.conversation_runs_url(conversation_id)?;
        url.path_segments_mut()
            .map_err(|()| RemoteError("Forge API endpoint is invalid".into()))?
            .push(run_id)
            .push("attempt-lease-dispatch-preflight")
            .push("preview");
        Ok(url)
    }

    pub(super) fn runner_dispatch_plan_preview_url(
        &self,
        conversation_id: &str,
        run_id: &str,
    ) -> Result<Url, RemoteError> {
        let mut url = self.conversation_runs_url(conversation_id)?;
        url.path_segments_mut()
            .map_err(|()| RemoteError("Forge API endpoint is invalid".into()))?
            .push(run_id)
            .push("runner-dispatch-plan-preview");
        Ok(url)
    }

    pub(super) fn local_runner_preview_url(
        &self,
        conversation_id: &str,
        intent_id: &str,
    ) -> Result<Url, RemoteError> {
        let mut url = self.pending_run_intents_url(conversation_id)?;
        url.path_segments_mut()
            .map_err(|()| RemoteError("Forge API endpoint is invalid".into()))?
            .push(intent_id)
            .push("execution-readiness-preview");
        Ok(url)
    }

    pub(super) fn execution_consent_preview_url(
        &self,
        conversation_id: &str,
    ) -> Result<Url, RemoteError> {
        let mut url = self.conversation_url(conversation_id)?;
        url.path_segments_mut()
            .map_err(|()| RemoteError("Forge API endpoint is invalid".into()))?
            .push("execution-consents");
        Ok(url)
    }

    pub(super) fn execution_reconciliation_preview_url(
        &self,
        conversation_id: &str,
        run_id: &str,
    ) -> Result<Url, RemoteError> {
        let mut url = self.conversation_runs_url(conversation_id)?;
        url.path_segments_mut()
            .map_err(|()| RemoteError("Forge API endpoint is invalid".into()))?
            .push(run_id)
            .push("execution-reconciliation")
            .push("preview");
        Ok(url)
    }
}
