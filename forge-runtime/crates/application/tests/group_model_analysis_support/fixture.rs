use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use forge_runtime_application::{
    GroupModelAnalysisDispatchProvider, GroupModelAnalysisRequestCodec, GroupModelAnalysisService,
    PrepareGroupModelAnalysisInput,
};
use forge_runtime_domain::{
    ClaimGroupModelAnalysisDispatch, GROUP_CONTEXT_DIGEST_DOMAIN, GROUP_CONTEXT_VERSION,
    GROUP_MODEL_ANALYSIS_CONSENT_VERSION, GROUP_MODEL_ANALYSIS_PROVIDER_ENDPOINT,
    GROUP_MODEL_ANALYSIS_VERSION, GROUP_RUN_SNAPSHOT_DIGEST_DOMAIN, GROUP_RUN_VERSION,
    GroupContextPayload, GroupContextPolicy, GroupContextSlice, GroupContextStats,
    GroupModelAnalysisProvider, GroupRunRecord, GroupRunSnapshot, GroupRunStatus, GroupRunStore,
    HubEntity, HubStoreError, ModelEvent, ModelEventStream, ModelRequest, PrepareGroupRun,
    PrepareGroupRunResult, PreparedModelProvider, PreparedModelRequest, ProviderError,
    SessionGroup,
};
use futures_util::stream;
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::group_model_analysis_support::MemoryAnalysisStore;

pub(crate) const ANALYSIS_ID: &str = "analysis-1";
pub(crate) const GROUP_RUN_ID: &str = "group-run-1";

pub(crate) struct Harness {
    pub(crate) service: GroupModelAnalysisService,
    pub(crate) analyses: Arc<MemoryAnalysisStore>,
    pub(crate) codec: Arc<JsonCodec>,
    pub(crate) context_json: String,
}

pub(crate) fn harness() -> Harness {
    let snapshot = snapshot();
    let context_json = snapshot.context_json.clone();
    let group_runs = Arc::new(FrozenRunStore { snapshot });
    let analyses = Arc::new(MemoryAnalysisStore::default());
    let codec = Arc::new(JsonCodec::default());
    let service = GroupModelAnalysisService::new(group_runs, analyses.clone(), codec.clone());
    Harness {
        service,
        analyses,
        codec,
        context_json,
    }
}

pub(crate) fn prepare_input(max_output_tokens: u32) -> PrepareGroupModelAnalysisInput {
    PrepareGroupModelAnalysisInput {
        analysis_id: ANALYSIS_ID.into(),
        group_run_id: GROUP_RUN_ID.into(),
        model: "gpt-test".into(),
        endpoint: GROUP_MODEL_ANALYSIS_PROVIDER_ENDPOINT.to_owned(),
        max_output_tokens,
        idempotency_key: "analysis-key".into(),
        created_at_ms: 10,
    }
}

pub(crate) fn claim_request() -> ClaimGroupModelAnalysisDispatch {
    ClaimGroupModelAnalysisDispatch {
        v: GROUP_MODEL_ANALYSIS_VERSION,
        analysis_id: ANALYSIS_ID.into(),
        dispatch_id: "dispatch-1".into(),
        consent_version: GROUP_MODEL_ANALYSIS_CONSENT_VERSION,
        released_at_ms: 20,
    }
}

#[derive(Default)]
pub(crate) struct JsonCodec {
    encode_calls: Mutex<usize>,
}

impl JsonCodec {
    pub(crate) fn encode_calls(&self) -> usize {
        *self.encode_calls.lock().expect("codec calls")
    }

    fn body(model: &str, request: &ModelRequest) -> Result<Vec<u8>, ProviderError> {
        canonical(&json!({
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
}

impl GroupModelAnalysisRequestCodec for JsonCodec {
    fn encode_request(
        &self,
        model: &str,
        request: &ModelRequest,
    ) -> Result<Vec<u8>, ProviderError> {
        let mut calls = self.encode_calls.lock().expect("codec calls");
        *calls = calls.saturating_add(1);
        Self::body(model, request)
    }

    fn validate_exact_request(
        &self,
        model: &str,
        expected: &ModelRequest,
        actual: &[u8],
    ) -> Result<(), ProviderError> {
        if Self::body(model, expected)? == actual {
            Ok(())
        } else {
            Err(ProviderError::new(
                "request_mismatch",
                "prepared request bytes changed",
                false,
            ))
        }
    }
}

pub(crate) struct ScriptedProvider {
    events: Mutex<Option<Vec<Result<ModelEvent, ProviderError>>>>,
    bodies: Mutex<Vec<Vec<u8>>>,
    endpoint: String,
    model: String,
}

impl ScriptedProvider {
    pub(crate) fn new(events: Vec<Result<ModelEvent, ProviderError>>) -> Self {
        Self {
            events: Mutex::new(Some(events)),
            bodies: Mutex::new(Vec::new()),
            endpoint: GROUP_MODEL_ANALYSIS_PROVIDER_ENDPOINT.into(),
            model: "gpt-test".into(),
        }
    }

    pub(crate) fn with_target(mut self, endpoint: &str, model: &str) -> Self {
        self.endpoint = endpoint.into();
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
            .expect("provider was called")
    }
}

impl PreparedModelProvider for ScriptedProvider {
    fn stream_prepared(&self, request: PreparedModelRequest) -> ModelEventStream {
        let (body, _cancellation) = request.into_parts();
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

impl GroupModelAnalysisDispatchProvider for ScriptedProvider {
    fn analysis_provider(&self) -> GroupModelAnalysisProvider {
        GroupModelAnalysisProvider::OpenAiResponses
    }

    fn endpoint(&self) -> &str {
        &self.endpoint
    }

    fn model(&self) -> &str {
        &self.model
    }
}

#[derive(Default)]
pub(crate) struct PendingProvider {
    calls: AtomicUsize,
}

impl PendingProvider {
    pub(crate) fn calls(&self) -> usize {
        self.calls.load(Ordering::Acquire)
    }
}

impl PreparedModelProvider for PendingProvider {
    fn stream_prepared(&self, _request: PreparedModelRequest) -> ModelEventStream {
        self.calls.fetch_add(1, Ordering::AcqRel);
        Box::pin(stream::pending())
    }
}

impl GroupModelAnalysisDispatchProvider for PendingProvider {
    fn analysis_provider(&self) -> GroupModelAnalysisProvider {
        GroupModelAnalysisProvider::OpenAiResponses
    }

    fn endpoint(&self) -> &str {
        GROUP_MODEL_ANALYSIS_PROVIDER_ENDPOINT
    }

    fn model(&self) -> &'static str {
        "gpt-test"
    }
}

struct FrozenRunStore {
    snapshot: GroupRunSnapshot,
}

impl GroupRunStore for FrozenRunStore {
    fn prepare_group_run(
        &self,
        _request: &PrepareGroupRun,
    ) -> Result<PrepareGroupRunResult, HubStoreError> {
        Err(unavailable("prepare is not used"))
    }

    fn inspect_group_run(&self, run_id: &str) -> Result<GroupRunSnapshot, HubStoreError> {
        if run_id == self.snapshot.run.run_id {
            Ok(self.snapshot.clone())
        } else {
            Err(HubStoreError::NotFound {
                entity: HubEntity::GroupRun,
                id: run_id.into(),
            })
        }
    }

    fn list_group_runs(
        &self,
        _group_id: Option<&str>,
        _limit: usize,
    ) -> Result<Vec<GroupRunRecord>, HubStoreError> {
        Err(unavailable("list is not used"))
    }
}

fn snapshot() -> GroupRunSnapshot {
    let payload = GroupContextPayload {
        policy: GroupContextPolicy::default(),
        group: SessionGroup {
            id: "group-1".into(),
            name: "Delivery".into(),
            created_at_ms: 1,
        },
        members: Vec::new(),
        conversations: Vec::new(),
        stats: GroupContextStats::default(),
    };
    let slice_sha256 = digest(
        GROUP_CONTEXT_DIGEST_DOMAIN,
        &canonical(&payload).expect("payload"),
    );
    let context = GroupContextSlice {
        v: GROUP_CONTEXT_VERSION,
        payload,
        slice_sha256: slice_sha256.clone(),
    };
    let context_bytes = canonical(&context).expect("context");
    GroupRunSnapshot {
        v: GROUP_RUN_VERSION,
        run: GroupRunRecord {
            v: GROUP_RUN_VERSION,
            run_id: GROUP_RUN_ID.into(),
            group_id: "group-1".into(),
            status: GroupRunStatus::Prepared,
            context_version: GROUP_CONTEXT_VERSION,
            context_slice_sha256: slice_sha256,
            snapshot_sha256: digest(GROUP_RUN_SNAPSHOT_DIGEST_DOMAIN, &context_bytes),
            snapshot_bytes: context_bytes.len(),
            created_at_ms: 5,
        },
        context,
        context_json: String::from_utf8(context_bytes).expect("context UTF-8"),
    }
}

pub(crate) fn canonical(value: &impl Serialize) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_value(value).and_then(|value| serde_json::to_vec(&sort_json(value)))
}

pub(crate) fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn sort_json(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(sort_json).collect()),
        Value::Object(items) => {
            let mut sorted: Vec<_> = items.into_iter().collect();
            sorted.sort_by(|left, right| left.0.cmp(&right.0));
            Value::Object(
                sorted
                    .into_iter()
                    .map(|(key, value)| (key, sort_json(value)))
                    .collect(),
            )
        }
        other => other,
    }
}

fn unavailable(message: &str) -> HubStoreError {
    HubStoreError::Unavailable {
        message: message.into(),
    }
}
