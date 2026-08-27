use std::sync::{Arc, Barrier};

use forge_runtime_application::{
    ScheduledReadyNodeReleaseService, ScheduledReadyNodeReleaseServiceError,
};
pub use forge_runtime_domain as runtime_domain;
pub use forge_runtime_domain::*;

#[path = "../../infrastructure/src/sqlite_hub/scheduled_graph_progress/atomicity_authorization.rs"]
mod legacy_authorization_support;
#[path = "../../domain/src/group_agent_node_execution/scheduled_ready_dispatch_release_test_authorization.rs"]
mod ready_authorization_support;
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "../../infrastructure/tests/sqlite_group_agent_graph_execution_schedule_support/mod.rs"]
mod sqlite_group_agent_graph_execution_schedule_support;
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "../../infrastructure/tests/sqlite_group_agent_graph_run_support/mod.rs"]
mod sqlite_group_agent_graph_run_support;
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "../../infrastructure/tests/sqlite_group_agent_scheduled_node_contract_support/mod.rs"]
mod sqlite_group_agent_scheduled_node_contract_support;
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "../../infrastructure/tests/sqlite_group_agent_scheduled_node_provider_request_support/mod.rs"]
mod sqlite_group_agent_scheduled_node_provider_request_support;
#[path = "scheduled_ready_release_sqlite_support/mod.rs"]
mod sqlite_support;

#[test]
fn real_sqlite_claim_between_source_a_and_b_is_rejected() {
    let fixture = sqlite_support::Fixture::new();
    let entered = Arc::new(Barrier::new(2));
    let resume = Arc::new(Barrier::new(2));
    let core = Arc::new(PausedCore {
        entered: entered.clone(),
        resume: resume.clone(),
    });
    let service = ScheduledReadyNodeReleaseService::new(
        fixture.reader.clone(),
        fixture.reader.clone(),
        Arc::new(ReadyReconcile),
        core,
    );
    let worker = std::thread::spawn(move || service.authorize("graph-run-1"));

    entered.wait();
    let claimed = fixture.claim();
    resume.wait();

    assert!(matches!(
        claimed.expect("legal claim while Core is paused"),
        ClaimGroupAgentScheduledNodeDispatchResult::Claimed { .. }
    ));
    assert_eq!(
        worker.join().expect("authorization worker"),
        Err(ScheduledReadyNodeReleaseServiceError::SourceChanged)
    );
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

struct PausedCore {
    entered: Arc<Barrier>,
    resume: Arc<Barrier>,
}

impl ScheduledReadyNodeReleasePort for PausedCore {
    fn authorize(
        &self,
        control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
    ) -> Result<GroupAgentScheduledReadyNodeDispatchAuthorization, ScheduledReadyNodeReleasePortError>
    {
        control
            .validate()
            .map_err(|_| ScheduledReadyNodeReleasePortError::InvalidAuthorization)?;
        self.entered.wait();
        self.resume.wait();
        Ok(ready_authorization_support::authorization(control))
    }
}
