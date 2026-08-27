use super::*;

#[tokio::test]
async fn terminal_failure_lifecycles_stop_with_their_exact_outcome() {
    assert_terminal_stop(
        ready_execution_support::ProviderBehavior::Length,
        GroupAgentNodeTerminalOutcome::Failed,
        ScheduledGraphControllerStopReason::Failed,
    )
    .await;
    assert_terminal_stop(
        ready_execution_support::ProviderBehavior::TransportError,
        GroupAgentNodeTerminalOutcome::FailedUncertain,
        ScheduledGraphControllerStopReason::FailedUncertain,
    )
    .await;
}

#[tokio::test]
async fn quarantined_lifecycle_stops_without_controller_resend() {
    let harness = Harness::new();
    let started = harness.start();
    let anchors = awaiting(&started.state);
    append_crash_dispatch_plan(&harness, &started.journal, &anchors);
    let counters = ready_execution_support::Counters::default();
    let executor = ready_execution_support::service(
        &harness.fixture,
        &counters,
        None,
        ready_execution_support::StoreFault::None,
        ready_execution_support::CoreBehavior::Fail,
    );

    let result = Box::pin(executor.execute(&ready_input(&harness, &anchors)))
        .await
        .expect("outside executor persists quarantine");
    assert!(matches!(
        result,
        ExecuteGroupAgentScheduledReadyNodeDispatchResult::Quarantined { .. }
    ));
    assert_stop(
        &harness,
        &anchors,
        ScheduledGraphControllerStopReason::Quarantined,
        1_002,
    );
    assert_eq!(counters.snapshot().provider, 1);
}

#[tokio::test]
async fn adjudicated_lifecycle_stops_without_controller_resend() {
    let harness = Harness::new();
    let started = harness.start();
    let anchors = awaiting(&started.state);
    append_crash_dispatch_plan(&harness, &started.journal, &anchors);
    let counters = ready_execution_support::Counters::default();
    let executor = ready_execution_support::service(
        &harness.fixture,
        &counters,
        None,
        ready_execution_support::StoreFault::ClaimAfterCommit,
        ready_execution_support::CoreBehavior::Receipt,
    );

    assert!(matches!(
        Box::pin(executor.execute(&ready_input(&harness, &anchors))).await,
        Err(GroupAgentScheduledReadyNodeDispatchExecutionServiceError::ClaimOutcomeUncertain)
    ));
    let claimed = harness
        .store
        .inspect_group_agent_scheduled_node_any_lifecycle(&anchors.provider_request_id)
        .expect("inspect committed claim");
    assert_eq!(
        claimed.status(),
        GroupAgentScheduledNodeLifecycleStatus::Claimed
    );
    let adjudicated = harness
        .store
        .adjudicate_group_agent_scheduled_node_any_dispatch(
            &AdjudicateGroupAgentScheduledNodeDispatch {
                v: 1,
                provider_request_id: anchors.provider_request_id.clone(),
                expected_lane_ownership_id: claimed.claim().lane_ownership_id.clone(),
                adjudicated_at_ms: 200,
            },
        )
        .expect("adjudicate stranded claim");
    assert_eq!(
        adjudicated.status(),
        GroupAgentScheduledNodeLifecycleStatus::Adjudicated
    );

    assert_stop(
        &harness,
        &anchors,
        ScheduledGraphControllerStopReason::Adjudicated,
        1_002,
    );
    assert_eq!(counters.snapshot().provider, 0);
}

#[tokio::test]
async fn recovery_rejects_a_lifecycle_from_the_wrong_dispatch_source() {
    let harness = Harness::new();
    let started = harness.start();
    let anchors = awaiting(&started.state);
    let planned = append_crash_dispatch_plan(&harness, &started.journal, &anchors);
    persist_terminal(
        &harness,
        &anchors,
        ready_execution_support::ProviderBehavior::TransportError,
    )
    .await;
    let mut wrong_source = planned.clone();
    let ScheduledGraphControllerEventPayload::DispatchPlanned {
        authorization_sha256,
        ..
    } = &mut wrong_source
        .events
        .last_mut()
        .expect("dispatch plan")
        .payload
    else {
        panic!("fixture must end at a dispatch plan");
    };
    *authorization_sha256 = STALE_SHA.into();

    assert!(matches!(
        harness.service.recover_dispatch(&wrong_source, None, 1_002),
        Err(ScheduledGraphControllerServiceError::CorruptEvidence)
    ));
    assert_eq!(harness.inspect(), planned);
    assert_eq!(harness.executor_calls.load(Ordering::Acquire), 0);
}

async fn assert_terminal_stop(
    behavior: ready_execution_support::ProviderBehavior,
    expected_outcome: GroupAgentNodeTerminalOutcome,
    reason: ScheduledGraphControllerStopReason,
) {
    let harness = Harness::new();
    let started = harness.start();
    let anchors = awaiting(&started.state);
    append_crash_dispatch_plan(&harness, &started.journal, &anchors);
    let inspection = persist_terminal(&harness, &anchors, behavior).await;
    assert_eq!(
        inspection
            .terminal_receipt()
            .expect("terminal receipt")
            .node_outcome,
        expected_outcome
    );
    assert_stop(&harness, &anchors, reason, 1_002);
}

async fn persist_terminal(
    harness: &Harness,
    anchors: &AwaitingAnchors,
    behavior: ready_execution_support::ProviderBehavior,
) -> GroupAgentScheduledNodeAnyLifecycleInspection {
    let counters = ready_execution_support::Counters::default();
    let executor = ready_execution_support::service_with_provider_behavior(
        &harness.fixture,
        &counters,
        None,
        ready_execution_support::StoreFault::None,
        ready_execution_support::CoreBehavior::Receipt,
        behavior,
    );
    Box::pin(executor.execute(&ready_input(harness, anchors)))
        .await
        .expect("outside executor terminalizes request");
    assert_eq!(counters.snapshot().provider, 1);
    harness
        .store
        .inspect_group_agent_scheduled_node_any_lifecycle(&anchors.provider_request_id)
        .expect("inspect terminal lifecycle")
}

fn ready_input(
    harness: &Harness,
    anchors: &AwaitingAnchors,
) -> ExecuteGroupAgentScheduledReadyNodeDispatchInput {
    let preview = ready_execution_support::preview(&harness.fixture);
    assert_eq!(
        preview.authorization.authorization_sha256,
        anchors.authorization_sha256
    );
    ready_execution_support::input(&preview, harness.fixture.pricing_json())
}

fn assert_stop(
    harness: &Harness,
    anchors: &AwaitingAnchors,
    reason: ScheduledGraphControllerStopReason,
    observed_at_ms: u64,
) {
    let output = harness
        .service
        .advance(&advance_input(observed_at_ms))
        .expect("recover lifecycle into controller stop");
    assert_eq!(
        output.state,
        ScheduledGraphControllerState::Stopped {
            reason,
            provider_request_id: Some(anchors.provider_request_id.clone()),
        }
    );
    assert_eq!(harness.executor_calls.load(Ordering::Acquire), 0);
    assert_eq!(harness.materializer_calls.load(Ordering::Acquire), 0);
    let replay = harness
        .service
        .advance(&advance_input(observed_at_ms + 1))
        .expect("stopped controller replay");
    assert_eq!(replay.journal, output.journal);
}
