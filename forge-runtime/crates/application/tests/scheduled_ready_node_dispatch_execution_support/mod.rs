use std::sync::{
    Arc, Barrier,
    atomic::{AtomicUsize, Ordering},
};

use forge_runtime_application::{
    ExecuteGroupAgentScheduledReadyNodeDispatchInput, GroupAgentNodeCredentialSource,
    GroupAgentNodeCredentialSourceError, GroupAgentNodeDispatchClaimMetadata,
    GroupAgentNodeDispatchMetadataSource, GroupAgentNodeDispatchMetadataSourceError,
    GroupAgentScheduledExecutorOwner, GroupAgentScheduledExecutorOwnerError,
    GroupAgentScheduledExecutorOwnerFactory, GroupAgentScheduledReadyNodeDispatchExecutionService,
    ScheduledReadyNodeReleaseService,
};
use forge_runtime_domain::*;
use forge_runtime_infrastructure::SqliteHubStore;

use super::{ready_authorization_support, sqlite_support};

#[path = "provider.rs"]
mod provider_support;
pub use provider_support::ProviderBehavior;
use provider_support::ProviderFactory;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StoreFault {
    #[default]
    None,
    ClaimAfterCommit,
    TerminalBeforeCommit,
    TerminalAfterCommit,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OwnerBehavior {
    #[default]
    CleanupSucceeds,
    CleanupFails,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CoreBehavior {
    #[default]
    Receipt,
    Fail,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EffectCounts {
    pub provider: usize,
    pub credential: usize,
    pub core: usize,
    pub owner_created: usize,
    pub owner_preserved: usize,
    pub owner_cleaned: usize,
}

#[derive(Clone, Default)]
pub struct Counters {
    provider_calls: Arc<AtomicUsize>,
    credential_reads: Arc<AtomicUsize>,
    core_calls: Arc<AtomicUsize>,
    owner_created: Arc<AtomicUsize>,
    owner_preserved: Arc<AtomicUsize>,
    owner_cleaned: Arc<AtomicUsize>,
}

impl Counters {
    pub fn snapshot(&self) -> EffectCounts {
        EffectCounts {
            provider: self.provider_calls.load(Ordering::Acquire),
            credential: self.credential_reads.load(Ordering::Acquire),
            core: self.core_calls.load(Ordering::Acquire),
            owner_created: self.owner_created.load(Ordering::Acquire),
            owner_preserved: self.owner_preserved.load(Ordering::Acquire),
            owner_cleaned: self.owner_cleaned.load(Ordering::Acquire),
        }
    }
}

pub fn preview(
    fixture: &sqlite_support::Fixture,
) -> forge_runtime_application::AuthorizedScheduledReadyNodeRelease {
    ScheduledReadyNodeReleaseService::new(
        fixture.reader.clone(),
        fixture.reader.clone(),
        Arc::new(ReadyReconcile),
        Arc::new(ReadyAuthorize),
    )
    .authorize("graph-run-1")
    .expect("preview ready authorization")
}

pub fn input(
    preview: &forge_runtime_application::AuthorizedScheduledReadyNodeRelease,
    pricing_json: &str,
) -> ExecuteGroupAgentScheduledReadyNodeDispatchInput {
    ExecuteGroupAgentScheduledReadyNodeDispatchInput {
        graph_run_id: preview.authorization.graph_run_id.clone(),
        expected_provider_request_id: preview.authorization.scheduled_provider_request_id.clone(),
        expected_authorization_sha256: preview.authorization.authorization_sha256.clone(),
        pricing_json: pricing_json.into(),
        confirm_off_machine: true,
        confirm_predecessor_content: false,
        cancellation: Cancellation::default(),
    }
}

#[allow(dead_code)]
pub fn different_pricing_json(pricing_json: &str) -> String {
    let mut pricing = GroupAgentNodePricingSnapshot::decode_exact(pricing_json)
        .expect("fixture pricing is canonical");
    pricing.input_usd_micros_per_token_unit += 1;
    pricing.pricing_snapshot_sha256 = pricing.expected_sha256().expect("changed pricing digest");
    pricing.canonical_json().expect("changed pricing JSON")
}

pub fn service(
    fixture: &sqlite_support::Fixture,
    counters: &Counters,
    build_barrier: Option<Arc<Barrier>>,
    store_fault: StoreFault,
    core_behavior: CoreBehavior,
) -> GroupAgentScheduledReadyNodeDispatchExecutionService {
    service_with_provider_behavior(
        fixture,
        counters,
        build_barrier,
        store_fault,
        core_behavior,
        ProviderBehavior::Completed,
    )
}

pub fn service_with_provider_behavior(
    fixture: &sqlite_support::Fixture,
    counters: &Counters,
    build_barrier: Option<Arc<Barrier>>,
    store_fault: StoreFault,
    core_behavior: CoreBehavior,
    provider_behavior: ProviderBehavior,
) -> GroupAgentScheduledReadyNodeDispatchExecutionService {
    service_with_behaviors(
        fixture,
        counters,
        build_barrier,
        store_fault,
        core_behavior,
        provider_behavior,
        OwnerBehavior::CleanupSucceeds,
    )
}

#[allow(dead_code)]
pub fn service_with_owner_behavior(
    fixture: &sqlite_support::Fixture,
    counters: &Counters,
    build_barrier: Option<Arc<Barrier>>,
    store_fault: StoreFault,
    core_behavior: CoreBehavior,
    owner_behavior: OwnerBehavior,
) -> GroupAgentScheduledReadyNodeDispatchExecutionService {
    service_with_behaviors(
        fixture,
        counters,
        build_barrier,
        store_fault,
        core_behavior,
        ProviderBehavior::Completed,
        owner_behavior,
    )
}

#[allow(clippy::too_many_arguments)]
fn service_with_behaviors(
    fixture: &sqlite_support::Fixture,
    counters: &Counters,
    build_barrier: Option<Arc<Barrier>>,
    store_fault: StoreFault,
    core_behavior: CoreBehavior,
    provider_behavior: ProviderBehavior,
    owner_behavior: OwnerBehavior,
) -> GroupAgentScheduledReadyNodeDispatchExecutionService {
    let store = fixture.writer();
    let lifecycles = lifecycle_store(store.clone(), store_fault);
    GroupAgentScheduledReadyNodeDispatchExecutionService::new(
        store.clone(),
        store,
        Arc::new(ReadyReconcile),
        Arc::new(ReadyAuthorize),
        lifecycles,
        Arc::new(ProviderFactory::new(
            counters.provider_calls.clone(),
            build_barrier,
            provider_behavior,
        )),
        Arc::new(Credential {
            reads: counters.credential_reads.clone(),
        }),
        Arc::new(TerminalCore {
            calls: counters.core_calls.clone(),
            behavior: core_behavior,
        }),
        Arc::new(Metadata {
            sequence: AtomicUsize::new(0),
        }),
        Arc::new(OwnerFactory {
            created: counters.owner_created.clone(),
            preserved: counters.owner_preserved.clone(),
            cleaned: counters.owner_cleaned.clone(),
            behavior: owner_behavior,
        }),
    )
}

fn lifecycle_store(
    store: Arc<SqliteHubStore>,
    fault: StoreFault,
) -> Arc<dyn GroupAgentScheduledReadyNodeLifecycleStore> {
    if fault == StoreFault::None {
        store
    } else {
        Arc::new(FaultStore {
            inner: store,
            fault,
        })
    }
}

struct FaultStore {
    inner: Arc<SqliteHubStore>,
    fault: StoreFault,
}

impl GroupAgentScheduledNodeAnyLifecycleStore for FaultStore {
    fn inspect_group_agent_scheduled_node_any_lifecycle(
        &self,
        provider_request_id: &str,
    ) -> Result<GroupAgentScheduledNodeAnyLifecycleInspection, HubStoreError> {
        self.inner
            .inspect_group_agent_scheduled_node_any_lifecycle(provider_request_id)
    }

    fn adjudicate_group_agent_scheduled_node_any_dispatch(
        &self,
        request: &AdjudicateGroupAgentScheduledNodeDispatch,
    ) -> Result<GroupAgentScheduledNodeAnyLifecycleInspection, HubStoreError> {
        self.inner
            .adjudicate_group_agent_scheduled_node_any_dispatch(request)
    }
}

impl GroupAgentScheduledReadyNodeLifecycleStore for FaultStore {
    fn claim_group_agent_scheduled_ready_node_dispatch(
        &self,
        request: &ClaimGroupAgentScheduledReadyNodeDispatch,
    ) -> Result<ClaimGroupAgentScheduledReadyNodeDispatchResult, HubStoreError> {
        let result = self
            .inner
            .claim_group_agent_scheduled_ready_node_dispatch(request)?;
        if self.fault == StoreFault::ClaimAfterCommit
            && matches!(
                &result,
                ClaimGroupAgentScheduledReadyNodeDispatchResult::Claimed { .. }
            )
        {
            return Err(unavailable("claim response lost after commit"));
        }
        Ok(result)
    }

    fn terminalize_group_agent_scheduled_ready_node_dispatch(
        &self,
        request: &TerminalizeGroupAgentScheduledNodeDispatch,
    ) -> Result<TerminalizeGroupAgentScheduledReadyNodeDispatchResult, HubStoreError> {
        if self.fault == StoreFault::TerminalBeforeCommit {
            return Err(unavailable("terminal write failed before commit"));
        }
        let result = self
            .inner
            .terminalize_group_agent_scheduled_ready_node_dispatch(request)?;
        if self.fault == StoreFault::TerminalAfterCommit {
            return Err(unavailable("terminal response lost after commit"));
        }
        Ok(result)
    }

    fn inspect_group_agent_scheduled_ready_node_lifecycle(
        &self,
        provider_request_id: &str,
    ) -> Result<GroupAgentScheduledReadyNodeLifecycleInspection, HubStoreError> {
        self.inner
            .inspect_group_agent_scheduled_ready_node_lifecycle(provider_request_id)
    }
}

fn unavailable(message: &str) -> HubStoreError {
    HubStoreError::Unavailable {
        message: message.into(),
    }
}

struct ReadyReconcile;

impl ScheduledGraphReconcilePort for ReadyReconcile {
    fn decide(
        &self,
        snapshot: &ScheduledGraphProgressSnapshot,
    ) -> Result<ScheduledGraphReconcileDecision, ScheduledGraphReconcilePortError> {
        let selected = snapshot
            .nodes
            .iter()
            .find(|node| node.candidate_id.is_some() && node.lifecycle_status.is_none())
            .ok_or(ScheduledGraphReconcilePortError::InvalidDecision)?;
        ScheduledGraphReconcileDecision {
            v: SCHEDULED_GRAPH_RECONCILE_DECISION_VERSION,
            progress_protocol_version: SCHEDULED_GRAPH_PROGRESS_PROTOCOL_VERSION,
            graph_run_id: snapshot.graph_run_id.clone(),
            schedule_id: snapshot.schedule_id.clone(),
            schedule_sha256: snapshot.schedule_sha256.clone(),
            snapshot_sha256: snapshot.snapshot_sha256.clone(),
            disposition: ScheduledGraphReconcileDisposition::Ready,
            next_execution_ordinal: Some(selected.execution_ordinal),
            next_node_id: Some(selected.node_id.clone()),
            decision_sha256: String::new(),
        }
        .seal()
        .map_err(|_| ScheduledGraphReconcilePortError::InvalidDecision)
    }
}

struct ReadyAuthorize;

impl ScheduledReadyNodeReleasePort for ReadyAuthorize {
    fn authorize(
        &self,
        control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
    ) -> Result<GroupAgentScheduledReadyNodeDispatchAuthorization, ScheduledReadyNodeReleasePortError>
    {
        Ok(ready_authorization_support::authorization(control))
    }
}

struct Credential {
    reads: Arc<AtomicUsize>,
}

impl GroupAgentNodeCredentialSource for Credential {
    fn read_credential(&self) -> Result<String, GroupAgentNodeCredentialSourceError> {
        self.reads.fetch_add(1, Ordering::AcqRel);
        Ok("secret".into())
    }
}

struct Metadata {
    sequence: AtomicUsize,
}

impl GroupAgentNodeDispatchMetadataSource for Metadata {
    fn claim_metadata(
        &self,
    ) -> Result<GroupAgentNodeDispatchClaimMetadata, GroupAgentNodeDispatchMetadataSourceError>
    {
        let value = self.sequence.fetch_add(1, Ordering::AcqRel);
        Ok(GroupAgentNodeDispatchClaimMetadata {
            dispatch_id: format!("unused-dispatch-{value}"),
            lane_ownership_id: format!("ready-owner-{value}"),
            released_at_ms: 100 + value as u64,
        })
    }

    fn terminal_time_ms(&self) -> Result<u64, GroupAgentNodeDispatchMetadataSourceError> {
        Ok(200)
    }
}

struct OwnerFactory {
    created: Arc<AtomicUsize>,
    preserved: Arc<AtomicUsize>,
    cleaned: Arc<AtomicUsize>,
    behavior: OwnerBehavior,
}

impl GroupAgentScheduledExecutorOwnerFactory for OwnerFactory {
    fn create(
        &self,
        _provider_request_id: &str,
        _lane_ownership_id: &str,
    ) -> Result<Box<dyn GroupAgentScheduledExecutorOwner>, GroupAgentScheduledExecutorOwnerError>
    {
        self.created.fetch_add(1, Ordering::AcqRel);
        Ok(Box::new(Owner {
            preserved: self.preserved.clone(),
            cleaned: self.cleaned.clone(),
            behavior: self.behavior,
        }))
    }
}

struct Owner {
    preserved: Arc<AtomicUsize>,
    cleaned: Arc<AtomicUsize>,
    behavior: OwnerBehavior,
}

impl GroupAgentScheduledExecutorOwner for Owner {
    fn preserve_on_drop(&mut self) {
        self.preserved.fetch_add(1, Ordering::AcqRel);
    }

    fn cleanup(self: Box<Self>) -> Result<(), GroupAgentScheduledExecutorOwnerError> {
        if self.behavior == OwnerBehavior::CleanupFails {
            return Err(GroupAgentScheduledExecutorOwnerError);
        }
        self.cleaned.fetch_add(1, Ordering::AcqRel);
        Ok(())
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
                message: "test Core receipt failure".into(),
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
        node_outcome: if artifact.classification == GroupAgentNodeTerminalClassification::Completed
        {
            GroupAgentNodeTerminalOutcome::Completed
        } else {
            GroupAgentNodeTerminalOutcome::FailedUncertain
        },
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
