use serde_json::Value;

use super::*;

const GOLDEN_DIGEST: &str = "809d5235e4298ea8a66cb0654b0e662b94a8568e4c184cf1a927bda1c46e8148";

#[test]
fn shared_go_schedule_is_byte_exact_and_fully_bound_to_control() {
    let (json, schedule, control) = fixture();
    let fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-graph-execution-schedule-v1.json"
    )))
    .expect("schedule fixture");

    assert_eq!(schedule.canonical_json().unwrap(), json);
    assert_eq!(
        super::codec::schedule_payload_json(&schedule).unwrap(),
        fixture["canonical_schedule_payload_json"].as_str().unwrap()
    );
    assert_eq!(schedule.expected_sha256().unwrap(), GOLDEN_DIGEST);
    assert_eq!(schedule.schedule_sha256, GOLDEN_DIGEST);
    assert_eq!(
        schedule.schedule_id,
        format!("graph-execution-schedule-{GOLDEN_DIGEST}")
    );
    schedule
        .validate_against_control(&control)
        .expect("exact shared control binding");
    assert_eq!(schedule.initial_frontier, ["frontend", "backend"]);
    assert_eq!(schedule.initial_node, "frontend");
    assert_eq!(
        schedule.nodes[2].direct_predecessor_node_ids,
        ["frontend", "backend"]
    );
}

#[test]
fn strict_decoder_rejects_noncanonical_unknown_null_and_policy_drift() {
    let (json, _, _) = fixture();
    let cases = [
        format!("{json}\n"),
        json.replacen("\"v\":1", "\"v\":1,\"unknown\":true", 1),
        json.replacen("\"nodes\":[", "\"nodes\":null,\"discarded\":[", 1),
        json.replacen(
            "\"partial_output_dataflow\":false",
            "\"partial_output_dataflow\":true",
            1,
        ),
    ];
    for candidate in cases {
        assert!(GroupAgentGraphExecutionSchedule::decode_exact(&candidate).is_err());
    }
}

#[test]
fn standalone_digest_cannot_replace_exact_control_topology() {
    let (_, mut schedule, control) = fixture();
    schedule.nodes[2].direct_predecessor_node_ids.clear();
    resign(&mut schedule);

    schedule.validate().expect("locally valid schedule shape");
    assert!(schedule.validate_against_control(&control).is_err());
}

#[test]
fn admission_and_inspection_bind_exact_bytes_without_run_progress() {
    let (schedule_json, schedule, control) = fixture();
    let control_snapshot_json = control.canonical_json().expect("control JSON");
    let request = AdmitGroupAgentGraphExecutionSchedule {
        v: GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION,
        graph_run_id: schedule.graph_run_id.clone(),
        control_snapshot: control,
        control_snapshot_json,
        schedule: schedule.clone(),
        schedule_json: schedule_json.clone(),
        idempotency_key: "schedule-key".into(),
        admitted_at_ms: 80,
    };
    request.validate().expect("schedule admission");

    let record = record(&schedule, schedule_json.len());
    let inspection = GroupAgentGraphExecutionScheduleInspection {
        v: GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION,
        record,
        schedule_json,
        schedule,
    };
    inspection.validate().expect("schedule inspection");
    assert!(!inspection.record.execution_contract_present);
    assert!(!inspection.record.dispatch_authority_released);
}

fn fixture() -> (
    String,
    GroupAgentGraphExecutionSchedule,
    GroupAgentGraphControlSnapshot,
) {
    let schedule_fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-graph-execution-schedule-v1.json"
    )))
    .expect("schedule fixture");
    let contract_fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-node-execution-contract-v1.json"
    )))
    .expect("control fixture");
    let schedule_json = schedule_fixture["canonical_execution_schedule_json"]
        .as_str()
        .expect("schedule JSON")
        .to_owned();
    let schedule = GroupAgentGraphExecutionSchedule::decode_exact(&schedule_json)
        .expect("shared canonical schedule");
    let control_json = contract_fixture["input"]["canonical_control_snapshot_json"]
        .as_str()
        .expect("control JSON");
    let control: GroupAgentGraphControlSnapshot =
        serde_json::from_str(control_json).expect("control snapshot");
    (schedule_json, schedule, control)
}

fn resign(schedule: &mut GroupAgentGraphExecutionSchedule) {
    let digest = schedule.expected_sha256().expect("schedule digest");
    schedule.schedule_id = format!("graph-execution-schedule-{digest}");
    schedule.schedule_sha256 = digest;
}

fn record(
    schedule: &GroupAgentGraphExecutionSchedule,
    schedule_bytes: usize,
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
        created_at_ms: 80,
    }
}
