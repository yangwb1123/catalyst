use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use forge_runtime_application::{
    ExecuteGroupAgentScheduledNodeDispatchInput, GroupAgentNodeCredentialSource,
    GroupAgentNodeCredentialSourceError, GroupAgentNodeDispatchClaimMetadata,
    GroupAgentNodeDispatchMetadataSource, GroupAgentNodeDispatchMetadataSourceError,
    GroupAgentNodeDispatchRequestCodec, GroupAgentScheduledNodeDispatchExecutionService,
};
use forge_runtime_domain::*;
use forge_runtime_infrastructure::{OpenAiResponsesProvider, SqliteHubStore};
use futures_util::stream;

use super::sqlite_support;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreFault {
    ClaimAfterCommit,
    TerminalAfterCommit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreBehavior {
    Receipt,
    Fail,
}

#[derive(Clone, Default)]
pub struct Counters {
    provider: Arc<AtomicUsize>,
    core: Arc<AtomicUsize>,
}

impl Counters {
    pub fn snapshot(&self) -> (usize, usize) {
        (
            self.provider.load(Ordering::Acquire),
            self.core.load(Ordering::Acquire),
        )
    }
}

pub fn input(fixture: &sqlite_support::Fixture) -> ExecuteGroupAgentScheduledNodeDispatchInput {
    ExecuteGroupAgentScheduledNodeDispatchInput {
        provider_request_id: fixture.provider_request_id().into(),
        authorization_json: fixture.authorization_json().into(),
        pricing_json: fixture.pricing_json().into(),
        confirm_off_machine: true,
        confirm_predecessor_content: false,
        cancellation: Cancellation::default(),
    }
}

pub fn service(
    fixture: &sqlite_support::Fixture,
    counters: &Counters,
    fault: StoreFault,
    core: CoreBehavior,
) -> GroupAgentScheduledNodeDispatchExecutionService {
    let store = fixture.writer();
    let lifecycles = Arc::new(FaultStore {
        inner: store.clone(),
        fault,
    });
    GroupAgentScheduledNodeDispatchExecutionService::new_with_successors(
        store.clone(),
        store.clone(),
        store.clone(),
        store.clone(),
        store.clone(),
        store.clone(),
        Arc::new(OpenAiCodec),
        lifecycles,
        Arc::new(ProviderFactory {
            calls: counters.provider.clone(),
        }),
        Arc::new(Credential),
        Arc::new(TerminalCore {
            calls: counters.core.clone(),
            behavior: core,
        }),
        Arc::new(Metadata),
    )
}

struct FaultStore {
    inner: Arc<SqliteHubStore>,
    fault: StoreFault,
}

impl GroupAgentScheduledNodeLifecycleStore for FaultStore {
    fn claim_group_agent_scheduled_node_dispatch(
        &self,
        request: &ClaimGroupAgentScheduledNodeDispatch,
    ) -> Result<ClaimGroupAgentScheduledNodeDispatchResult, HubStoreError> {
        let result = self
            .inner
            .claim_group_agent_scheduled_node_dispatch(request)?;
        if self.fault == StoreFault::ClaimAfterCommit
            && matches!(
                &result,
                ClaimGroupAgentScheduledNodeDispatchResult::Claimed { .. }
            )
        {
            return Err(unavailable("claim response lost after commit"));
        }
        Ok(result)
    }

    fn terminalize_group_agent_scheduled_node_dispatch(
        &self,
        request: &TerminalizeGroupAgentScheduledNodeDispatch,
    ) -> Result<TerminalizeGroupAgentScheduledNodeDispatchResult, HubStoreError> {
        let result = self
            .inner
            .terminalize_group_agent_scheduled_node_dispatch(request)?;
        if self.fault == StoreFault::TerminalAfterCommit {
            return Err(unavailable("terminal response lost after commit"));
        }
        Ok(result)
    }

    fn inspect_group_agent_scheduled_node_lifecycle(
        &self,
        provider_request_id: &str,
    ) -> Result<GroupAgentScheduledNodeLifecycleInspection, HubStoreError> {
        self.inner
            .inspect_group_agent_scheduled_node_lifecycle(provider_request_id)
    }

    fn adjudicate_group_agent_scheduled_node_dispatch(
        &self,
        request: &AdjudicateGroupAgentScheduledNodeDispatch,
    ) -> Result<GroupAgentScheduledNodeLifecycleInspection, HubStoreError> {
        self.inner
            .adjudicate_group_agent_scheduled_node_dispatch(request)
    }
}

fn unavailable(message: &str) -> HubStoreError {
    HubStoreError::Unavailable {
        message: message.into(),
    }
}

struct OpenAiCodec;

impl GroupAgentNodeDispatchRequestCodec for OpenAiCodec {
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
        (self.encode_request(model, expected)? == actual)
            .then_some(())
            .ok_or_else(codec_error)
    }
}

fn codec_error() -> ProviderError {
    ProviderError::new("invalid_request", "request bytes disagree", false)
}

struct ProviderFactory {
    calls: Arc<AtomicUsize>,
}

impl GroupAgentScheduledNodeProviderFactory for ProviderFactory {
    fn resolve(
        &self,
        authorization: &GroupAgentScheduledNodeDispatchAuthorization,
        pricing: &GroupAgentNodePricingSnapshot,
    ) -> Result<GroupAgentScheduledNodeResolvedDispatch, GroupAgentScheduledNodeProviderFactoryError>
    {
        let quote = pricing
            .verify_scheduled_authorization(authorization)
            .map_err(|_| provider_factory_error())?;
        Ok(GroupAgentScheduledNodeResolvedDispatch {
            authorization_sha256: authorization.authorization_sha256.clone(),
            provider_kind: authorization.provider_kind,
            endpoint: authorization.endpoint.clone(),
            model: authorization.model.clone(),
            destination_sha256: authorization.destination_sha256.clone(),
            pricing_snapshot_sha256: authorization.pricing_snapshot_sha256.clone(),
            quote,
        })
    }

    fn build(
        &self,
        _resolved: GroupAgentScheduledNodeResolvedDispatch,
        credential: String,
    ) -> Result<Box<dyn PreparedModelProvider>, GroupAgentScheduledNodeProviderFactoryError> {
        if credential != "secret" {
            return Err(provider_factory_error());
        }
        Ok(Box::new(Provider {
            calls: self.calls.clone(),
        }))
    }
}

fn provider_factory_error() -> GroupAgentScheduledNodeProviderFactoryError {
    GroupAgentScheduledNodeProviderFactoryError {
        message: "test provider rejected".into(),
    }
}

struct Provider {
    calls: Arc<AtomicUsize>,
}

impl PreparedModelProvider for Provider {
    fn stream_prepared(&self, request: PreparedModelRequest) -> ModelEventStream {
        assert!(!request.body().is_empty());
        self.calls.fetch_add(1, Ordering::AcqRel);
        Box::pin(stream::iter([
            Ok(ModelEvent::TextDelta {
                delta: "done".into(),
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

struct Credential;

impl GroupAgentNodeCredentialSource for Credential {
    fn read_credential(&self) -> Result<String, GroupAgentNodeCredentialSourceError> {
        Ok("secret".into())
    }
}

struct Metadata;

impl GroupAgentNodeDispatchMetadataSource for Metadata {
    fn claim_metadata(
        &self,
    ) -> Result<GroupAgentNodeDispatchClaimMetadata, GroupAgentNodeDispatchMetadataSourceError>
    {
        Ok(GroupAgentNodeDispatchClaimMetadata {
            dispatch_id: "legacy-fault-dispatch".into(),
            lane_ownership_id: "legacy-fault-owner".into(),
            released_at_ms: 100,
        })
    }

    fn terminal_time_ms(&self) -> Result<u64, GroupAgentNodeDispatchMetadataSourceError> {
        Ok(200)
    }
}

struct TerminalCore {
    calls: Arc<AtomicUsize>,
    behavior: CoreBehavior,
}

impl GroupAgentScheduledNodeTerminalReceiptPort for TerminalCore {
    fn decide(
        &self,
        control: &GroupAgentScheduledNodeTerminalControl,
    ) -> Result<
        GroupAgentScheduledNodeCoreTerminalReceiptEnvelope,
        GroupAgentScheduledNodeTerminalReceiptPortError,
    > {
        self.calls.fetch_add(1, Ordering::AcqRel);
        if self.behavior == CoreBehavior::Fail {
            return Err(GroupAgentScheduledNodeTerminalReceiptPortError {
                message: "test Core refusal".into(),
            });
        }
        let receipt = terminal_receipt(control);
        Ok(GroupAgentScheduledNodeCoreTerminalReceiptEnvelope {
            receipt_json: receipt.canonical_json().expect("receipt JSON"),
            receipt,
        })
    }
}

fn terminal_receipt(
    control: &GroupAgentScheduledNodeTerminalControl,
) -> GroupAgentScheduledNodeTerminalReceipt {
    let artifact = &control.artifact;
    let mut value = GroupAgentScheduledNodeTerminalReceipt {
        v: GROUP_AGENT_SCHEDULED_NODE_TERMINAL_RECEIPT_VERSION,
        scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
        terminal_receipt_protocol_version: GROUP_AGENT_SCHEDULED_NODE_TERMINAL_PROTOCOL_VERSION,
        terminal_control_sha256: control.snapshot_sha256.clone(),
        graph_run_id: control.graph_run_id.clone(),
        graph_id: control.graph_id.clone(),
        node_id: control.node_id.clone(),
        attempt: control.attempt,
        dispatch_id: control.dispatch_id.clone(),
        provider_request_id: control.provider_request_id.clone(),
        project_lane_sha256: control.project_lane_sha256.clone(),
        artifact_kind: artifact.artifact_kind,
        artifact_id: artifact.artifact_id.clone(),
        artifact_sha256: artifact.artifact_sha256.clone(),
        node_outcome: GroupAgentNodeTerminalOutcome::Completed,
        retry_authorized: false,
        lane_release_authorized: true,
        successor_advance_authorized: false,
        receipt_id: String::new(),
        receipt_sha256: String::new(),
    };
    value.receipt_sha256 = value.expected_sha256().expect("receipt digest");
    value.receipt_id = group_agent_scheduled_node_terminal_receipt_id(&value.receipt_sha256);
    value
        .validate_against_control(control)
        .expect("valid terminal receipt");
    value
}
