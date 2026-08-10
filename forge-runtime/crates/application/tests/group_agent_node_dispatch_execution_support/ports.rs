use std::sync::{
    Arc, Barrier, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use forge_runtime_application::{
    ExecuteGroupAgentNodeDispatchInput, GroupAgentNodeCredentialSource,
    GroupAgentNodeCredentialSourceError, GroupAgentNodeDispatchClaimMetadata,
    GroupAgentNodeDispatchExecutionService, GroupAgentNodeDispatchMetadataSource,
    GroupAgentNodeDispatchMetadataSourceError,
};
use forge_runtime_domain::{
    Cancellation, GROUP_AGENT_NODE_TERMINAL_RECEIPT_PROTOCOL_VERSION,
    GROUP_AGENT_NODE_TERMINAL_RECEIPT_VERSION, GroupAgentGraphRunStatus,
    GroupAgentNodeCoreTerminalReceiptEnvelope, GroupAgentNodeCoreTerminalReceiptPort,
    GroupAgentNodeCoreTerminalReceiptPortError, GroupAgentNodeDispatchAuthorization,
    GroupAgentNodeDispatchProviderFactory, GroupAgentNodeDispatchProviderFactoryError,
    GroupAgentNodePricingSnapshot, GroupAgentNodeResolvedDispatch,
    GroupAgentNodeTerminalClassification, GroupAgentNodeTerminalControl,
    GroupAgentNodeTerminalOutcome, GroupAgentNodeTerminalReceipt, ModelEvent, ModelEventStream,
    PreparedModelProvider, PreparedModelRequest, ProviderError,
    group_agent_node_terminal_receipt_id,
};
use futures_util::stream;

use super::data::{ExactJsonCodec, PreparedExecution, prepare, prepare_with_max_result_bytes};
use crate::group_agent_node_execution_support::MemoryContractHub;

pub(crate) struct ExecutionHarness {
    pub(crate) service: Arc<GroupAgentNodeDispatchExecutionService>,
    pub(crate) input: ExecuteGroupAgentNodeDispatchInput,
    pub(crate) hub: Arc<MemoryContractHub>,
    pub(crate) codec: Arc<ExactJsonCodec>,
    pub(crate) provider_calls: Arc<AtomicUsize>,
    pub(crate) credential_reads: Arc<AtomicUsize>,
    pub(crate) core_calls: Arc<AtomicUsize>,
}

impl ExecutionHarness {
    pub(crate) fn new(events: Vec<Result<ModelEvent, ProviderError>>, reject_core: bool) -> Self {
        Self::with_options(events, reject_core, false, None, false)
    }

    pub(crate) fn with_drifted_quote(events: Vec<Result<ModelEvent, ProviderError>>) -> Self {
        Self::with_options(events, false, true, None, false)
    }

    pub(crate) fn with_max_result_bytes(
        events: Vec<Result<ModelEvent, ProviderError>>,
        max_result_bytes: usize,
    ) -> Self {
        Self::with_options(events, false, false, Some(max_result_bytes), false)
    }

    pub(crate) fn concurrent(events: Vec<Result<ModelEvent, ProviderError>>) -> Self {
        Self::with_options(events, false, false, None, true)
    }

    fn with_options(
        events: Vec<Result<ModelEvent, ProviderError>>,
        reject_core: bool,
        drift_quote: bool,
        max_result_bytes: Option<usize>,
        synchronize_builds: bool,
    ) -> Self {
        let prepared = max_result_bytes.map_or_else(prepare, prepare_with_max_result_bytes);
        let input = execution_input(&prepared);
        let provider_calls = Arc::new(AtomicUsize::new(0));
        let credential_reads = Arc::new(AtomicUsize::new(0));
        let core_calls = Arc::new(AtomicUsize::new(0));
        let hub = prepared.hub.clone();
        let codec = prepared.codec.clone();
        let providers = deterministic_factory(
            events,
            provider_calls.clone(),
            drift_quote,
            synchronize_builds,
        );
        let credentials = Arc::new(DeterministicCredential {
            reads: credential_reads.clone(),
        });
        let core = Arc::new(DeterministicCore {
            reject: reject_core,
            calls: core_calls.clone(),
        });
        let service = Arc::new(GroupAgentNodeDispatchExecutionService::new(
            hub.clone(),
            hub.clone(),
            hub.clone(),
            hub.clone(),
            codec.clone(),
            providers,
            credentials,
            core,
            Arc::new(DeterministicMetadata),
        ));
        Self {
            service,
            input,
            hub,
            codec,
            provider_calls,
            credential_reads,
            core_calls,
        }
    }
}

fn execution_input(prepared: &PreparedExecution) -> ExecuteGroupAgentNodeDispatchInput {
    ExecuteGroupAgentNodeDispatchInput {
        graph_run_id: prepared.fixture.run.run.graph_run_id.clone(),
        authorization_json: prepared.authorization_json.clone(),
        pricing_json: prepared.pricing_json.clone(),
        confirm_off_machine: true,
        cancellation: Cancellation::default(),
    }
}

struct DeterministicFactory {
    events: Vec<Result<ModelEvent, ProviderError>>,
    calls: Arc<AtomicUsize>,
    drift_quote: bool,
    build_barrier: Option<Arc<Barrier>>,
}

impl GroupAgentNodeDispatchProviderFactory for DeterministicFactory {
    fn resolve(
        &self,
        authorization: &GroupAgentNodeDispatchAuthorization,
        pricing: &GroupAgentNodePricingSnapshot,
    ) -> Result<GroupAgentNodeResolvedDispatch, GroupAgentNodeDispatchProviderFactoryError> {
        let quote = pricing
            .verify_authorization(authorization)
            .map_err(|_| factory_error())?;
        let max_cost_usd_micros = if self.drift_quote {
            quote.max_cost_usd_micros.saturating_sub(1)
        } else {
            quote.max_cost_usd_micros
        };
        Ok(GroupAgentNodeResolvedDispatch {
            authorization_sha256: authorization.authorization_sha256.clone(),
            provider_kind: authorization.provider_kind,
            endpoint: authorization.endpoint.clone(),
            model: authorization.model.clone(),
            destination_sha256: authorization.destination_sha256.clone(),
            pricing_snapshot_sha256: pricing.pricing_snapshot_sha256.clone(),
            max_input_tokens: quote.max_input_tokens,
            max_output_tokens: quote.max_output_tokens,
            max_cost_usd_micros,
        })
    }

    fn build(
        &self,
        _resolved: GroupAgentNodeResolvedDispatch,
        credential: String,
    ) -> Result<Box<dyn PreparedModelProvider>, GroupAgentNodeDispatchProviderFactoryError> {
        if credential != "test-secret" {
            return Err(factory_error());
        }
        if let Some(barrier) = &self.build_barrier {
            barrier.wait();
        }
        Ok(Box::new(DeterministicProvider {
            events: Mutex::new(Some(self.events.clone())),
            calls: self.calls.clone(),
        }))
    }
}

fn deterministic_factory(
    events: Vec<Result<ModelEvent, ProviderError>>,
    calls: Arc<AtomicUsize>,
    drift_quote: bool,
    synchronize_builds: bool,
) -> Arc<DeterministicFactory> {
    Arc::new(DeterministicFactory {
        events,
        calls,
        drift_quote,
        build_barrier: synchronize_builds.then(|| Arc::new(Barrier::new(2))),
    })
}

struct DeterministicProvider {
    events: Mutex<Option<Vec<Result<ModelEvent, ProviderError>>>>,
    calls: Arc<AtomicUsize>,
}

impl PreparedModelProvider for DeterministicProvider {
    fn stream_prepared(&self, request: PreparedModelRequest) -> ModelEventStream {
        assert!(!request.body().is_empty());
        self.calls.fetch_add(1, Ordering::AcqRel);
        let events = self
            .events
            .lock()
            .expect("provider events")
            .take()
            .unwrap_or_default();
        Box::pin(stream::iter(events))
    }
}

struct DeterministicCredential {
    reads: Arc<AtomicUsize>,
}

impl GroupAgentNodeCredentialSource for DeterministicCredential {
    fn read_credential(&self) -> Result<String, GroupAgentNodeCredentialSourceError> {
        self.reads.fetch_add(1, Ordering::AcqRel);
        Ok("test-secret".into())
    }
}

pub(crate) struct DeterministicMetadata;

impl GroupAgentNodeDispatchMetadataSource for DeterministicMetadata {
    fn claim_metadata(
        &self,
    ) -> Result<GroupAgentNodeDispatchClaimMetadata, GroupAgentNodeDispatchMetadataSourceError>
    {
        Ok(GroupAgentNodeDispatchClaimMetadata {
            dispatch_id: "dispatch-test-1".into(),
            lane_ownership_id: "lane-ownership-test-1".into(),
            released_at_ms: 100,
        })
    }

    fn terminal_time_ms(&self) -> Result<u64, GroupAgentNodeDispatchMetadataSourceError> {
        Ok(110)
    }
}

pub(crate) struct DeterministicCore {
    pub(crate) reject: bool,
    pub(crate) calls: Arc<AtomicUsize>,
}

impl GroupAgentNodeCoreTerminalReceiptPort for DeterministicCore {
    fn decide(
        &self,
        control: &GroupAgentNodeTerminalControl,
    ) -> Result<GroupAgentNodeCoreTerminalReceiptEnvelope, GroupAgentNodeCoreTerminalReceiptPortError>
    {
        self.calls.fetch_add(1, Ordering::AcqRel);
        if self.reject {
            return Err(core_error());
        }
        let mut receipt = receipt(control);
        receipt.receipt_sha256 = receipt.expected_sha256().map_err(|_| core_error())?;
        receipt.receipt_id = group_agent_node_terminal_receipt_id(&receipt.receipt_sha256);
        let receipt_json = receipt.canonical_json().map_err(|_| core_error())?;
        let envelope = GroupAgentNodeCoreTerminalReceiptEnvelope {
            receipt,
            receipt_json,
        };
        envelope
            .validate_against_control(control)
            .map_err(|_| core_error())?;
        Ok(envelope)
    }
}

pub(crate) fn receipt(control: &GroupAgentNodeTerminalControl) -> GroupAgentNodeTerminalReceipt {
    let (node_outcome, graph_status) = outcome(control.artifact.classification);
    GroupAgentNodeTerminalReceipt {
        v: GROUP_AGENT_NODE_TERMINAL_RECEIPT_VERSION,
        scheduler_protocol_version: control.scheduler_protocol_version,
        terminal_receipt_protocol_version: GROUP_AGENT_NODE_TERMINAL_RECEIPT_PROTOCOL_VERSION,
        terminal_control_sha256: control.snapshot_sha256.clone(),
        expected_last_event_seq: 4,
        expected_last_event_sha256: control.claim.claim_event_sha256.clone(),
        graph_run_id: control.claim.graph_run_id.clone(),
        graph_id: control.graph_run.graph_id.clone(),
        node_id: control.claim.node_id.clone(),
        attempt: control.claim.attempt,
        dispatch_id: control.claim.dispatch_id.clone(),
        lane_ownership_id: control.claim.lane_ownership_id.clone(),
        project_lane_sha256: control.claim.project_lane_sha256.clone(),
        artifact_kind: control.artifact.artifact_kind,
        artifact_id: control.artifact.artifact_id.clone(),
        artifact_sha256: control.artifact.artifact_sha256.clone(),
        node_outcome,
        wave_index: 0,
        wave_outcome: node_outcome,
        graph_status,
        retry_authorized: false,
        lane_release_authorized: true,
        receipt_id: String::new(),
        receipt_sha256: String::new(),
    }
}

fn outcome(
    classification: GroupAgentNodeTerminalClassification,
) -> (GroupAgentNodeTerminalOutcome, GroupAgentGraphRunStatus) {
    match classification {
        GroupAgentNodeTerminalClassification::Completed => (
            GroupAgentNodeTerminalOutcome::Completed,
            GroupAgentGraphRunStatus::Completed,
        ),
        GroupAgentNodeTerminalClassification::Length => (
            GroupAgentNodeTerminalOutcome::Failed,
            GroupAgentGraphRunStatus::Failed,
        ),
        _ => (
            GroupAgentNodeTerminalOutcome::FailedUncertain,
            GroupAgentGraphRunStatus::FailedUncertain,
        ),
    }
}

fn factory_error() -> GroupAgentNodeDispatchProviderFactoryError {
    GroupAgentNodeDispatchProviderFactoryError {
        message: "test provider unavailable".into(),
    }
}

fn core_error() -> GroupAgentNodeCoreTerminalReceiptPortError {
    GroupAgentNodeCoreTerminalReceiptPortError {
        message: "test Core rejected".into(),
    }
}
