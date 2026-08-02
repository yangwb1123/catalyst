use serde_json::Value;

use super::*;
use crate::{
    GroupAgentGraphControlSnapshot, GroupAgentGraphExecutionSchedule,
    MAX_GROUP_AGENT_GRAPH_NODE_ACCEPTANCE_BYTES, MAX_GROUP_AGENT_GRAPH_NODE_TASK_BYTES,
    group_agent_project_lane_sha256, group_agent_prompt_sha256,
};

#[test]
fn shared_candidate_fixture_is_byte_exact() {
    let fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-scheduled-node-contract-v2.json"
    )))
    .expect("candidate fixture");
    let json = fixture["expected"]["canonical_contract_json"]
        .as_str()
        .expect("canonical contract JSON");
    let candidate = GroupAgentScheduledNodeContractCandidate::decode_exact(json)
        .expect("exact shared scheduled contract");
    assert_eq!(candidate.canonical_json().unwrap(), json);
    assert_eq!(
        candidate.expected_sha256().unwrap(),
        candidate.contract_sha256
    );
    assert_eq!(
        candidate.request.expected_sha256().unwrap(),
        candidate.request.request_sha256
    );
    assert_eq!(
        super::codec::request_payload_json(&candidate.request).unwrap(),
        fixture["expected"]["canonical_request_payload_json"]
            .as_str()
            .expect("request payload")
    );
    assert_eq!(
        super::codec::contract_payload_json(&candidate).unwrap(),
        fixture["expected"]["canonical_contract_payload_json"]
            .as_str()
            .expect("contract payload")
    );
}

#[test]
fn source_validation_rejects_a_substituted_schedule() {
    let (candidate, control, mut schedule) = fixture();
    schedule.nodes[0].project_lane_sha256 = "0".repeat(64);
    let digest = schedule.expected_sha256().expect("schedule digest");
    schedule.schedule_id = format!("graph-execution-schedule-{digest}");
    schedule.schedule_sha256 = digest;
    schedule
        .validate()
        .expect("self-consistent substituted schedule");
    assert!(
        candidate
            .validate_against_control_and_schedule(&control, &schedule)
            .is_err()
    );
}

#[test]
fn source_validation_rejects_self_consistent_binding_substitutions() {
    let (candidate, control, schedule) = fixture();
    for drift in intrinsic_source_substitutions(&candidate) {
        assert_intrinsic_but_source_rejected(&drift, &control, &schedule);
    }
    for invalid in invalid_slot_substitutions(candidate) {
        assert!(
            invalid
                .validate_against_control_and_schedule(&control, &schedule)
                .is_err()
        );
    }
}

#[test]
fn self_consistent_effect_or_predecessor_claims_remain_invalid() {
    let (candidate, _, _) = fixture();
    let mut effectful = candidate.clone();
    effectful.execution_authority_released = true;
    resign_candidate(&mut effectful);
    assert!(effectful.validate().is_err());

    let mut predecessor = candidate;
    predecessor.request.required_predecessor_node_ids = vec!["backend".into()];
    resign_request(&mut predecessor.request);
    resign_candidate(&mut predecessor);
    assert!(predecessor.validate().is_err());
}

#[test]
fn decoder_rejects_wire_shape_encoding_bounds_and_identity_drift() {
    let (candidate, _, _) = fixture();
    let json = candidate.canonical_json().expect("candidate JSON");
    for mutation in wire_mutations(&json) {
        assert_ne!(mutation, json);
        assert!(GroupAgentScheduledNodeContractCandidate::decode_exact(&mutation).is_err());
    }
    assert_invalid_encoding_and_bounds(json);
}

#[test]
fn decoder_rejects_unstructured_user_prompt_with_resigned_identities() {
    let (mut candidate, _, _) = fixture();
    candidate.request.user_prompt = "arbitrary prose".into();
    resign_prompt_and_candidate(&mut candidate);

    let json = candidate.canonical_json().expect("self-consistent JSON");
    assert!(GroupAgentScheduledNodeContractCandidate::decode_exact(&json).is_err());
}

#[test]
fn decoder_rejects_escape_expanded_user_prompt_above_core_bound() {
    let (mut candidate, _, _) = fixture();
    candidate.request.user_prompt = group_agent_scheduled_node_user_prompt(
        &candidate.node.node_id,
        &"\"".repeat(MAX_GROUP_AGENT_GRAPH_NODE_TASK_BYTES),
        &"\"".repeat(MAX_GROUP_AGENT_GRAPH_NODE_ACCEPTANCE_BYTES),
    )
    .expect("escaped user Prompt");
    let core_bound =
        MAX_GROUP_AGENT_GRAPH_NODE_TASK_BYTES + MAX_GROUP_AGENT_GRAPH_NODE_ACCEPTANCE_BYTES + 1_024;
    assert!(candidate.request.user_prompt.len() > core_bound);
    resign_prompt_and_candidate(&mut candidate);

    let json = candidate.canonical_json().expect("self-consistent JSON");
    assert!(GroupAgentScheduledNodeContractCandidate::decode_exact(&json).is_err());
}

fn intrinsic_source_substitutions(
    candidate: &GroupAgentScheduledNodeContractCandidate,
) -> Vec<GroupAgentScheduledNodeContractCandidate> {
    let mut substitutions = identity_source_substitutions(candidate);
    substitutions.extend(digest_source_substitutions(candidate));
    substitutions.extend(node_source_substitutions(candidate));
    substitutions
}

fn identity_source_substitutions(
    candidate: &GroupAgentScheduledNodeContractCandidate,
) -> Vec<GroupAgentScheduledNodeContractCandidate> {
    let mut cross_run = candidate.clone();
    cross_run.graph_run_id = "other-graph-run".into();
    cross_run.request.graph_run_id = cross_run.graph_run_id.clone();
    resign_request(&mut cross_run.request);
    resign_candidate(&mut cross_run);

    let mut cross_schedule = candidate.clone();
    cross_schedule.schedule_sha256 = "0".repeat(64);
    cross_schedule.schedule_id = format!(
        "graph-execution-schedule-{}",
        cross_schedule.schedule_sha256
    );
    cross_schedule.request.schedule_id = cross_schedule.schedule_id.clone();
    cross_schedule.request.schedule_sha256 = cross_schedule.schedule_sha256.clone();
    resign_request(&mut cross_schedule.request);
    resign_candidate(&mut cross_schedule);
    vec![cross_run, cross_schedule]
}

fn digest_source_substitutions(
    candidate: &GroupAgentScheduledNodeContractCandidate,
) -> Vec<GroupAgentScheduledNodeContractCandidate> {
    ["source", "manifest", "plan", "control"]
        .into_iter()
        .map(|field| {
            let mut drift = candidate.clone();
            match field {
                "source" => drift.source_snapshot_sha256 = "0".repeat(64),
                "manifest" => drift.graph_manifest_sha256 = "0".repeat(64),
                "plan" => drift.core_plan_sha256 = "0".repeat(64),
                "control" => drift.control_snapshot_sha256 = "0".repeat(64),
                _ => unreachable!(),
            }
            resign_candidate(&mut drift);
            drift
        })
        .collect()
}

fn node_source_substitutions(
    candidate: &GroupAgentScheduledNodeContractCandidate,
) -> Vec<GroupAgentScheduledNodeContractCandidate> {
    let mut other_node = candidate.clone();
    other_node.node.node_id = "other-node".into();
    other_node.request.node_id = other_node.node.node_id.clone();
    let prompt: Value = serde_json::from_str(&other_node.request.user_prompt).expect("user Prompt");
    other_node.request.user_prompt = group_agent_scheduled_node_user_prompt(
        &other_node.node.node_id,
        prompt["task"].as_str().expect("task"),
        prompt["acceptance"].as_str().expect("acceptance"),
    )
    .expect("other-node Prompt");
    resign_prompt_and_candidate(&mut other_node);

    let mut other_lane = candidate.clone();
    other_lane.node.project_id = "other-project".into();
    other_lane.node.project_lane_sha256 =
        group_agent_project_lane_sha256(&other_lane.node.project_id);
    resign_candidate(&mut other_lane);
    vec![other_node, other_lane]
}

fn invalid_slot_substitutions(
    candidate: GroupAgentScheduledNodeContractCandidate,
) -> [GroupAgentScheduledNodeContractCandidate; 2] {
    let mut wrong_ordinal = candidate.clone();
    wrong_ordinal.node.execution_ordinal = 1;
    wrong_ordinal.request.execution_ordinal = 1;
    resign_request(&mut wrong_ordinal.request);
    resign_candidate(&mut wrong_ordinal);

    let mut wrong_attempt = candidate;
    wrong_attempt.node.attempt = 2;
    wrong_attempt.request.attempt = 2;
    resign_request(&mut wrong_attempt.request);
    resign_candidate(&mut wrong_attempt);
    [wrong_ordinal, wrong_attempt]
}

fn wire_mutations(json: &str) -> [String; 12] {
    [
        json.replacen("{\"v\":2,", "{\"v\":2,\"v\":2,", 1),
        json.replacen("{\"v\":2,", "{\"v\":2,\"unknown\":false,", 1),
        json.replacen("\"contract_scope\":\"schedule_initial_node_only\",", "", 1),
        json.replacen(
            "\"required_predecessor_node_ids\":[]",
            "\"required_predecessor_node_ids\":null",
            1,
        ),
        json.replacen(
            "{\"v\":2,\"scheduler_protocol_version\":1",
            "{\"scheduler_protocol_version\":1,\"v\":2",
            1,
        ),
        format!("{json}\n"),
        format!("{json}{{}}"),
        json.replacen("graph-run-fixture-v1", r"\u0067raph-run-fixture-v1", 1),
        json.replacen(
            "\"execution_ordinal\":0,",
            "\"execution_ordinal\":0,\"unknown\":0,",
            1,
        ),
        json.replacen(
            "\"required_predecessor_node_ids\":[]",
            "\"required_predecessor_node_ids\":[\"frontend\"]",
            1,
        ),
        json.replacen(
            "\"request_id\":\"scheduled-node-request-",
            "\"request_id\":\"scheduled-node-request-0",
            1,
        ),
        json.replacen(
            "\"contract_id\":\"scheduled-node-contract-",
            "\"contract_id\":\"scheduled-node-contract-0",
            1,
        ),
    ]
}

fn assert_invalid_encoding_and_bounds(json: String) {
    let mut invalid_utf8 = json.into_bytes();
    let position = invalid_utf8
        .windows("graph-run-fixture-v1".len())
        .position(|window| window == b"graph-run-fixture-v1")
        .expect("fixture identifier");
    invalid_utf8[position] = 0xff;
    assert!(GroupAgentScheduledNodeContractCandidate::decode_exact_bytes(&invalid_utf8).is_err());
    assert!(
        GroupAgentScheduledNodeContractCandidate::decode_exact_bytes(&vec![
            b' ';
            MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_BYTES
                + 1
        ])
        .is_err()
    );
}

fn fixture() -> (
    GroupAgentScheduledNodeContractCandidate,
    GroupAgentGraphControlSnapshot,
    GroupAgentGraphExecutionSchedule,
) {
    let candidate_fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-scheduled-node-contract-v2.json"
    )))
    .expect("candidate fixture");
    let control_json = candidate_fixture["input"]["canonical_control_snapshot_json"]
        .as_str()
        .expect("control JSON");
    let candidate_json = candidate_fixture["expected"]["canonical_contract_json"]
        .as_str()
        .expect("candidate JSON");
    let schedule_fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-graph-execution-schedule-v1.json"
    )))
    .expect("schedule fixture");
    let schedule_json = schedule_fixture["canonical_execution_schedule_json"]
        .as_str()
        .expect("schedule JSON");
    (
        GroupAgentScheduledNodeContractCandidate::decode_exact(candidate_json).expect("candidate"),
        serde_json::from_str(control_json).expect("control"),
        GroupAgentGraphExecutionSchedule::decode_exact(schedule_json).expect("schedule"),
    )
}

fn resign_request(request: &mut GroupAgentScheduledNodeRequest) {
    let digest = request.expected_sha256().expect("request digest");
    request.request_id = format!("scheduled-node-request-{digest}");
    request.request_sha256 = digest;
}

fn resign_prompt_and_candidate(candidate: &mut GroupAgentScheduledNodeContractCandidate) {
    let request = &mut candidate.request;
    request.user_prompt_bytes = request.user_prompt.len();
    request.user_prompt_sha256 = group_agent_prompt_sha256(&request.user_prompt);
    resign_request(request);
    resign_candidate(candidate);
}

fn resign_candidate(candidate: &mut GroupAgentScheduledNodeContractCandidate) {
    let digest = candidate.expected_sha256().expect("contract digest");
    candidate.contract_id = format!("scheduled-node-contract-{digest}");
    candidate.contract_sha256 = digest;
}

fn assert_intrinsic_but_source_rejected(
    candidate: &GroupAgentScheduledNodeContractCandidate,
    control: &GroupAgentGraphControlSnapshot,
    schedule: &GroupAgentGraphExecutionSchedule,
) {
    candidate
        .validate()
        .expect("intrinsically valid substitution");
    assert!(
        candidate
            .validate_against_control_and_schedule(control, schedule)
            .is_err()
    );
}
