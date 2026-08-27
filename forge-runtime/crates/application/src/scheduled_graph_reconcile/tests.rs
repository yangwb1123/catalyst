use std::sync::{Arc, Mutex};

use super::*;
use crate::runtime_domain::{
    GroupAgentGraphExecutionAttemptPolicy, GroupAgentGraphExecutionFailurePolicy,
    GroupAgentGraphExecutionMode, GroupAgentGraphExecutionProgressionPolicy,
    SCHEDULED_GRAPH_PROGRESS_PROTOCOL_VERSION, SCHEDULED_GRAPH_PROGRESS_SNAPSHOT_VERSION,
    SCHEDULED_GRAPH_RECONCILE_DECISION_VERSION, ScheduledGraphProgressNode,
    ScheduledGraphReconcileDisposition, ScheduledGraphReconcilePortError,
};

const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[test]
fn service_returns_one_bound_core_decision() {
    let snapshot = snapshot("graph-run-fixture");
    let decision = decision(&snapshot);
    let store = Arc::new(FakeStore::ok(snapshot.clone()));
    let core = Arc::new(FakeCore::ok(decision.clone()));
    let service = ScheduledGraphReconcileService::new(store.clone(), core.clone());

    assert_eq!(
        service.reconcile("graph-run-fixture").expect("decision"),
        decision
    );
    assert_eq!(
        store.requests.lock().expect("requests").as_slice(),
        ["graph-run-fixture"]
    );
    assert_eq!(
        core.snapshots.lock().expect("snapshots").as_slice(),
        [snapshot]
    );
}

#[test]
fn service_observes_the_atomic_snapshot_with_its_bound_decision() {
    let snapshot = snapshot("graph-run-fixture");
    let decision = decision(&snapshot);
    let store = Arc::new(FakeStore::ok(snapshot.clone()));
    let core = Arc::new(FakeCore::ok(decision.clone()));
    let service = ScheduledGraphReconcileService::new(store.clone(), core.clone());

    assert_eq!(
        service.observe("graph-run-fixture").expect("observation"),
        ScheduledGraphReconcileObservation {
            snapshot: snapshot.clone(),
            decision,
        }
    );
    assert_eq!(
        store.requests.lock().expect("requests").as_slice(),
        ["graph-run-fixture"]
    );
    assert_eq!(
        core.snapshots.lock().expect("snapshots").as_slice(),
        [snapshot]
    );
}

#[test]
fn invalid_input_opens_neither_store_nor_core() {
    let snapshot = snapshot("graph-run-fixture");
    let store = Arc::new(FakeStore::ok(snapshot.clone()));
    let core = Arc::new(FakeCore::ok(decision(&snapshot)));
    let service = ScheduledGraphReconcileService::new(store.clone(), core.clone());

    assert_eq!(
        service.reconcile("\nsecret"),
        Err(ScheduledGraphReconcileServiceError::InvalidInput)
    );
    assert!(store.requests.lock().expect("requests").is_empty());
    assert!(core.snapshots.lock().expect("snapshots").is_empty());
}

#[test]
fn store_errors_are_structured_and_redacted() {
    let store = Arc::new(FakeStore::err(HubStoreError::Unavailable {
        message: "sensitive sqlite path".into(),
    }));
    let core = Arc::new(FakeCore::err(ScheduledGraphReconcilePortError::Unavailable));
    let service = ScheduledGraphReconcileService::new(store, core);
    let error = service
        .reconcile("graph-run-fixture")
        .expect_err("unavailable");
    assert_eq!(
        error,
        ScheduledGraphReconcileServiceError::StorageUnavailable
    );
    assert!(!error.to_string().contains("sensitive"));
}

#[test]
fn substituted_store_snapshot_is_corruption_before_core() {
    let snapshot = snapshot("graph-run-other");
    let store = Arc::new(FakeStore::ok(snapshot.clone()));
    let core = Arc::new(FakeCore::ok(decision(&snapshot)));
    let service = ScheduledGraphReconcileService::new(store, core.clone());
    assert_eq!(
        service.reconcile("graph-run-fixture"),
        Err(ScheduledGraphReconcileServiceError::CorruptProgress)
    );
    assert!(core.snapshots.lock().expect("snapshots").is_empty());
}

#[test]
fn core_failure_and_substitution_are_separate_redacted_errors() {
    let source = snapshot("graph-run-fixture");
    let unavailable = ScheduledGraphReconcileService::new(
        Arc::new(FakeStore::ok(source.clone())),
        Arc::new(FakeCore::err(ScheduledGraphReconcilePortError::Unavailable)),
    );
    assert_eq!(
        unavailable.reconcile("graph-run-fixture"),
        Err(ScheduledGraphReconcileServiceError::CoreUnavailable)
    );

    let malformed = ScheduledGraphReconcileService::new(
        Arc::new(FakeStore::ok(source.clone())),
        Arc::new(FakeCore::err(
            ScheduledGraphReconcilePortError::InvalidDecision,
        )),
    );
    assert_eq!(
        malformed.reconcile("graph-run-fixture"),
        Err(ScheduledGraphReconcileServiceError::InvalidCoreDecision)
    );

    let wrong_source = snapshot("graph-run-other");
    let invalid = ScheduledGraphReconcileService::new(
        Arc::new(FakeStore::ok(source)),
        Arc::new(FakeCore::ok(decision(&wrong_source))),
    );
    assert_eq!(
        invalid.reconcile("graph-run-fixture"),
        Err(ScheduledGraphReconcileServiceError::InvalidCoreDecision)
    );
}

struct FakeStore {
    result: Result<ScheduledGraphProgressSnapshot, HubStoreError>,
    requests: Mutex<Vec<String>>,
}

impl FakeStore {
    fn ok(snapshot: ScheduledGraphProgressSnapshot) -> Self {
        Self {
            result: Ok(snapshot),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn err(error: HubStoreError) -> Self {
        Self {
            result: Err(error),
            requests: Mutex::new(Vec::new()),
        }
    }
}

impl ScheduledGraphProgressStore for FakeStore {
    fn snapshot_scheduled_graph_progress(
        &self,
        graph_run_id: &str,
    ) -> Result<ScheduledGraphProgressSnapshot, HubStoreError> {
        self.requests
            .lock()
            .expect("requests")
            .push(graph_run_id.into());
        self.result.clone()
    }
}

struct FakeCore {
    result: Result<ScheduledGraphReconcileDecision, ScheduledGraphReconcilePortError>,
    snapshots: Mutex<Vec<ScheduledGraphProgressSnapshot>>,
}

impl FakeCore {
    fn ok(decision: ScheduledGraphReconcileDecision) -> Self {
        Self {
            result: Ok(decision),
            snapshots: Mutex::new(Vec::new()),
        }
    }

    fn err(error: ScheduledGraphReconcilePortError) -> Self {
        Self {
            result: Err(error),
            snapshots: Mutex::new(Vec::new()),
        }
    }
}

impl ScheduledGraphReconcilePort for FakeCore {
    fn decide(
        &self,
        snapshot: &ScheduledGraphProgressSnapshot,
    ) -> Result<ScheduledGraphReconcileDecision, ScheduledGraphReconcilePortError> {
        self.snapshots
            .lock()
            .expect("snapshots")
            .push(snapshot.clone());
        self.result.clone()
    }
}

fn snapshot(graph_run_id: &str) -> ScheduledGraphProgressSnapshot {
    ScheduledGraphProgressSnapshot {
        v: SCHEDULED_GRAPH_PROGRESS_SNAPSHOT_VERSION,
        progress_protocol_version: SCHEDULED_GRAPH_PROGRESS_PROTOCOL_VERSION,
        graph_run_id: graph_run_id.into(),
        graph_id: "graph-fixture".into(),
        schedule_id: format!("graph-execution-schedule-{A}"),
        schedule_sha256: A.into(),
        node_count: 2,
        execution_mode: GroupAgentGraphExecutionMode::Serial,
        max_in_flight_nodes: 1,
        progression_policy: GroupAgentGraphExecutionProgressionPolicy::CompletedContiguousPrefix,
        attempt_policy: GroupAgentGraphExecutionAttemptPolicy::ExactlyOne,
        failure_policy: GroupAgentGraphExecutionFailurePolicy::FailFastNoRetry,
        nodes: vec![node(0, "first"), node(1, "second")],
        snapshot_sha256: String::new(),
    }
    .seal()
    .expect("snapshot")
}

fn node(execution_ordinal: usize, node_id: &str) -> ScheduledGraphProgressNode {
    ScheduledGraphProgressNode {
        execution_ordinal,
        node_id: node_id.into(),
        attempt: 1,
        candidate_id: None,
        candidate_sha256: None,
        provider_request_id: None,
        prepared_request_sha256: None,
        lifecycle_status: None,
        terminal_outcome: None,
        terminal_receipt_sha256: None,
    }
}

fn decision(snapshot: &ScheduledGraphProgressSnapshot) -> ScheduledGraphReconcileDecision {
    ScheduledGraphReconcileDecision {
        v: SCHEDULED_GRAPH_RECONCILE_DECISION_VERSION,
        progress_protocol_version: SCHEDULED_GRAPH_PROGRESS_PROTOCOL_VERSION,
        graph_run_id: snapshot.graph_run_id.clone(),
        schedule_id: snapshot.schedule_id.clone(),
        schedule_sha256: snapshot.schedule_sha256.clone(),
        snapshot_sha256: snapshot.snapshot_sha256.clone(),
        disposition: ScheduledGraphReconcileDisposition::Ready,
        next_execution_ordinal: Some(0),
        next_node_id: Some("first".into()),
        decision_sha256: String::new(),
    }
    .seal()
    .expect("decision")
}
