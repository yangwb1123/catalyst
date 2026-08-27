use super::*;
use crate::ExecuteGroupAgentScheduledNodeDispatchResult;

#[allow(dead_code, clippy::duplicate_mod)]
#[path = "../../../tests/scheduled_legacy_node_dispatch_execution_support/mod.rs"]
mod legacy_execution_support;

#[tokio::test]
async fn legacy_completed_recovery_records_owned_completion_without_resend() {
    let harness = Harness::new();
    let started = harness.start();
    let anchors = awaiting(&started.state);
    let planned = append_crash_dispatch_plan(&harness, &started.journal, &anchors);
    let terminal = persist_legacy_terminal(
        &harness,
        legacy_execution_support::ProviderBehavior::Completed,
    )
    .await;
    assert_exact_source(&terminal.inspection, &planned, &anchors);
    assert_eq!(terminal.outcome(), GroupAgentNodeTerminalOutcome::Completed);
    let observation = harness
        .service
        .observe("graph-run-1")
        .expect("observe completed legacy prefix");
    let super::super::recovery::Recovery::Updated(repaired) = harness
        .service
        .recover_dispatch(&planned, Some(&observation), 1_002)
        .expect("recover legacy completion")
    else {
        panic!("legacy completion must update the controller journal");
    };
    assert_owned_completion(&repaired, &anchors, terminal.receipt_sha256());
    assert_no_resend(&harness, &terminal.counters);

    assert_eq!(
        harness.service.advance(&advance_input(1_003)),
        Err(ScheduledGraphControllerServiceError::MaterializationFailed)
    );
    assert_eq!(
        harness.service.advance(&advance_input(1_004)),
        Err(ScheduledGraphControllerServiceError::MaterializationFailed)
    );
    assert_no_resend(&harness, &terminal.counters);
}

#[tokio::test]
async fn legacy_failed_recovery_stops_without_resend() {
    assert_legacy_stop(
        legacy_execution_support::ProviderBehavior::Length,
        GroupAgentNodeTerminalOutcome::Failed,
        ScheduledGraphControllerStopReason::Failed,
    )
    .await;
}

#[tokio::test]
async fn legacy_failed_uncertain_recovery_stops_without_resend() {
    assert_legacy_stop(
        legacy_execution_support::ProviderBehavior::TransportError,
        GroupAgentNodeTerminalOutcome::FailedUncertain,
        ScheduledGraphControllerStopReason::FailedUncertain,
    )
    .await;
}

async fn assert_legacy_stop(
    behavior: legacy_execution_support::ProviderBehavior,
    expected_outcome: GroupAgentNodeTerminalOutcome,
    reason: ScheduledGraphControllerStopReason,
) {
    let harness = Harness::new();
    let started = harness.start();
    let anchors = awaiting(&started.state);
    let planned = append_crash_dispatch_plan(&harness, &started.journal, &anchors);
    let terminal = persist_legacy_terminal(&harness, behavior).await;
    assert_exact_source(&terminal.inspection, &planned, &anchors);
    assert_eq!(terminal.outcome(), expected_outcome);

    let output = harness
        .service
        .advance(&advance_input(1_002))
        .expect("recover legacy terminal failure");
    assert_eq!(
        output.state,
        ScheduledGraphControllerState::Stopped {
            reason,
            provider_request_id: Some(anchors.provider_request_id.clone()),
        }
    );
    assert_no_resend(&harness, &terminal.counters);
    let replay = harness
        .service
        .advance(&advance_input(1_003))
        .expect("reenter stopped controller");
    assert_eq!(replay.journal, output.journal);
    assert_no_resend(&harness, &terminal.counters);
}

struct LegacyTerminal {
    counters: legacy_execution_support::Counters,
    inspection: GroupAgentScheduledNodeLifecycleInspection,
}

impl LegacyTerminal {
    fn outcome(&self) -> GroupAgentNodeTerminalOutcome {
        self.inspection
            .terminal_receipt
            .as_ref()
            .expect("legacy terminal receipt")
            .node_outcome
    }

    fn receipt_sha256(&self) -> &str {
        &self
            .inspection
            .terminal_receipt
            .as_ref()
            .expect("legacy terminal receipt")
            .receipt_sha256
    }
}

async fn persist_legacy_terminal(
    harness: &Harness,
    behavior: legacy_execution_support::ProviderBehavior,
) -> LegacyTerminal {
    let counters = legacy_execution_support::Counters::default();
    let executor = legacy_execution_support::service_with_provider_behavior(
        &harness.fixture,
        &counters,
        legacy_execution_support::StoreFault::None,
        legacy_execution_support::CoreBehavior::Receipt,
        behavior,
    );
    let result = Box::pin(executor.execute(&legacy_execution_support::input(&harness.fixture)))
        .await
        .expect("legacy executor terminalizes request");
    let ExecuteGroupAgentScheduledNodeDispatchResult::Terminalized(inspection) = result else {
        panic!("legacy fixture must terminalize");
    };
    assert_eq!(counters.snapshot(), (1, 1));
    LegacyTerminal {
        counters,
        inspection,
    }
}

fn assert_exact_source(
    lifecycle: &GroupAgentScheduledNodeLifecycleInspection,
    journal: &ScheduledGraphControllerJournal,
    anchors: &AwaitingAnchors,
) {
    assert_eq!(
        lifecycle.graph_run.run.graph_run_id,
        journal.header.graph_run_id
    );
    assert_eq!(lifecycle.claim.graph_run_id, journal.header.graph_run_id);
    assert_eq!(
        lifecycle.claim.provider_request_id,
        anchors.provider_request_id
    );
    assert_eq!(
        lifecycle.provider_request.provider_request_id,
        anchors.provider_request_id
    );
    assert_eq!(lifecycle.claim.node_id, anchors.node_id);
    assert_eq!(lifecycle.provider_request.node_id, anchors.node_id);
    assert_eq!(
        lifecycle.provider_request.execution_ordinal,
        anchors.execution_ordinal
    );
    assert_eq!(
        lifecycle.provider_request.schedule_sha256,
        journal.header.schedule_sha256
    );
}

fn assert_owned_completion(
    journal: &ScheduledGraphControllerJournal,
    anchors: &AwaitingAnchors,
    receipt_sha256: &str,
) {
    let completions = journal
        .events
        .iter()
        .filter_map(|event| match &event.payload {
            ScheduledGraphControllerEventPayload::NodeCompleted {
                execution_ordinal,
                node_id,
                provider_request_id,
                terminal_receipt_sha256,
            } => Some((
                execution_ordinal,
                node_id,
                provider_request_id,
                terminal_receipt_sha256,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(completions.len(), 1);
    let (ordinal, node_id, provider_request_id, receipt) = completions[0];
    assert_eq!(*ordinal, anchors.execution_ordinal);
    assert_eq!(node_id, &anchors.node_id);
    assert_eq!(provider_request_id, &anchors.provider_request_id);
    assert_eq!(receipt, receipt_sha256);
}

fn assert_no_resend(harness: &Harness, counters: &legacy_execution_support::Counters) {
    assert_eq!(counters.snapshot(), (1, 1));
    assert_eq!(harness.executor_calls.load(Ordering::Acquire), 0);
}
