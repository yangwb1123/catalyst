use serde_json::Value;

use crate::runtime_domain::{
    AdmitGroupAgentScheduledNodeContractDisposition, AdmitGroupAgentScheduledNodeContractResult,
    GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION, GroupAgentGraphControlSnapshot,
    GroupAgentGraphExecutionSchedule, GroupAgentGraphExecutionScheduleInspection,
    GroupAgentGraphExecutionScheduleRecord, GroupAgentScheduledNodeContractCandidate,
    GroupAgentScheduledNodeContractInspection, GroupAgentScheduledNodeContractRecord,
};

use super::{
    AdmitGroupAgentScheduledNodeContractInput, ExportGroupAgentGraphControl,
    GroupAgentScheduledNodeContractServiceError,
    scheduled_contract_validation::{
        admission_request, parse_admit_input, validate_admit_result, validate_list,
    },
};

#[test]
fn canonical_input_builds_an_exact_passive_admission() {
    let fixture = fixture();
    let candidate = parse_admit_input(&fixture.input).expect("parse candidate");
    let request = admission_request(
        &fixture.input,
        candidate,
        ExportGroupAgentGraphControl {
            snapshot: fixture.control.clone(),
            snapshot_json: fixture.control.canonical_json().expect("control JSON"),
        },
        &fixture.schedule,
    )
    .expect("admission request");

    request.validate().expect("exact passive admission");
    assert!(!request.candidate.lifecycle_contract_admitted);
    assert!(!request.candidate.execution_authority_released);
    assert!(
        request
            .candidate
            .request
            .predecessor_terminal_receipts
            .is_empty()
    );
}

#[test]
fn noncanonical_input_is_invalid_before_admission_construction() {
    let mut fixture = fixture();
    fixture.input.contract_json.push('\n');
    assert!(matches!(
        parse_admit_input(&fixture.input),
        Err(GroupAgentScheduledNodeContractServiceError::InvalidInput { .. })
    ));
}

#[test]
fn result_and_list_validation_reject_drift_and_duplicate_runs() {
    let fixture = fixture();
    let record = record(&fixture.candidate, 80);
    let inspection = GroupAgentScheduledNodeContractInspection {
        v: GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION,
        record: record.clone(),
        candidate_json: fixture.input.contract_json.clone(),
        candidate: fixture.candidate.clone(),
    };
    let request = admission_request(
        &fixture.input,
        fixture.candidate,
        ExportGroupAgentGraphControl {
            snapshot: fixture.control.clone(),
            snapshot_json: fixture.control.canonical_json().unwrap(),
        },
        &fixture.schedule,
    )
    .unwrap();
    let result = AdmitGroupAgentScheduledNodeContractResult {
        v: GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION,
        disposition: AdmitGroupAgentScheduledNodeContractDisposition::Created,
        inspection,
    };
    validate_admit_result(&request, result).expect("valid store result");
    assert!(validate_list(&[record.clone(), record], None, 10).is_err());
}

struct Fixture {
    input: AdmitGroupAgentScheduledNodeContractInput,
    candidate: GroupAgentScheduledNodeContractCandidate,
    control: GroupAgentGraphControlSnapshot,
    schedule: GroupAgentGraphExecutionScheduleInspection,
}

fn fixture() -> Fixture {
    let candidate_fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-scheduled-node-contract-v2.json"
    )))
    .expect("candidate fixture");
    let control_fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-node-execution-contract-v1.json"
    )))
    .expect("control fixture");
    let schedule_fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-graph-execution-schedule-v1.json"
    )))
    .expect("schedule fixture");
    let candidate_json = candidate_fixture["expected"]["canonical_contract_json"]
        .as_str()
        .expect("candidate JSON")
        .to_owned();
    let candidate =
        GroupAgentScheduledNodeContractCandidate::decode_exact(&candidate_json).expect("candidate");
    let control_json = control_fixture["input"]["canonical_control_snapshot_json"]
        .as_str()
        .expect("control JSON");
    let control: GroupAgentGraphControlSnapshot = serde_json::from_str(control_json).unwrap();
    let schedule_json = schedule_fixture["canonical_execution_schedule_json"]
        .as_str()
        .expect("schedule JSON")
        .to_owned();
    let schedule = GroupAgentGraphExecutionSchedule::decode_exact(&schedule_json).unwrap();
    Fixture {
        input: AdmitGroupAgentScheduledNodeContractInput {
            graph_run_id: candidate.graph_run_id.clone(),
            contract_json: candidate_json,
            idempotency_key: "scheduled-contract-key".into(),
            admitted_at_ms: 80,
        },
        candidate,
        control,
        schedule: schedule_inspection(schedule_json, schedule),
    }
}

fn schedule_inspection(
    schedule_json: String,
    schedule: GroupAgentGraphExecutionSchedule,
) -> GroupAgentGraphExecutionScheduleInspection {
    GroupAgentGraphExecutionScheduleInspection {
        v: schedule.v,
        record: GroupAgentGraphExecutionScheduleRecord {
            v: schedule.v,
            schedule_id: schedule.schedule_id.clone(),
            graph_run_id: schedule.graph_run_id.clone(),
            graph_id: schedule.graph_id.clone(),
            control_snapshot_sha256: schedule.control_snapshot_sha256.clone(),
            schedule_sha256: schedule.schedule_sha256.clone(),
            schedule_bytes: schedule_json.len(),
            node_count: schedule.node_count,
            wave_count: schedule.wave_count,
            expected_last_event_seq: schedule.expected_last_event_seq,
            expected_last_event_sha256: schedule.expected_last_event_sha256.clone(),
            execution_contract_present: false,
            dispatch_authority_released: false,
            created_at_ms: 40,
        },
        schedule_json,
        schedule,
    }
}

fn record(
    candidate: &GroupAgentScheduledNodeContractCandidate,
    created_at_ms: u64,
) -> GroupAgentScheduledNodeContractRecord {
    GroupAgentScheduledNodeContractRecord {
        v: candidate.v,
        contract_id: candidate.contract_id.clone(),
        graph_run_id: candidate.graph_run_id.clone(),
        schedule_id: candidate.schedule_id.clone(),
        node_id: candidate.node.node_id.clone(),
        execution_ordinal: candidate.node.execution_ordinal,
        attempt: candidate.node.attempt,
        control_snapshot_sha256: candidate.control_snapshot_sha256.clone(),
        schedule_sha256: candidate.schedule_sha256.clone(),
        contract_sha256: candidate.contract_sha256.clone(),
        contract_bytes: candidate.canonical_json().unwrap().len(),
        request_id: candidate.request.request_id.clone(),
        request_sha256: candidate.request.request_sha256.clone(),
        project_lane_sha256: candidate.node.project_lane_sha256.clone(),
        expected_last_event_seq: candidate.expected_last_event_seq,
        expected_last_event_sha256: candidate.expected_last_event_sha256.clone(),
        predecessor_receipt_count: 0,
        lifecycle_contract_admitted: false,
        provider_request_present: false,
        execution_authority_released: false,
        dispatch_authority_released: false,
        progress_observed: false,
        successor_advance_authorized: false,
        created_at_ms,
    }
}
