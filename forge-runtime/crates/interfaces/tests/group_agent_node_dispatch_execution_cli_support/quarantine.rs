use std::sync::Arc;

use forge_runtime_application::{
    ExecuteGroupAgentNodeDispatchInput, GroupAgentNodeCredentialSource,
    GroupAgentNodeCredentialSourceError, GroupAgentNodeDispatchClaimMetadata,
    GroupAgentNodeDispatchExecutionService, GroupAgentNodeDispatchExecutionServiceError,
    GroupAgentNodeDispatchMetadataSource, GroupAgentNodeDispatchMetadataSourceError,
    GroupAgentNodeDispatchRequestCodec,
};
use forge_runtime_domain::{
    Cancellation, GroupAgentGraphRunStatus, GroupAgentNodeCoreTerminalReceiptEnvelope,
    GroupAgentNodeCoreTerminalReceiptPort, GroupAgentNodeCoreTerminalReceiptPortError,
    GroupAgentNodeDispatchAuthorization, GroupAgentNodeDispatchProviderFactory,
    GroupAgentNodeDispatchProviderFactoryError, GroupAgentNodeLifecycleStore,
    GroupAgentNodePricingSnapshot, GroupAgentNodeResolvedDispatch, GroupAgentNodeTerminalControl,
    ModelEvent, ModelEventStream, ModelFinishReason, ModelRequest, PreparedModelProvider,
    PreparedModelRequest, ProviderError, Usage,
};
use forge_runtime_infrastructure::{
    OpenAiResponsesProvider, RegisteredGroupAgentNodeProviderFactory, SqliteHubStore,
};
use futures_util::stream;
use rusqlite::Connection;

use super::*;

impl Fixture {
    pub(crate) fn lifecycle_inspection(&self) -> GroupAgentNodeLifecycleInspection {
        let database = self.state.path().join("hub.sqlite3");
        let store = Arc::new(SqliteHubStore::open(database).expect("open test Hub"));
        store
            .inspect_group_agent_node_lifecycle(&self.graph_run_id)
            .expect("inspect stranded v4 lifecycle")
    }

    pub(crate) fn strand_v4_with_local_provider(&self) {
        let database = self.state.path().join("hub.sqlite3");
        let store = Arc::new(SqliteHubStore::open(database).expect("open test Hub"));
        let service = GroupAgentNodeDispatchExecutionService::new(
            store.clone(),
            store.clone(),
            store.clone(),
            store.clone(),
            Arc::new(ExactOpenAiCodec),
            Arc::new(LocalProviderFactory),
            Arc::new(LocalCredential),
            Arc::new(RejectingCore),
            Arc::new(RealTimeMetadata),
        );
        let input = ExecuteGroupAgentNodeDispatchInput {
            graph_run_id: self.graph_run_id.clone(),
            authorization_json: fs::read_to_string(&self.authorization_path)
                .expect("authorization fixture"),
            pricing_json: fs::read_to_string(&self.pricing_path).expect("pricing fixture"),
            confirm_off_machine: true,
            cancellation: Cancellation::default(),
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("test runtime");
        let error = runtime
            .block_on(service.execute(&input))
            .expect_err("rejecting Core must strand the durable claim");
        assert!(
            matches!(
                error,
                GroupAgentNodeDispatchExecutionServiceError::DispatchQuarantined
            ),
            "unexpected local dispatch failure: {error:?}"
        );
        let lifecycle = store
            .inspect_group_agent_node_lifecycle(&self.graph_run_id)
            .expect("durable v4 lifecycle");
        assert_eq!(
            lifecycle.graph_run.run.status,
            GroupAgentGraphRunStatus::DispatchUnknown
        );
        assert!(lifecycle.active_lane.is_some());
    }

    pub(crate) fn remove_execution_preflight_files(&self) {
        fs::remove_file(&self.authorization_path).expect("remove authorization fixture");
        fs::remove_file(&self.pricing_path).expect("remove pricing fixture");
        fs::remove_file(&self.core_bin).expect("remove pinned Core fixture");
    }

    pub(crate) fn hold_wal_open(&self) -> Connection {
        let connection = Connection::open(self.state.path().join("hub.sqlite3"))
            .expect("open WAL keeper connection");
        connection
            .pragma_update(None, "wal_autocheckpoint", 0)
            .expect("disable WAL keeper autocheckpoint");
        let journal_mode: String = connection
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .expect("read WAL mode");
        assert_eq!(journal_mode, "wal");
        connection
    }

    pub(crate) fn database_bytes(&self) -> Vec<u8> {
        fs::read(self.state.path().join("hub.sqlite3")).expect("Hub database bytes")
    }

    pub(crate) fn wal_bytes(&self) -> Vec<u8> {
        fs::read(self.state.path().join("hub.sqlite3-wal")).expect("Hub WAL bytes")
    }

    pub(crate) fn shm_exists(&self) -> bool {
        self.state.path().join("hub.sqlite3-shm").exists()
    }
}

struct ExactOpenAiCodec;

impl GroupAgentNodeDispatchRequestCodec for ExactOpenAiCodec {
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

struct LocalProviderFactory;

impl GroupAgentNodeDispatchProviderFactory for LocalProviderFactory {
    fn resolve(
        &self,
        authorization: &GroupAgentNodeDispatchAuthorization,
        pricing: &GroupAgentNodePricingSnapshot,
    ) -> Result<GroupAgentNodeResolvedDispatch, GroupAgentNodeDispatchProviderFactoryError> {
        GroupAgentNodeDispatchProviderFactory::resolve(
            &RegisteredGroupAgentNodeProviderFactory::new(),
            authorization,
            pricing,
        )
    }

    fn build(
        &self,
        _resolved: GroupAgentNodeResolvedDispatch,
        credential: String,
    ) -> Result<Box<dyn PreparedModelProvider>, GroupAgentNodeDispatchProviderFactoryError> {
        assert_eq!(credential, "local-test-credential");
        Ok(Box::new(LocalProvider))
    }
}

struct LocalProvider;

impl PreparedModelProvider for LocalProvider {
    fn stream_prepared(&self, request: PreparedModelRequest) -> ModelEventStream {
        assert!(!request.body().is_empty());
        Box::pin(stream::iter([
            Ok(ModelEvent::TextDelta {
                delta: "local-result".into(),
            }),
            Ok(ModelEvent::Usage {
                usage: Usage {
                    input_tokens: 1,
                    output_tokens: 1,
                },
            }),
            Ok(ModelEvent::Finished {
                reason: ModelFinishReason::Completed,
            }),
        ]))
    }
}

struct LocalCredential;

impl GroupAgentNodeCredentialSource for LocalCredential {
    fn read_credential(&self) -> Result<String, GroupAgentNodeCredentialSourceError> {
        Ok("local-test-credential".into())
    }
}

/// Real-time claim metadata with deterministic identities.
///
/// The claim's `released_at_ms` must be at or after the dispatch request's
/// `created_at_ms` (real wall clock at fixture preparation) and the adjudication
/// remedy must observe `artifact.created_at_ms >= claim.released_at_ms` — so a
/// fixed past or future constant is invalid for this fixture. Real time is the
/// only honest choice; the identities stay deterministic for reproducibility.
struct RealTimeMetadata;

impl GroupAgentNodeDispatchMetadataSource for RealTimeMetadata {
    fn claim_metadata(
        &self,
    ) -> Result<GroupAgentNodeDispatchClaimMetadata, GroupAgentNodeDispatchMetadataSourceError>
    {
        Ok(GroupAgentNodeDispatchClaimMetadata {
            dispatch_id: "dispatch-cli-quarantine".into(),
            lane_ownership_id: "lane-owner-cli-quarantine".into(),
            released_at_ms: now_ms()?,
        })
    }

    fn terminal_time_ms(&self) -> Result<u64, GroupAgentNodeDispatchMetadataSourceError> {
        now_ms()
    }
}

fn now_ms() -> Result<u64, GroupAgentNodeDispatchMetadataSourceError> {
    let milliseconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| GroupAgentNodeDispatchMetadataSourceError)?
        .as_millis();
    u64::try_from(milliseconds)
        .ok()
        .filter(|value| i64::try_from(*value).is_ok())
        .ok_or(GroupAgentNodeDispatchMetadataSourceError)
}

struct RejectingCore;

impl GroupAgentNodeCoreTerminalReceiptPort for RejectingCore {
    fn decide(
        &self,
        _control: &GroupAgentNodeTerminalControl,
    ) -> Result<GroupAgentNodeCoreTerminalReceiptEnvelope, GroupAgentNodeCoreTerminalReceiptPortError>
    {
        Err(GroupAgentNodeCoreTerminalReceiptPortError {
            message: "intentional local Core rejection".into(),
        })
    }
}
