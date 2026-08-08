use crate::{
    GroupAgentScheduledNodeContractCandidate, GroupAgentScheduledNodeContractScope,
    GroupAgentScheduledNodePredecessorOutcome, GroupAgentScheduledNodePredecessorReceipt,
    MAX_GROUP_AGENT_GRAPH_NODE_ACCEPTANCE_BYTES, MAX_GROUP_AGENT_GRAPH_NODE_TASK_BYTES,
    MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_BYTES,
    MAX_GROUP_AGENT_SCHEDULED_NODE_PREDECESSOR_OUTPUT_BYTES,
    MAX_GROUP_AGENT_SCHEDULED_NODE_USER_PROMPT_BYTES,
    group_agent_scheduled_node_predecessor_output, group_agent_scheduled_node_user_prompt,
    group_agent_scheduled_node_user_prompt_with_output,
};

use super::tests::{fixture, resign_prompt_and_candidate};

#[test]
fn prompt_with_predecessor_output_round_trips_exactly() {
    let with_output = group_agent_scheduled_node_user_prompt_with_output(
        "backend",
        "backend task",
        "backend acceptance",
        "frontend produced: login flow verified",
    )
    .expect("prompt with output");
    let embedded =
        group_agent_scheduled_node_predecessor_output(&with_output).expect("decode output");
    assert_eq!(
        embedded.as_deref(),
        Some("frontend produced: login flow verified")
    );
    assert!(with_output.contains("predecessor_output"));

    let plain =
        group_agent_scheduled_node_user_prompt("backend", "backend task", "backend acceptance")
            .expect("plain prompt");
    let embedded = group_agent_scheduled_node_predecessor_output(&plain).expect("decode plain");
    assert_eq!(embedded, None);
    assert!(!plain.contains("predecessor_output"));
}

#[test]
fn content_flag_must_match_prompt_field() {
    let mut candidate = successor_fixture(None);
    candidate.request.predecessor_content_included = true;
    resign_prompt_and_candidate(&mut candidate);
    assert!(
        candidate.validate().is_err(),
        "content flag without embedded output must fail"
    );
}

#[test]
fn successor_with_matching_predecessor_content_validates() {
    let candidate = successor_fixture(Some("frontend produced: login flow verified"));
    candidate
        .validate()
        .expect("content-bearing successor must validate");
    let canonical = candidate.canonical_json().expect("canonical candidate");
    assert_eq!(
        GroupAgentScheduledNodeContractCandidate::decode_exact(&canonical)
            .expect("exact content-bearing successor"),
        candidate
    );
}

#[test]
fn successor_content_requires_a_predecessor_receipt() {
    let mut candidate = successor_fixture(Some("unbound predecessor output"));
    candidate.request.predecessor_terminal_receipts.clear();
    resign_prompt_and_candidate(&mut candidate);
    assert!(
        candidate.validate().is_err(),
        "content-bearing successor without an authenticating receipt must fail"
    );
}

#[test]
fn successor_rejects_unrelated_or_ambiguous_predecessor_evidence() {
    let mut extra = successor_fixture(Some("authenticated predecessor output"));
    extra
        .request
        .predecessor_terminal_receipts
        .push(unrelated_predecessor_receipt());
    resign_prompt_and_candidate(&mut extra);
    assert!(
        extra.validate().is_err(),
        "an unrelated extra receipt must fail"
    );

    let mut ambiguous = successor_fixture(Some("authenticated predecessor output"));
    ambiguous.request.required_predecessor_node_ids = vec!["frontend".into(), "frontend".into()];
    ambiguous
        .request
        .predecessor_terminal_receipts
        .push(unrelated_predecessor_receipt());
    resign_prompt_and_candidate(&mut ambiguous);
    assert!(
        ambiguous.validate().is_err(),
        "duplicate required IDs must not hide an unrelated receipt"
    );
}

#[test]
fn successor_accepts_predecessor_output_at_one_mibibyte_limit() {
    let output = "x".repeat(MAX_GROUP_AGENT_SCHEDULED_NODE_PREDECESSOR_OUTPUT_BYTES);
    successor_fixture(Some(&output))
        .validate()
        .expect("one-MiB predecessor output must validate");
}

#[test]
fn successor_rejects_predecessor_output_above_one_mibibyte_limit() {
    let output = "x".repeat(MAX_GROUP_AGENT_SCHEDULED_NODE_PREDECESSOR_OUTPUT_BYTES + 1);
    assert!(
        successor_fixture(Some(&output)).validate().is_err(),
        "predecessor output above one MiB must fail its field bound"
    );
}

#[test]
fn maximum_escape_expanded_user_prompt_and_candidate_validate() {
    let mut candidate = successor_fixture(None);
    let escaped_node_id = "\"".repeat(crate::MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES);
    candidate.node.node_id.clone_from(&escaped_node_id);
    candidate.request.node_id = escaped_node_id;
    candidate.request.user_prompt = group_agent_scheduled_node_user_prompt_with_output(
        &candidate.request.node_id,
        &"\"".repeat(MAX_GROUP_AGENT_GRAPH_NODE_TASK_BYTES),
        &"\"".repeat(MAX_GROUP_AGENT_GRAPH_NODE_ACCEPTANCE_BYTES),
        &"\"".repeat(MAX_GROUP_AGENT_SCHEDULED_NODE_PREDECESSOR_OUTPUT_BYTES),
    )
    .expect("maximally escaped Prompt");
    assert_eq!(
        candidate.request.user_prompt.len(),
        MAX_GROUP_AGENT_SCHEDULED_NODE_USER_PROMPT_BYTES,
        "the public Prompt bound must include exact canonical JSON escaping"
    );
    candidate.request.predecessor_content_included = true;
    resign_prompt_and_candidate(&mut candidate);

    candidate
        .validate()
        .expect("all individually bounded Prompt fields must remain reachable");
    let canonical = candidate.canonical_json().expect("canonical candidate");
    assert!(canonical.len() > 4 * 1024 * 1024);
    assert!(canonical.len() <= MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_BYTES);
    GroupAgentScheduledNodeContractCandidate::decode_exact(&canonical)
        .expect("escape-expanded candidate must fit the candidate bound");
}

#[test]
fn initial_candidate_still_rejects_predecessor_content() {
    let (mut candidate, _, _) = fixture();
    candidate.request.user_prompt = group_agent_scheduled_node_user_prompt_with_output(
        &candidate.request.node_id,
        "initial task",
        "initial acceptance",
        "unexpected predecessor output",
    )
    .expect("content-bearing initial Prompt");
    candidate.request.predecessor_content_included = true;
    resign_prompt_and_candidate(&mut candidate);
    assert!(
        candidate.validate().is_err(),
        "initial candidate must remain content-free"
    );
}

fn successor_fixture(predecessor_output: Option<&str>) -> GroupAgentScheduledNodeContractCandidate {
    let (mut candidate, _, _) = fixture();
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
    resign_prompt_and_candidate(&mut candidate);
    candidate
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
