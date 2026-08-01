use std::sync::{Arc, Mutex};

use forge_runtime_application::{
    GroupModelAnalysisRequestCodec, GroupPanelSynthesisDispatchProvider,
    GroupPanelSynthesisService, PrepareGroupPanelSynthesisInput,
};
use forge_runtime_domain::{
    ClaimGroupPanelSynthesisDispatch, GROUP_PANEL_SYNTHESIS_CONSENT_VERSION,
    GROUP_PANEL_SYNTHESIS_PROVIDER_ENDPOINT, GROUP_PANEL_SYNTHESIS_VERSION,
    GroupModelAnalysisInspection, GroupPanelSynthesisProvider, Message, ModelEvent,
    ModelEventStream, ModelRequest, PreparedModelProvider, PreparedModelRequest, ProviderError,
};
use futures_util::stream;

use crate::group_analysis_panel_support;

use super::store::MemorySynthesisStore;

pub(crate) const SYNTHESIS_ID: &str = "synthesis-1";
pub(crate) const PANEL_ID: &str = "panel-1";
pub(crate) const MODEL: &str = "gpt-test";

pub(crate) struct Harness {
    pub(crate) service: GroupPanelSynthesisService,
    pub(crate) syntheses: Arc<MemorySynthesisStore>,
    pub(crate) analyses: Arc<group_analysis_panel_support::MemoryAnalysisStore>,
    pub(crate) snapshot: forge_runtime_domain::GroupRunSnapshot,
}

pub(crate) fn harness() -> Harness {
    let panel = group_analysis_panel_support::harness();
    panel
        .service
        .prepare(&group_analysis_panel_support::prepare_input(&[
            "analysis-a",
            "analysis-b",
        ]))
        .expect("prepare panel");
    let panel_service = Arc::new(panel.service);
    let syntheses = Arc::new(MemorySynthesisStore::default());
    let service =
        GroupPanelSynthesisService::new(panel_service, syntheses.clone(), Arc::new(JsonCodec));
    Harness {
        service,
        syntheses,
        analyses: panel.analyses,
        snapshot: panel.snapshot,
    }
}

pub(crate) fn prepare_input() -> PrepareGroupPanelSynthesisInput {
    PrepareGroupPanelSynthesisInput {
        synthesis_id: SYNTHESIS_ID.into(),
        panel_id: PANEL_ID.into(),
        model: MODEL.into(),
        max_output_tokens: 64,
        idempotency_key: "synthesis-key".into(),
        created_at_ms: 60,
    }
}

pub(crate) fn claim_request() -> ClaimGroupPanelSynthesisDispatch {
    ClaimGroupPanelSynthesisDispatch {
        v: GROUP_PANEL_SYNTHESIS_VERSION,
        synthesis_id: SYNTHESIS_ID.into(),
        dispatch_id: "synthesis-dispatch-1".into(),
        consent_version: GROUP_PANEL_SYNTHESIS_CONSENT_VERSION,
        released_at_ms: 70,
    }
}

struct JsonCodec;

impl GroupModelAnalysisRequestCodec for JsonCodec {
    fn encode_request(
        &self,
        model: &str,
        request: &ModelRequest,
    ) -> Result<Vec<u8>, ProviderError> {
        serde_json::to_vec(&serde_json::json!({
            "max_output_tokens": request.max_output_tokens,
            "messages": request.messages,
            "model": model,
            "store": false,
            "stream": true,
            "system_prompt": request.system_prompt,
            "tools": request.tools,
        }))
        .map_err(|error| ProviderError::new("encode", error.to_string(), false))
    }

    fn validate_exact_request(
        &self,
        model: &str,
        expected: &ModelRequest,
        actual: &[u8],
    ) -> Result<(), ProviderError> {
        let encoded = self.encode_request(model, expected)?;
        (encoded == actual)
            .then_some(())
            .ok_or_else(|| ProviderError::new("mismatch", "exact request mismatch", false))
    }
}

pub(crate) struct ScriptedProvider {
    target: String,
    model: String,
    events: Mutex<Option<Vec<Result<ModelEvent, ProviderError>>>>,
    bodies: Mutex<Vec<Vec<u8>>>,
}

impl ScriptedProvider {
    pub(crate) fn new(events: Vec<Result<ModelEvent, ProviderError>>) -> Self {
        Self {
            target: GROUP_PANEL_SYNTHESIS_PROVIDER_ENDPOINT.into(),
            model: MODEL.into(),
            events: Mutex::new(Some(events)),
            bodies: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn with_target(mut self, endpoint: &str, model: &str) -> Self {
        self.target = endpoint.into();
        self.model = model.into();
        self
    }

    pub(crate) fn calls(&self) -> usize {
        self.bodies.lock().expect("provider bodies").len()
    }

    pub(crate) fn first_body(&self) -> Vec<u8> {
        self.bodies
            .lock()
            .expect("provider bodies")
            .first()
            .cloned()
            .expect("provider called")
    }
}

impl PreparedModelProvider for ScriptedProvider {
    fn stream_prepared(&self, request: PreparedModelRequest) -> ModelEventStream {
        let (body, _) = request.into_parts();
        self.bodies.lock().expect("provider bodies").push(body);
        let events = self
            .events
            .lock()
            .expect("provider events")
            .take()
            .unwrap_or_default();
        Box::pin(stream::iter(events))
    }
}

impl GroupPanelSynthesisDispatchProvider for ScriptedProvider {
    fn synthesis_provider(&self) -> GroupPanelSynthesisProvider {
        GroupPanelSynthesisProvider::OpenAiResponses
    }

    fn endpoint(&self) -> &str {
        &self.target
    }

    fn model(&self) -> &str {
        &self.model
    }
}

pub(crate) fn replace_analysis(
    harness: &Harness,
    id: &str,
    inspection: GroupModelAnalysisInspection,
) {
    harness.analyses.respond_with(id, inspection);
}

pub(crate) fn only_user_message(request: &serde_json::Value) -> Option<String> {
    let messages: Vec<Message> = serde_json::from_value(request["messages"].clone()).ok()?;
    match messages.as_slice() {
        [Message::User { text }] => Some(text.clone()),
        _ => None,
    }
}
