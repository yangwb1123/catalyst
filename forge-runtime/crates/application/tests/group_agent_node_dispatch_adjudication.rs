#![allow(dead_code)]

mod group_agent_node_dispatch_execution_support;
mod group_agent_node_execution_support;

use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use forge_runtime_application::{
    AdjudicateGroupAgentNodeDispatchInput, AdjudicateGroupAgentNodeDispatchResult,
    AdjudicationRefused, GroupAgentNodeDispatchAdjudicationService,
    GroupAgentNodeDispatchAdjudicationServiceError, GroupAgentNodeDispatchExecutionServiceError,
};
use forge_runtime_domain::{
    ClaimGroupAgentNodeDispatch, ClaimGroupAgentNodeDispatchResult,
    GROUP_AGENT_GRAPH_RUN_TERMINAL_VERSION, GroupAgentGraphRunEventKind, GroupAgentGraphRunStatus,
    GroupAgentNodeDispatchAuthorization, GroupAgentNodeLifecycleInspection,
    GroupAgentNodeLifecycleStore, GroupAgentNodeTerminalArtifactKind,
    GroupAgentNodeTerminalClassification, GroupAgentNodeTerminalOutcome, HubStoreError,
    TerminalizeGroupAgentNodeDispatch, TerminalizeGroupAgentNodeDispatchResult,
    group_agent_node_dispatch_authorization_id,
};

use group_agent_node_dispatch_execution_support::{
    DeterministicCore, DeterministicMetadata, ExactJsonCodec, ExecutionHarness, prepare,
};
use group_agent_node_execution_support::MemoryContractHub;

struct AdjudicationHarness {
    service: GroupAgentNodeDispatchAdjudicationService,
    hub: Arc<MemoryContractHub>,
    input: AdjudicateGroupAgentNodeDispatchInput,
    core_calls: Arc<AtomicUsize>,
}

/// Strands a durable v4 claim with a rejecting Core, then builds the
/// no-send adjudication service over the same in-memory Hub.
async fn stranded() -> AdjudicationHarness {
    let harness = ExecutionHarness::new(vec![], true);
    let error = harness
        .service
        .execute(&harness.input)
        .await
        .expect_err("Core rejection strands the durable claim");
    assert!(matches!(
        error,
        GroupAgentNodeDispatchExecutionServiceError::DispatchQuarantined
    ));
    let core_calls = Arc::new(AtomicUsize::new(0));
    let service = GroupAgentNodeDispatchAdjudicationService::new(
        harness.hub.clone(),
        harness.hub.clone(),
        harness.hub.clone(),
        harness.codec.clone(),
        Arc::new(DeterministicCore {
            reject: false,
            calls: core_calls.clone(),
        }),
        Arc::new(DeterministicMetadata),
    );
    AdjudicationHarness {
        service,
        hub: harness.hub.clone(),
        input: AdjudicateGroupAgentNodeDispatchInput {
            graph_run_id: harness.input.graph_run_id.clone(),
            authorization_json: harness.input.authorization_json.clone(),
            pricing_json: harness.input.pricing_json.clone(),
        },
        core_calls,
    }
}

fn assert_terminal_state(inspection: &GroupAgentNodeLifecycleInspection) {
    assert_run_terminal(&inspection.graph_run.run);
    assert_hard_crash_evidence(inspection);
}

fn assert_run_terminal(run: &forge_runtime_domain::GroupAgentGraphRunRecord) {
    assert_eq!(run.status, GroupAgentGraphRunStatus::FailedUncertain);
    assert_eq!(run.v, GROUP_AGENT_GRAPH_RUN_TERMINAL_VERSION);
    assert_eq!(run.last_event_seq, 5);
}

fn assert_hard_crash_evidence(inspection: &GroupAgentNodeLifecycleInspection) {
    assert_eq!(inspection.graph_run.events.len(), 5);
    assert!(inspection.active_lane.is_none());

    let artifact = inspection.artifact.as_ref().expect("hard-crash artifact");
    assert_eq!(
        artifact.artifact_kind,
        GroupAgentNodeTerminalArtifactKind::Uncertainty
    );
    assert_eq!(
        artifact.classification,
        GroupAgentNodeTerminalClassification::HardCrash
    );
    assert!(!artifact.provider_poll_started);
    assert!(!artifact.terminal_seen);
    assert!(!artifact.stream_eof_seen);
    assert!(!artifact.usage_observed);
    assert!(!artifact.actual_cost_calculated);
    assert!(artifact.output_text.is_empty());
    assert!(!artifact.retry_authorized);
    assert!(artifact.created_at_ms >= inspection.claim.released_at_ms);

    let receipt = inspection.terminal_receipt.as_ref().expect("Core receipt");
    assert_eq!(
        receipt.node_outcome,
        GroupAgentNodeTerminalOutcome::FailedUncertain
    );
    assert_eq!(
        receipt.graph_status,
        GroupAgentGraphRunStatus::FailedUncertain
    );
    assert_eq!(receipt.expected_last_event_seq, 4);
    assert!(receipt.lane_release_authorized);
    assert!(!receipt.retry_authorized);

    let GroupAgentGraphRunEventKind::NodeLifecycleTerminalized {
        retry_authorized,
        lane_released,
        artifact_id,
        terminal_receipt_id,
        ..
    } = &inspection.graph_run.events[4].kind
    else {
        panic!("seq-5 event must be a terminalized lifecycle");
    };
    assert!(!retry_authorized);
    assert!(lane_released);
    assert_eq!(artifact_id, &artifact.artifact_id);
    assert_eq!(terminal_receipt_id, &receipt.receipt_id);
}

#[tokio::test]
async fn adjudication_writes_a_deterministic_failed_uncertain_terminal_without_sending() {
    let harness = stranded().await;
    let result = harness
        .service
        .adjudicate(&harness.input)
        .expect("adjudicate stranded claim");
    let AdjudicateGroupAgentNodeDispatchResult::Adjudicated(inspection) = result;
    assert_terminal_state(&inspection);
    assert_eq!(harness.core_calls.load(Ordering::Acquire), 1);

    let replay = harness
        .service
        .adjudicate(&harness.input)
        .expect_err("re-adjudication of a terminal claim must be refused");
    assert!(matches!(
        replay,
        GroupAgentNodeDispatchAdjudicationServiceError::Refused(
            AdjudicationRefused::NotStranded { .. }
        )
    ));
    assert_eq!(harness.core_calls.load(Ordering::Acquire), 1);
}

#[tokio::test]
async fn a_concurrent_terminalizer_maps_to_cas_conflict_and_the_winner_commits() {
    let harness = stranded().await;
    let racing = Arc::new(RacingTerminalizeStore {
        inner: harness.hub.clone(),
        raced: AtomicBool::new(false),
    });
    let service = GroupAgentNodeDispatchAdjudicationService::new(
        harness.hub.clone(),
        harness.hub.clone(),
        racing.clone(),
        Arc::new(ExactJsonCodec),
        Arc::new(DeterministicCore {
            reject: false,
            calls: Arc::new(AtomicUsize::new(0)),
        }),
        Arc::new(DeterministicMetadata),
    );
    let error = service
        .adjudicate(&harness.input)
        .expect_err("the live executor wins the CAS");
    assert!(matches!(
        error,
        GroupAgentNodeDispatchAdjudicationServiceError::Refused(AdjudicationRefused::CasConflict)
    ));

    // The winner's committed terminal state is intact and authoritative.
    let inspection = harness
        .hub
        .inspect_group_agent_node_lifecycle(&harness.input.graph_run_id)
        .expect("winner terminal inspection");
    assert_eq!(
        inspection.graph_run.run.status,
        GroupAgentGraphRunStatus::FailedUncertain
    );
    assert!(inspection.active_lane.is_none());
    assert!(racing.raced.load(Ordering::Acquire));
}
#[tokio::test]
async fn a_core_without_hard_crash_support_is_refused_with_a_repin_hint_and_zero_mutation() {
    let harness = stranded().await;
    let service = GroupAgentNodeDispatchAdjudicationService::new(
        harness.hub.clone(),
        harness.hub.clone(),
        harness.hub.clone(),
        Arc::new(ExactJsonCodec),
        Arc::new(DeterministicCore {
            reject: true,
            calls: Arc::new(AtomicUsize::new(0)),
        }),
        Arc::new(DeterministicMetadata),
    );
    let error = service
        .adjudicate(&harness.input)
        .expect_err("old Core refuses the hard-crash control");
    assert!(matches!(
        error,
        GroupAgentNodeDispatchAdjudicationServiceError::Refused(
            AdjudicationRefused::CoreRefused { .. }
        )
    ));
    let message = error.to_string();
    assert!(message.starts_with("adjudication refused:"));
    assert!(message.contains("re-pin to a Core with hard_crash support"));
    assert!(!message.contains("quarantined; resend is forbidden"));

    let inspection = harness
        .hub
        .inspect_group_agent_node_lifecycle(&harness.input.graph_run_id)
        .expect("stranded claim still present");
    assert_eq!(
        inspection.graph_run.run.status,
        GroupAgentGraphRunStatus::DispatchUnknown
    );
    assert!(inspection.active_lane.is_some());
    assert!(inspection.artifact.is_none());
}

#[tokio::test]
async fn wrong_operator_bodies_are_refused_as_digest_mismatch_before_any_core_call() {
    let harness = stranded().await;
    let mut input = harness.input.clone();
    input.authorization_json = different_valid_authorization(&harness.input.authorization_json);
    let error = harness
        .service
        .adjudicate(&input)
        .expect_err("digest mismatch must refuse");
    assert!(matches!(
        error,
        GroupAgentNodeDispatchAdjudicationServiceError::Refused(
            AdjudicationRefused::DigestMismatch {
                field: "authorization"
            }
        )
    ));
    assert_eq!(harness.core_calls.load(Ordering::Acquire), 0);
    let inspection = harness
        .hub
        .inspect_group_agent_node_lifecycle(&harness.input.graph_run_id)
        .expect("stranded claim still present");
    assert_eq!(
        inspection.graph_run.run.status,
        GroupAgentGraphRunStatus::DispatchUnknown
    );
}

#[tokio::test]
async fn adjudication_of_a_run_without_a_claim_is_refused_as_not_stranded() {
    let prepared = prepare();
    let service = GroupAgentNodeDispatchAdjudicationService::new(
        prepared.hub.clone(),
        prepared.hub.clone(),
        prepared.hub.clone(),
        prepared.codec,
        Arc::new(DeterministicCore {
            reject: false,
            calls: Arc::new(AtomicUsize::new(0)),
        }),
        Arc::new(DeterministicMetadata),
    );
    let error = service
        .adjudicate(&AdjudicateGroupAgentNodeDispatchInput {
            graph_run_id: prepared.fixture.run.run.graph_run_id,
            authorization_json: prepared.authorization_json,
            pricing_json: prepared.pricing_json,
        })
        .expect_err("no claim means no stranded lifecycle");
    assert!(matches!(
        error,
        GroupAgentNodeDispatchAdjudicationServiceError::Refused(
            AdjudicationRefused::NotStranded { .. }
        )
    ));
    assert!(error.to_string().contains("no stranded dispatch claim"));
}

/// Re-signs the fixture authorization with a different budget so the canonical
/// payload digest differs while every self-validation still holds.
fn different_valid_authorization(canonical: &str) -> String {
    let mut authorization: GroupAgentNodeDispatchAuthorization =
        serde_json::from_str(canonical).expect("fixture authorization JSON");
    authorization.budgets.max_cost_usd_micros += 1;
    authorization.authorization_sha256 = authorization
        .expected_sha256()
        .expect("recomputed authorization digest");
    authorization.authorization_id =
        group_agent_node_dispatch_authorization_id(&authorization.authorization_sha256);
    let json = authorization.canonical_json().expect("canonical JSON");
    authorization.validate().expect("still self-valid");
    assert_ne!(json, canonical);
    json
}

/// A lifecycle store that lets the "live executor" win the terminalize CAS
/// first, so the adjudication's own terminalize hits the loser Conflict.
struct RacingTerminalizeStore {
    inner: Arc<dyn GroupAgentNodeLifecycleStore>,
    raced: AtomicBool,
}

impl GroupAgentNodeLifecycleStore for RacingTerminalizeStore {
    fn claim_group_agent_node_dispatch(
        &self,
        request: &ClaimGroupAgentNodeDispatch,
    ) -> Result<ClaimGroupAgentNodeDispatchResult, HubStoreError> {
        self.inner.claim_group_agent_node_dispatch(request)
    }

    fn terminalize_group_agent_node_dispatch(
        &self,
        request: &TerminalizeGroupAgentNodeDispatch,
    ) -> Result<TerminalizeGroupAgentNodeDispatchResult, HubStoreError> {
        if !self.raced.swap(true, Ordering::AcqRel) {
            self.inner.terminalize_group_agent_node_dispatch(request)?;
        }
        self.inner.terminalize_group_agent_node_dispatch(request)
    }

    fn inspect_group_agent_node_lifecycle(
        &self,
        graph_run_id: &str,
    ) -> Result<GroupAgentNodeLifecycleInspection, HubStoreError> {
        self.inner.inspect_group_agent_node_lifecycle(graph_run_id)
    }
}
