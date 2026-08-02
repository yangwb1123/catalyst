use std::{collections::BTreeMap, sync::Arc};

use serde_json::Value;

use crate::runtime_domain::{
    AdmitGroupAgentGraphExecutionScheduleDisposition, GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION,
    GROUP_AGENT_GRAPH_RUN_VERSION, GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
    GROUP_AGENT_GRAPH_VERSION, GroupAgentGraphControlSnapshot, GroupAgentGraphExecutionSchedule,
    GroupAgentGraphInspection, GroupAgentGraphRecord, GroupAgentGraphRunEvent,
    GroupAgentGraphRunEventKind, GroupAgentGraphRunInspection, GroupAgentGraphRunRecord,
    GroupAgentGraphRunStatus, GroupAgentGraphStatus,
};

use super::*;

#[path = "schedule_test_store.rs"]
mod store;
use store::{MemoryScheduleHub, resign_inspection};

#[test]
fn admission_writes_only_the_sidecar_and_preserves_exact_run_journal() {
    let fixture = fixture();
    let (service, hub) = harness(&fixture);
    let before = hub.run();
    let result = service.admit(&input(&fixture, 80)).expect("admit schedule");

    assert_eq!(
        result.disposition,
        AdmitGroupAgentGraphExecutionScheduleDisposition::Created
    );
    assert_eq!(hub.sidecar_admits(), 1);
    assert_eq!(hub.run(), before);
    assert_eq!(result.inspection.record.created_at_ms, 80);
    assert!(!result.inspection.schedule.progress_observed);
    assert!(!result.inspection.schedule.successor_advanced);
}

#[test]
fn exact_replay_preserves_original_schedule_identity_bytes_and_time() {
    let fixture = fixture();
    let (service, _) = harness(&fixture);
    let created = service.admit(&input(&fixture, 80)).expect("create");
    let replayed = service.admit(&input(&fixture, 999)).expect("replay");

    assert_eq!(
        replayed.disposition,
        AdmitGroupAgentGraphExecutionScheduleDisposition::Replayed
    );
    assert_eq!(replayed.inspection, created.inspection);
    assert_eq!(replayed.inspection.record.created_at_ms, 80);
}

#[test]
fn malformed_schedule_is_rejected_before_any_source_or_sidecar_access() {
    let fixture = fixture();
    let (service, hub) = harness(&fixture);
    let mut request = input(&fixture, 80);
    request.schedule_json.push('\n');

    assert!(matches!(
        service.admit(&request),
        Err(GroupAgentGraphExecutionScheduleServiceError::InvalidInput { .. })
    ));
    assert_eq!(hub.run_reads(), 0);
    assert_eq!(hub.sidecar_admits(), 0);
}

#[test]
fn admission_does_not_misclassify_a_valid_post_commit_source_advance() {
    let fixture = fixture();
    let (service, hub) = harness(&fixture);
    hub.set_advance_run_after_admit();

    let admitted = service
        .admit(&input(&fixture, 80))
        .expect("post-commit source advancement is not schedule corruption");

    assert_eq!(
        admitted.disposition,
        AdmitGroupAgentGraphExecutionScheduleDisposition::Created
    );
    assert_eq!(hub.run().run.v, GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION);
    assert_eq!(hub.run_reads(), 1);
}

#[test]
fn inspection_rejects_self_consistent_schedule_drift_from_exact_control() {
    let fixture = fixture();
    let (service, hub) = harness(&fixture);
    let mut inspection = service
        .admit(&input(&fixture, 80))
        .expect("admit")
        .inspection;
    inspection.schedule.nodes[2]
        .direct_predecessor_node_ids
        .clear();
    resign_inspection(&mut inspection);
    inspection.validate().expect("locally valid sidecar");
    let drifted_id = inspection.record.schedule_id.clone();
    hub.set_inspection(inspection);

    assert!(matches!(
        service.inspect(&drifted_id),
        Err(GroupAgentGraphExecutionScheduleServiceError::Corrupt { .. })
    ));
}

#[test]
fn inspection_remains_readable_after_legacy_contract_v1_admission() {
    let fixture = fixture();
    let (service, hub) = harness(&fixture);
    let created = service.admit(&input(&fixture, 80)).expect("admit schedule");
    hub.promote_run_to_contract_v2();

    let shown = service
        .inspect(&created.inspection.record.schedule_id)
        .expect("inspect passive schedule from v2");
    assert_eq!(shown, created.inspection);
}

struct Fixture {
    schedule_json: String,
    schedule: GroupAgentGraphExecutionSchedule,
    graph: GroupAgentGraphInspection,
    run: GroupAgentGraphRunInspection,
}

fn fixture() -> Fixture {
    let schedule_fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-graph-execution-schedule-v1.json"
    )))
    .expect("schedule fixture");
    let control_fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-node-execution-contract-v1.json"
    )))
    .expect("control fixture");
    let schedule_json = schedule_fixture["canonical_execution_schedule_json"]
        .as_str()
        .expect("schedule JSON")
        .to_owned();
    let schedule =
        GroupAgentGraphExecutionSchedule::decode_exact(&schedule_json).expect("canonical schedule");
    let control_json = control_fixture["input"]["canonical_control_snapshot_json"]
        .as_str()
        .expect("control JSON");
    let control: GroupAgentGraphControlSnapshot =
        serde_json::from_str(control_json).expect("control");
    fixture_from_control(schedule_json, schedule, control)
}

fn fixture_from_control(
    _schedule_json: String,
    mut schedule: GroupAgentGraphExecutionSchedule,
    mut control: GroupAgentGraphControlSnapshot,
) -> Fixture {
    let event = prepared_event(&control);
    control.last_event_sha256 = event.expected_sha256().expect("event digest");
    control.snapshot_sha256 = control.expected_sha256().expect("control digest");
    schedule.expected_last_event_sha256 = control.last_event_sha256.clone();
    schedule.control_snapshot_sha256 = control.snapshot_sha256.clone();
    let digest = schedule.expected_sha256().expect("schedule digest");
    schedule.schedule_id = format!("graph-execution-schedule-{digest}");
    schedule.schedule_sha256 = digest;
    let schedule_json = schedule.canonical_json().expect("schedule JSON");
    let event_json = event.canonical_json().expect("event JSON");
    let plan_json = control.plan.canonical_json().expect("plan JSON");
    let manifest_json = sorted_json(&control.manifest);
    Fixture {
        schedule_json,
        schedule,
        graph: graph_inspection(&control, manifest_json),
        run: run_inspection(&control, plan_json, event, event_json),
    }
}

fn prepared_event(control: &GroupAgentGraphControlSnapshot) -> GroupAgentGraphRunEvent {
    GroupAgentGraphRunEvent {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        graph_run_id: control.graph_run_id.clone(),
        seq: 1,
        kind: GroupAgentGraphRunEventKind::GraphRunPrepared {
            graph_id: control.graph_id.clone(),
            graph_manifest_sha256: control.graph_manifest_sha256.clone(),
            plan_sha256: control.core_plan_sha256.clone(),
            scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
            prepared_at_ms: 73,
        },
    }
}

fn graph_inspection(
    control: &GroupAgentGraphControlSnapshot,
    manifest_json: String,
) -> GroupAgentGraphInspection {
    GroupAgentGraphInspection {
        v: GROUP_AGENT_GRAPH_VERSION,
        graph: GroupAgentGraphRecord {
            v: GROUP_AGENT_GRAPH_VERSION,
            graph_id: control.graph_id.clone(),
            group_run_id: control.manifest.source.group_run_id.clone(),
            status: GroupAgentGraphStatus::Prepared,
            source_snapshot_sha256: control.source_snapshot_sha256.clone(),
            manifest_sha256: control.graph_manifest_sha256.clone(),
            manifest_bytes: manifest_json.len(),
            node_count: control.manifest.nodes.len(),
            edge_count: control.manifest.edges.len(),
            wave_count: control.manifest.waves.len(),
            created_at_ms: 72,
        },
        manifest: control.manifest.clone(),
        manifest_json,
    }
}

fn run_inspection(
    control: &GroupAgentGraphControlSnapshot,
    plan_json: String,
    event: GroupAgentGraphRunEvent,
    event_json: String,
) -> GroupAgentGraphRunInspection {
    GroupAgentGraphRunInspection {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        run: run_record(control, plan_json.len(), event_json.len()),
        plan_json,
        plan: control.plan.clone(),
        event_jsons: vec![event_json],
        events: vec![event],
    }
}

fn run_record(
    control: &GroupAgentGraphControlSnapshot,
    plan_bytes: usize,
    journal_bytes: usize,
) -> GroupAgentGraphRunRecord {
    GroupAgentGraphRunRecord {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        graph_run_id: control.graph_run_id.clone(),
        graph_id: control.graph_id.clone(),
        status: GroupAgentGraphRunStatus::AwaitingExecutionContract,
        source_snapshot_sha256: control.source_snapshot_sha256.clone(),
        graph_manifest_sha256: control.graph_manifest_sha256.clone(),
        scheduler_protocol_version: control.scheduler_protocol_version,
        plan_sha256: control.core_plan_sha256.clone(),
        plan_bytes,
        node_count: control.plan.authored_node_ids.len(),
        wave_count: control.plan.waves.len(),
        execution_contract_present: false,
        dispatch_request_present: false,
        dispatch_authority_released: false,
        last_event_seq: 1,
        journal_bytes,
        created_at_ms: 73,
    }
}

fn sorted_json(value: &impl serde::Serialize) -> String {
    let value = serde_json::to_value(value).expect("serialize");
    serde_json::to_string(&sort(value)).expect("sorted JSON")
}

fn sort(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(sort).collect()),
        Value::Object(items) => Value::Object(
            items
                .into_iter()
                .map(|(key, value)| (key, sort(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        other => other,
    }
}

fn input(fixture: &Fixture, admitted_at_ms: u64) -> AdmitGroupAgentGraphExecutionScheduleInput {
    AdmitGroupAgentGraphExecutionScheduleInput {
        graph_run_id: fixture.schedule.graph_run_id.clone(),
        schedule_json: fixture.schedule_json.clone(),
        idempotency_key: "schedule-key".into(),
        admitted_at_ms,
    }
}

fn harness(
    fixture: &Fixture,
) -> (
    GroupAgentGraphExecutionScheduleService,
    Arc<MemoryScheduleHub>,
) {
    let hub = Arc::new(MemoryScheduleHub::new(fixture));
    let service =
        GroupAgentGraphExecutionScheduleService::new(hub.clone(), hub.clone(), hub.clone());
    (service, hub)
}
