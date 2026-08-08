use serde_json::Value;

use crate::runtime_domain::{
    AdmitGroupAgentScheduledNodeContractDisposition, AdmitGroupAgentScheduledNodeContractResult,
    GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION, GroupAgentGraphControlSnapshot,
    GroupAgentGraphExecutionSchedule, GroupAgentGraphExecutionScheduleInspection,
    GroupAgentGraphExecutionScheduleRecord, GroupAgentScheduledNodeContractCandidate,
    GroupAgentScheduledNodeContractInspection, GroupAgentScheduledNodeContractRecord,
    GroupAgentScheduledNodeContractScope, GroupAgentScheduledNodePredecessorOutcome,
    GroupAgentScheduledNodePredecessorReceipt, group_agent_prompt_sha256,
    group_agent_scheduled_node_user_prompt, group_agent_scheduled_node_user_prompt_with_output,
};

use super::{
    AdmitGroupAgentScheduledNodeContractInput, AdmitGroupAgentScheduledNodeSuccessorInput,
    ExportGroupAgentGraphControl, GroupAgentScheduledNodeContractService,
    GroupAgentScheduledNodeContractServiceError, GroupAgentScheduledNodeSuccessorService,
    scheduled_contract_validation::{
        admission_request, parse_admit_input, validate_admit_result, validate_list,
        validate_successor_list,
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

#[test]
fn zero_receipt_successor_record_rejects_ordinal_above_bound() {
    let candidate = successor_candidate(None);
    let mut stored = record(&candidate, 80);
    stored.execution_ordinal = 32;
    stored.predecessor_receipt_count = 0;
    assert!(stored.validate().is_err());
}

#[test]
fn initial_service_preflight_rejects_successor_scope() {
    let candidate = successor_candidate(None);
    let input = AdmitGroupAgentScheduledNodeContractInput {
        graph_run_id: candidate.graph_run_id.clone(),
        contract_json: candidate.canonical_json().expect("successor JSON"),
        idempotency_key: "wrong-initial-path".into(),
        admitted_at_ms: 80,
    };
    assert!(GroupAgentScheduledNodeContractService::preflight_admit(&input).is_err());
}

#[test]
fn successor_list_allows_distinct_slots_in_the_same_run() {
    let candidate = successor_candidate(None);
    let first = record(&candidate, 80);
    let mut second = first.clone();
    second.contract_sha256 = "e".repeat(64);
    second.contract_id = format!("scheduled-node-contract-{}", second.contract_sha256);
    second.request_sha256 = "f".repeat(64);
    second.request_id = format!("scheduled-node-request-{}", second.request_sha256);
    second.node_id = "sso".into();
    second.execution_ordinal = 2;
    validate_successor_list(&[first.clone(), second], None, 10)
        .expect("distinct successor slots may share one Graph Run");
    assert!(validate_list(&[first.clone(), first], None, 10).is_err());
}

#[test]
fn successor_preflight_accepts_disclosed_content_and_preserves_plain_path() {
    let content = "frontend produced: login flow verified";
    let content_candidate = successor_candidate(Some(content));
    GroupAgentScheduledNodeSuccessorService::preflight_admit(&successor_input(
        &content_candidate,
        Some(content),
    ))
    .expect("content-bearing successor preflight");

    let plain_candidate = successor_candidate(None);
    GroupAgentScheduledNodeSuccessorService::preflight_admit(&successor_input(
        &plain_candidate,
        None,
    ))
    .expect("content-free successor preflight");
}

#[test]
fn successor_preflight_rejects_an_unrelated_extra_receipt() {
    let content = "frontend produced: login flow verified";
    let mut candidate = successor_candidate(Some(content));
    candidate
        .request
        .predecessor_terminal_receipts
        .push(unrelated_predecessor_receipt());
    resign_candidate(&mut candidate);

    assert!(
        GroupAgentScheduledNodeSuccessorService::preflight_admit(&successor_input(
            &candidate,
            Some(content),
        ))
        .is_err(),
        "application preflight must inherit the domain's exact receipt-set rule"
    );
}

#[test]
fn successor_preflight_rejects_failed_predecessor_outcomes() {
    for outcome in [
        GroupAgentScheduledNodePredecessorOutcome::Failed,
        GroupAgentScheduledNodePredecessorOutcome::FailedUncertain,
    ] {
        let mut candidate = successor_candidate(None);
        candidate.request.predecessor_terminal_receipts[0].node_outcome = outcome;
        resign_candidate(&mut candidate);
        assert!(
            GroupAgentScheduledNodeSuccessorService::preflight_admit(&successor_input(
                &candidate, None,
            ))
            .is_err(),
            "application preflight must reject a non-completed predecessor"
        );
    }
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

pub(super) fn successor_candidate(
    predecessor_output: Option<&str>,
) -> GroupAgentScheduledNodeContractCandidate {
    let mut candidate = fixture().candidate;
    candidate.contract_scope = GroupAgentScheduledNodeContractScope::ScheduleSuccessorOnly;
    candidate.node.execution_ordinal = 1;
    candidate.node.node_id = "backend".into();
    candidate.request.execution_ordinal = 1;
    candidate.request.node_id = candidate.node.node_id.clone();
    candidate.request.required_predecessor_node_ids = vec!["frontend".into()];
    candidate.request.predecessor_terminal_receipts = vec![predecessor_receipt()];
    candidate.request.user_prompt = match predecessor_output {
        Some(output) => group_agent_scheduled_node_user_prompt_with_output(
            &candidate.node.node_id,
            "backend task",
            "backend acceptance",
            output,
        ),
        None => group_agent_scheduled_node_user_prompt(
            &candidate.node.node_id,
            "backend task",
            "backend acceptance",
        ),
    }
    .expect("successor Prompt");
    candidate.request.predecessor_content_included = predecessor_output.is_some();
    resign_candidate(&mut candidate);
    candidate
}

fn successor_input(
    candidate: &GroupAgentScheduledNodeContractCandidate,
    predecessor_content: Option<&str>,
) -> AdmitGroupAgentScheduledNodeSuccessorInput {
    AdmitGroupAgentScheduledNodeSuccessorInput {
        graph_run_id: candidate.graph_run_id.clone(),
        contract_json: candidate.canonical_json().expect("successor JSON"),
        idempotency_key: "scheduled-successor-key".into(),
        admitted_at_ms: 81,
        predecessor_content: predecessor_content.map(str::to_owned),
    }
}

pub(super) fn resign_candidate(candidate: &mut GroupAgentScheduledNodeContractCandidate) {
    candidate.request.user_prompt_bytes = candidate.request.user_prompt.len();
    candidate.request.user_prompt_sha256 =
        group_agent_prompt_sha256(&candidate.request.user_prompt);
    let request_digest = candidate.request.expected_sha256().expect("request digest");
    candidate.request.request_id = format!("scheduled-node-request-{request_digest}");
    candidate.request.request_sha256 = request_digest;
    let contract_digest = candidate.expected_sha256().expect("contract digest");
    candidate.contract_id = format!("scheduled-node-contract-{contract_digest}");
    candidate.contract_sha256 = contract_digest;
}

fn predecessor_receipt() -> GroupAgentScheduledNodePredecessorReceipt {
    GroupAgentScheduledNodePredecessorReceipt {
        predecessor_node_id: "frontend".into(),
        predecessor_attempt: 1,
        terminal_event_seq: 0,
        terminal_event_sha256: String::new(),
        terminal_receipt_id: format!("scheduled-node-terminal-receipt-{}", "a".repeat(64)),
        terminal_receipt_sha256: "a".repeat(64),
        node_outcome: GroupAgentScheduledNodePredecessorOutcome::Completed,
        provider_request_id: "scheduled-node-provider-request-frontend".into(),
        dispatch_id: "dispatch-frontend".into(),
    }
}

fn unrelated_predecessor_receipt() -> GroupAgentScheduledNodePredecessorReceipt {
    let mut receipt = predecessor_receipt();
    receipt.predecessor_node_id = "unrelated".into();
    receipt.terminal_receipt_id = format!("scheduled-node-terminal-receipt-{}", "b".repeat(64));
    receipt.terminal_receipt_sha256 = "b".repeat(64);
    receipt.provider_request_id = "scheduled-node-provider-request-unrelated".into();
    receipt.dispatch_id = "dispatch-unrelated".into();
    receipt
}
