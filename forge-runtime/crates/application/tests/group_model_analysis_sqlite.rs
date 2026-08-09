use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use forge_runtime_application::{
    GroupModelAnalysisDispatchProvider, GroupModelAnalysisRequestCodec, GroupModelAnalysisService,
    PrepareGroupModelAnalysisInput, SendGroupModelAnalysisResult,
};
use forge_runtime_domain::{
    Cancellation, ClaimGroupModelAnalysisDispatch, GROUP_MODEL_ANALYSIS_CONSENT_VERSION,
    GROUP_MODEL_ANALYSIS_PROVIDER_ENDPOINT, GROUP_MODEL_ANALYSIS_VERSION, GROUP_RUN_VERSION,
    GroupContextPolicy, GroupModelAnalysisOutcome, GroupModelAnalysisProvider,
    GroupModelAnalysisRecovery, GroupModelAnalysisStore, GroupRunStore, HubStore, ModelEvent,
    ModelEventStream, ModelFinishReason, ModelRequest, PreparedModelProvider, PreparedModelRequest,
    ProviderError, Usage,
};
use forge_runtime_infrastructure::{OpenAiResponsesProvider, SqliteHubStore};
use futures_util::stream;
use tempfile::TempDir;

const ANALYSIS_ID: &str = "analysis-sqlite-1";
const GROUP_RUN_ID: &str = "group-run-sqlite-1";
const MODEL: &str = "gpt-test";

#[tokio::test]
async fn application_completion_round_trips_through_sqlite() {
    let fixture = Fixture::new();
    let service = fixture.service();
    service.prepare(&prepare_input()).expect("prepare analysis");
    let provider = FakeProvider::default();

    let sent = service
        .send(&claim_request(), &provider, Cancellation::default(), 31)
        .await
        .expect("complete analysis");
    let SendGroupModelAnalysisResult::Completed { completion } = sent else {
        panic!("fresh SQLite dispatch must complete");
    };
    let artifact = completion.inspection.result.expect("completion result");

    assert_eq!(provider.calls(), 1);
    assert_eq!(artifact.result.answer, "persisted cross-project analysis");
    assert_eq!(
        artifact.result.outcome,
        GroupModelAnalysisOutcome::Completed
    );
    assert_reopened_result(&fixture.database, &artifact);
}

fn assert_reopened_result(
    database: &PathBuf,
    expected: &forge_runtime_domain::GroupModelAnalysisResultArtifact,
) {
    let reopened = SqliteHubStore::open(database).expect("reopen Hub");
    let inspection = reopened
        .inspect_group_model_analysis(ANALYSIS_ID)
        .expect("inspect persisted analysis");
    assert_eq!(inspection.result.as_ref(), Some(expected));
    assert!(matches!(
        inspection.recovery,
        GroupModelAnalysisRecovery::Terminal {
            outcome: GroupModelAnalysisOutcome::Completed
        }
    ));
}

struct Fixture {
    _root: TempDir,
    database: PathBuf,
    store: Arc<SqliteHubStore>,
}

impl Fixture {
    fn new() -> Self {
        let root = TempDir::new().expect("temporary Hub root");
        let database = root.path().join("state").join("hub.sqlite3");
        let store = Arc::new(SqliteHubStore::open(&database).expect("open Hub"));
        let group = store
            .create_group("SQLite analysis", "group-key")
            .expect("create Group");
        store
            .prepare_group_run(&forge_runtime_domain::PrepareGroupRun {
                v: GROUP_RUN_VERSION,
                run_id: GROUP_RUN_ID.into(),
                group_id: group.id,
                policy: GroupContextPolicy::default(),
                idempotency_key: "group-run-key".into(),
                created_at_ms: 10,
            })
            .expect("prepare Group Run");
        Self {
            _root: root,
            database,
            store,
        }
    }

    fn service(&self) -> GroupModelAnalysisService {
        GroupModelAnalysisService::new(
            self.store.clone(),
            self.store.clone(),
            Arc::new(OpenAiCodec),
        )
    }
}

fn prepare_input() -> PrepareGroupModelAnalysisInput {
    PrepareGroupModelAnalysisInput {
        analysis_id: ANALYSIS_ID.into(),
        group_run_id: GROUP_RUN_ID.into(),
        model: MODEL.into(),
        endpoint: GROUP_MODEL_ANALYSIS_PROVIDER_ENDPOINT.to_owned(),
        max_output_tokens: 64,
        idempotency_key: "analysis-key".into(),
        created_at_ms: 20,
    }
}

fn claim_request() -> ClaimGroupModelAnalysisDispatch {
    ClaimGroupModelAnalysisDispatch {
        v: GROUP_MODEL_ANALYSIS_VERSION,
        analysis_id: ANALYSIS_ID.into(),
        dispatch_id: "dispatch-sqlite-1".into(),
        consent_version: GROUP_MODEL_ANALYSIS_CONSENT_VERSION,
        released_at_ms: 30,
    }
}

struct OpenAiCodec;

impl GroupModelAnalysisRequestCodec for OpenAiCodec {
    fn encode_request(
        &self,
        model: &str,
        request: &ModelRequest,
    ) -> Result<Vec<u8>, ProviderError> {
        OpenAiResponsesProvider::encode_request_bytes(model, request)
    }

    fn validate_exact_request(
        &self,
        model: &str,
        expected: &ModelRequest,
        actual: &[u8],
    ) -> Result<(), ProviderError> {
        OpenAiResponsesProvider::validate_exact_request_bytes(model, expected, actual)
    }
}

#[derive(Default)]
struct FakeProvider {
    bodies: Mutex<Vec<Vec<u8>>>,
}

impl FakeProvider {
    fn calls(&self) -> usize {
        self.bodies.lock().expect("provider bodies").len()
    }
}

impl PreparedModelProvider for FakeProvider {
    fn stream_prepared(&self, request: PreparedModelRequest) -> ModelEventStream {
        let (body, _) = request.into_parts();
        self.bodies.lock().expect("provider bodies").push(body);
        Box::pin(stream::iter(success_events()))
    }
}

impl GroupModelAnalysisDispatchProvider for FakeProvider {
    fn analysis_provider(&self) -> GroupModelAnalysisProvider {
        GroupModelAnalysisProvider::OpenAiResponses
    }

    fn endpoint(&self) -> &str {
        GROUP_MODEL_ANALYSIS_PROVIDER_ENDPOINT
    }

    fn model(&self) -> &str {
        MODEL
    }
}

fn success_events() -> Vec<Result<ModelEvent, ProviderError>> {
    vec![
        Ok(ModelEvent::TextDelta {
            delta: "persisted cross-project analysis".into(),
        }),
        Ok(ModelEvent::Usage {
            usage: Usage {
                input_tokens: 12,
                output_tokens: 4,
            },
        }),
        Ok(ModelEvent::Finished {
            reason: ModelFinishReason::Completed,
        }),
    ]
}
