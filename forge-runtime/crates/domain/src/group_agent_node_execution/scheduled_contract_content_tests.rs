use crate::{
    group_agent_scheduled_node_predecessor_output, group_agent_scheduled_node_user_prompt,
    group_agent_scheduled_node_user_prompt_with_output, GroupAgentScheduledNodeContractScope,
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
    let embedded = group_agent_scheduled_node_predecessor_output(&with_output)
        .expect("decode output");
    assert_eq!(
        embedded.as_deref(),
        Some("frontend produced: login flow verified")
    );
    assert!(with_output.contains("predecessor_output"));

    let plain = group_agent_scheduled_node_user_prompt("backend", "backend task", "backend acceptance")
        .expect("plain prompt");
    let embedded = group_agent_scheduled_node_predecessor_output(&plain).expect("decode plain");
    assert_eq!(embedded, None);
    assert!(!plain.contains("predecessor_output"));
}

#[test]
fn content_flag_must_match_prompt_field() {
    // included=true 但 prompt 无内容 → 拒绝
    let (mut candidate, _, _) = fixture();
    candidate.contract_scope = GroupAgentScheduledNodeContractScope::ScheduleSuccessorOnly;
    candidate.request.predecessor_content_included = true;
    candidate.request.predecessor_terminal_receipts = vec![
        crate::GroupAgentScheduledNodePredecessorReceipt {
            predecessor_node_id: "frontend".into(),
            predecessor_attempt: 1,
            terminal_event_seq: 0,
            terminal_event_sha256: String::new(),
            terminal_receipt_id: format!("scheduled-node-terminal-receipt-{}", "a".repeat(64)),
            terminal_receipt_sha256: "a".repeat(64),
            node_outcome: crate::GroupAgentScheduledNodePredecessorOutcome::Completed,
            provider_request_id: "scheduled-node-provider-request-frontend".into(),
            dispatch_id: "dispatch-frontend".into(),
        },
    ];
    resign_prompt_and_candidate(&mut candidate);
    assert!(
        candidate.validate().is_err(),
        "content flag without embedded output must fail"
    );
}
