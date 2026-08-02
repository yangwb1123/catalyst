use std::sync::{
    Mutex, MutexGuard,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use serde_json::Value;

use crate::runtime_domain::{
    AdmitGroupAgentGraphExecutionSchedule, AdmitGroupAgentGraphExecutionScheduleDisposition,
    AdmitGroupAgentGraphExecutionScheduleResult, BeginGroupAgentGraphRun,
    BeginGroupAgentGraphRunResult, GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION,
    GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION, GroupAgentGraphExecutionSchedule,
    GroupAgentGraphExecutionScheduleInspection, GroupAgentGraphExecutionScheduleRecord,
    GroupAgentGraphExecutionScheduleStore, GroupAgentGraphInspection, GroupAgentGraphRecord,
    GroupAgentGraphRunEvent, GroupAgentGraphRunEventKind, GroupAgentGraphRunInspection,
    GroupAgentGraphRunRecord, GroupAgentGraphRunStatus, GroupAgentGraphRunStore,
    GroupAgentGraphStore, HubEntity, HubStoreError, PrepareGroupAgentGraph,
    PrepareGroupAgentGraphResult,
};

use super::Fixture;

struct StoredSchedule {
    key: String,
    inspection: GroupAgentGraphExecutionScheduleInspection,
}

struct TestState {
    graph: GroupAgentGraphInspection,
    run: GroupAgentGraphRunInspection,
    schedule: Option<StoredSchedule>,
}

pub(super) struct MemoryScheduleHub {
    state: Mutex<TestState>,
    run_reads: AtomicUsize,
    sidecar_admits: AtomicUsize,
    advance_run_after_admit: AtomicBool,
}

impl MemoryScheduleHub {
    pub(super) fn new(fixture: &Fixture) -> Self {
        Self {
            state: Mutex::new(TestState {
                graph: fixture.graph.clone(),
                run: fixture.run.clone(),
                schedule: None,
            }),
            run_reads: AtomicUsize::new(0),
            sidecar_admits: AtomicUsize::new(0),
            advance_run_after_admit: AtomicBool::new(false),
        }
    }

    fn lock(&self) -> MutexGuard<'_, TestState> {
        self.state.lock().expect("test state")
    }

    pub(super) fn run(&self) -> GroupAgentGraphRunInspection {
        self.lock().run.clone()
    }

    pub(super) fn set_inspection(&self, inspection: GroupAgentGraphExecutionScheduleInspection) {
        self.lock().schedule.as_mut().expect("schedule").inspection = inspection;
    }

    pub(super) fn promote_run_to_contract_v2(&self) {
        promote_state_to_contract_v2(&mut self.lock());
    }

    pub(super) fn set_advance_run_after_admit(&self) {
        self.advance_run_after_admit.store(true, Ordering::Relaxed);
    }

    pub(super) fn run_reads(&self) -> usize {
        self.run_reads.load(Ordering::Relaxed)
    }

    pub(super) fn sidecar_admits(&self) -> usize {
        self.sidecar_admits.load(Ordering::Relaxed)
    }
}

fn promote_state_to_contract_v2(state: &mut TestState) {
    let fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-node-execution-contract-v1.json"
    )))
    .expect("contract fixture");
    let event_json = fixture["expected"]["canonical_admission_event_json"]
        .as_str()
        .expect("admission event")
        .to_owned();
    let mut event: GroupAgentGraphRunEvent =
        serde_json::from_str(&event_json).expect("admission event value");
    bind_admission_event(&mut event, state);
    let event_json = event.canonical_json().expect("admission event JSON");
    promote_run(&mut state.run, event, event_json);
}

fn bind_admission_event(event: &mut GroupAgentGraphRunEvent, state: &TestState) {
    let previous = state.run.events[0]
        .expected_sha256()
        .expect("prepared head");
    let control = state
        .schedule
        .as_ref()
        .expect("schedule")
        .inspection
        .schedule
        .control_snapshot_sha256
        .clone();
    let GroupAgentGraphRunEventKind::NodeExecutionContractAdmitted {
        previous_event_sha256,
        control_snapshot_sha256,
        ..
    } = &mut event.kind
    else {
        panic!("contract admission event");
    };
    *previous_event_sha256 = previous;
    *control_snapshot_sha256 = control;
}

fn promote_run(
    run: &mut GroupAgentGraphRunInspection,
    event: GroupAgentGraphRunEvent,
    event_json: String,
) {
    run.v = GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION;
    run.run.v = GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION;
    run.run.status = GroupAgentGraphRunStatus::AwaitingCoreDispatch;
    run.run.execution_contract_present = true;
    run.run.last_event_seq = 2;
    run.run.journal_bytes += event_json.len();
    run.events.push(event);
    run.event_jsons.push(event_json);
    run.validate().expect("valid v2 Graph Run");
}

impl GroupAgentGraphStore for MemoryScheduleHub {
    fn prepare_group_agent_graph(
        &self,
        _request: &PrepareGroupAgentGraph,
    ) -> Result<PrepareGroupAgentGraphResult, HubStoreError> {
        Err(unavailable("Graph preparation is outside this test"))
    }

    fn inspect_group_agent_graph(
        &self,
        graph_id: &str,
    ) -> Result<GroupAgentGraphInspection, HubStoreError> {
        let state = self.lock();
        (state.graph.graph.graph_id == graph_id)
            .then(|| state.graph.clone())
            .ok_or_else(|| not_found(HubEntity::GroupAgentGraph, graph_id))
    }

    fn list_group_agent_graphs(
        &self,
        _group_run_id: Option<&str>,
        _limit: usize,
    ) -> Result<Vec<GroupAgentGraphRecord>, HubStoreError> {
        Ok(vec![self.lock().graph.graph.clone()])
    }
}

impl GroupAgentGraphRunStore for MemoryScheduleHub {
    fn begin_group_agent_graph_run(
        &self,
        _request: &BeginGroupAgentGraphRun,
    ) -> Result<BeginGroupAgentGraphRunResult, HubStoreError> {
        Err(unavailable("Graph Run preparation is outside this test"))
    }

    fn inspect_group_agent_graph_run(
        &self,
        graph_run_id: &str,
    ) -> Result<GroupAgentGraphRunInspection, HubStoreError> {
        self.run_reads.fetch_add(1, Ordering::Relaxed);
        let state = self.lock();
        (state.run.run.graph_run_id == graph_run_id)
            .then(|| state.run.clone())
            .ok_or_else(|| not_found(HubEntity::GroupAgentGraphRun, graph_run_id))
    }

    fn list_group_agent_graph_runs(
        &self,
        _graph_id: Option<&str>,
        _limit: usize,
    ) -> Result<Vec<GroupAgentGraphRunRecord>, HubStoreError> {
        Ok(vec![self.lock().run.run.clone()])
    }
}

impl GroupAgentGraphExecutionScheduleStore for MemoryScheduleHub {
    fn admit_group_agent_graph_execution_schedule(
        &self,
        request: &AdmitGroupAgentGraphExecutionSchedule,
    ) -> Result<AdmitGroupAgentGraphExecutionScheduleResult, HubStoreError> {
        request
            .validate()
            .map_err(|error| conflict(&error.to_string()))?;
        self.sidecar_admits.fetch_add(1, Ordering::Relaxed);
        let mut state = self.lock();
        if let Some(stored) = &state.schedule {
            return replay(stored, request);
        }
        let inspection = schedule_inspection(request);
        state.schedule = Some(StoredSchedule {
            key: request.idempotency_key.clone(),
            inspection: inspection.clone(),
        });
        if self.advance_run_after_admit.load(Ordering::Relaxed) {
            promote_state_to_contract_v2(&mut state);
        }
        Ok(schedule_result(
            AdmitGroupAgentGraphExecutionScheduleDisposition::Created,
            inspection,
        ))
    }

    fn inspect_group_agent_graph_execution_schedule(
        &self,
        schedule_id: &str,
    ) -> Result<GroupAgentGraphExecutionScheduleInspection, HubStoreError> {
        self.lock()
            .schedule
            .as_ref()
            .filter(|stored| stored.inspection.record.schedule_id == schedule_id)
            .map(|stored| stored.inspection.clone())
            .ok_or_else(|| not_found(HubEntity::GroupAgentGraphExecutionSchedule, schedule_id))
    }

    fn list_group_agent_graph_execution_schedules(
        &self,
        graph_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAgentGraphExecutionScheduleRecord>, HubStoreError> {
        Ok(self
            .lock()
            .schedule
            .iter()
            .map(|stored| stored.inspection.record.clone())
            .filter(|record| graph_run_id.is_none_or(|id| id == record.graph_run_id))
            .take(limit)
            .collect())
    }
}

fn replay(
    stored: &StoredSchedule,
    request: &AdmitGroupAgentGraphExecutionSchedule,
) -> Result<AdmitGroupAgentGraphExecutionScheduleResult, HubStoreError> {
    let exact = stored.key == request.idempotency_key
        && stored.inspection.schedule == request.schedule
        && stored.inspection.schedule_json == request.schedule_json;
    if !exact {
        return Err(conflict("schedule replay semantics disagree"));
    }
    Ok(schedule_result(
        AdmitGroupAgentGraphExecutionScheduleDisposition::Replayed,
        stored.inspection.clone(),
    ))
}

fn schedule_inspection(
    request: &AdmitGroupAgentGraphExecutionSchedule,
) -> GroupAgentGraphExecutionScheduleInspection {
    GroupAgentGraphExecutionScheduleInspection {
        v: GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION,
        record: schedule_record(request),
        schedule_json: request.schedule_json.clone(),
        schedule: request.schedule.clone(),
    }
}

fn schedule_record(
    request: &AdmitGroupAgentGraphExecutionSchedule,
) -> GroupAgentGraphExecutionScheduleRecord {
    record_from_schedule(
        &request.schedule,
        request.schedule_json.len(),
        request.admitted_at_ms,
    )
}

fn record_from_schedule(
    schedule: &GroupAgentGraphExecutionSchedule,
    schedule_bytes: usize,
    created_at_ms: u64,
) -> GroupAgentGraphExecutionScheduleRecord {
    GroupAgentGraphExecutionScheduleRecord {
        v: GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION,
        schedule_id: schedule.schedule_id.clone(),
        graph_run_id: schedule.graph_run_id.clone(),
        graph_id: schedule.graph_id.clone(),
        control_snapshot_sha256: schedule.control_snapshot_sha256.clone(),
        schedule_sha256: schedule.schedule_sha256.clone(),
        schedule_bytes,
        node_count: schedule.node_count,
        wave_count: schedule.wave_count,
        expected_last_event_seq: schedule.expected_last_event_seq,
        expected_last_event_sha256: schedule.expected_last_event_sha256.clone(),
        execution_contract_present: schedule.execution_contract_present,
        dispatch_authority_released: schedule.dispatch_authority_released,
        created_at_ms,
    }
}

fn schedule_result(
    disposition: AdmitGroupAgentGraphExecutionScheduleDisposition,
    inspection: GroupAgentGraphExecutionScheduleInspection,
) -> AdmitGroupAgentGraphExecutionScheduleResult {
    AdmitGroupAgentGraphExecutionScheduleResult {
        v: GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION,
        disposition,
        inspection,
    }
}

pub(super) fn resign_inspection(inspection: &mut GroupAgentGraphExecutionScheduleInspection) {
    let digest = inspection.schedule.expected_sha256().expect("digest");
    inspection.schedule.schedule_id = format!("graph-execution-schedule-{digest}");
    inspection.schedule.schedule_sha256 = digest;
    inspection.schedule_json = inspection.schedule.canonical_json().expect("schedule JSON");
    inspection.record = record_from_schedule(
        &inspection.schedule,
        inspection.schedule_json.len(),
        inspection.record.created_at_ms,
    );
}

fn not_found(entity: HubEntity, id: &str) -> HubStoreError {
    HubStoreError::NotFound {
        entity,
        id: id.into(),
    }
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupAgentGraphExecutionSchedule,
        message: message.into(),
    }
}

fn unavailable(message: &str) -> HubStoreError {
    HubStoreError::Unavailable {
        message: message.into(),
    }
}
