use std::sync::Arc;

use rusqlite::{Connection, params};

use super::*;

#[test]
fn valid_core_incompatible_progress_is_a_durable_terminal_stop() {
    let harness = Harness::new();
    let started = harness.start();
    let service = service_with_reconcile(&harness, Arc::new(IncompatibleReconcile));

    let output = service
        .advance(&advance_input(1_001))
        .expect("persist semantic incompatibility");
    assert_eq!(
        output.state,
        ScheduledGraphControllerState::Stopped {
            reason: ScheduledGraphControllerStopReason::IncompatibleProgress,
            provider_request_id: Some(harness.provider_request_id.clone()),
        }
    );
    assert_eq!(
        output.journal.events.len(),
        started.journal.events.len() + 1
    );
    let replay = service
        .advance(&advance_input(1_002))
        .expect("terminal replay");
    assert_eq!(replay.journal, output.journal);
    assert_uncharged_and_uninvoked(&harness, &replay.journal);
}

#[test]
fn core_failures_are_hard_errors_without_journal_append() {
    for error in [
        ScheduledGraphReconcilePortError::Unavailable,
        ScheduledGraphReconcilePortError::InvalidDecision,
    ] {
        let harness = Harness::new();
        let started = harness.start();
        let service = service_with_reconcile(&harness, Arc::new(FailingReconcile(error)));

        assert_eq!(
            service.advance(&advance_input(1_001)),
            Err(ScheduledGraphControllerServiceError::ReconcileFailed)
        );
        assert_eq!(harness.inspect(), started.journal);
        assert_uncharged_and_uninvoked(&harness, &started.journal);
    }
}

#[test]
fn corrupt_schedule_is_a_hard_error_without_journal_append() {
    let harness = Harness::new();
    let started = harness.start();
    corrupt_schedule_blob(&harness);

    assert_eq!(
        harness.service.advance(&advance_input(1_001)),
        Err(ScheduledGraphControllerServiceError::CorruptEvidence)
    );
    assert_eq!(harness.inspect(), started.journal);
    assert_uncharged_and_uninvoked(&harness, &started.journal);
}

fn service_with_reconcile(
    harness: &Harness,
    reconcile: Arc<dyn ScheduledGraphReconcilePort>,
) -> ScheduledGraphControllerService {
    ScheduledGraphControllerService::new(
        harness.store.clone(),
        reconcile,
        Arc::new(ReadyAuthorize),
        Arc::new(ForbiddenMaterializer {
            calls: harness.materializer_calls.clone(),
        }),
        Arc::new(OpenAiCodec),
        Arc::new(CountingExecutor {
            calls: harness.executor_calls.clone(),
        }),
        Arc::new(InputTimeClock),
    )
}

fn corrupt_schedule_blob(harness: &Harness) {
    let connection = Connection::open(harness.fixture.database()).expect("open fixture database");
    connection
        .execute(
            "UPDATE group_agent_graph_execution_schedules \
             SET schedule_blob=?1,schedule_bytes=?2 WHERE graph_run_id='graph-run-1'",
            params![b"{}".as_slice(), 2_i64],
        )
        .expect("corrupt schedule body");
}

struct IncompatibleReconcile;

impl ScheduledGraphReconcilePort for IncompatibleReconcile {
    fn decide(
        &self,
        snapshot: &ScheduledGraphProgressSnapshot,
    ) -> Result<ScheduledGraphReconcileDecision, ScheduledGraphReconcilePortError> {
        decision_with(
            snapshot,
            ScheduledGraphReconcileDisposition::IncompatibleProgress,
        )
    }
}

struct FailingReconcile(ScheduledGraphReconcilePortError);

impl ScheduledGraphReconcilePort for FailingReconcile {
    fn decide(
        &self,
        _snapshot: &ScheduledGraphProgressSnapshot,
    ) -> Result<ScheduledGraphReconcileDecision, ScheduledGraphReconcilePortError> {
        Err(self.0)
    }
}

fn decision_with(
    snapshot: &ScheduledGraphProgressSnapshot,
    disposition: ScheduledGraphReconcileDisposition,
) -> Result<ScheduledGraphReconcileDecision, ScheduledGraphReconcilePortError> {
    ScheduledGraphReconcileDecision {
        v: SCHEDULED_GRAPH_RECONCILE_DECISION_VERSION,
        progress_protocol_version: SCHEDULED_GRAPH_PROGRESS_PROTOCOL_VERSION,
        graph_run_id: snapshot.graph_run_id.clone(),
        schedule_id: snapshot.schedule_id.clone(),
        schedule_sha256: snapshot.schedule_sha256.clone(),
        snapshot_sha256: snapshot.snapshot_sha256.clone(),
        disposition,
        next_execution_ordinal: None,
        next_node_id: None,
        decision_sha256: String::new(),
    }
    .seal()
    .map_err(|_| ScheduledGraphReconcilePortError::InvalidDecision)
}
