use crate::{
    GroupAgentGraphControlSnapshot, GroupAgentGraphExecutionSchedule,
    GroupAgentScheduledNodeContractCandidate, GroupAgentScheduledNodeContractScope,
    GroupAgentScheduledNodePredecessorOutcome, GroupAgentScheduledNodePredecessorReceipt,
    group_agent_project_lane_sha256, group_agent_scheduled_node_user_prompt,
    group_agent_scheduled_node_user_prompt_with_output,
};

use super::tests::{fixture, resign_prompt_and_candidate};

#[test]
fn source_bound_successor_with_exact_predecessor_receipts_validates() {
    let (candidate, control, schedule) = source_bound_successor_fixture();
    candidate
        .validate_against_control_and_schedule(&control, &schedule)
        .expect("source-bound successor candidate must validate");
}

#[test]
fn source_validation_rejects_an_intrinsically_exact_unscheduled_predecessor() {
    let (mut candidate, control, schedule) = source_bound_successor_fixture();
    candidate.request.required_predecessor_node_ids[1] = "unrelated".into();
    candidate.request.predecessor_terminal_receipts[1].predecessor_node_id = "unrelated".into();
    resign_prompt_and_candidate(&mut candidate);

    candidate
        .validate()
        .expect("the forged request remains intrinsically exact");
    assert!(
        candidate
            .validate_against_control_and_schedule(&control, &schedule)
            .is_err(),
        "source binding must reject predecessor IDs outside the schedule"
    );
}

#[test]
fn intrinsic_rejects_failed_and_uncertain_predecessor_outcomes() {
    for outcome in [
        GroupAgentScheduledNodePredecessorOutcome::Failed,
        GroupAgentScheduledNodePredecessorOutcome::FailedUncertain,
    ] {
        let (mut candidate, _, _) = source_bound_successor_fixture();
        candidate.request.predecessor_terminal_receipts[0].node_outcome = outcome;
        resign_prompt_and_candidate(&mut candidate);
        assert!(
            candidate.validate().is_err(),
            "only a completed predecessor may advance a successor"
        );
    }
}

#[test]
fn intrinsic_rejects_nonempty_reserved_terminal_event_fields() {
    let (mut sequenced, _, _) = source_bound_successor_fixture();
    sequenced.request.predecessor_terminal_receipts[0].terminal_event_seq = 2;
    resign_prompt_and_candidate(&mut sequenced);
    assert!(sequenced.validate().is_err());

    let (mut digested, _, _) = source_bound_successor_fixture();
    digested.request.predecessor_terminal_receipts[0].terminal_event_sha256 = "0".repeat(64);
    resign_prompt_and_candidate(&mut digested);
    assert!(digested.validate().is_err());
}

#[test]
fn predecessor_receipts_must_follow_the_scheduled_direct_order() {
    let (mut shuffled, _, _) = source_bound_successor_fixture();
    shuffled.request.predecessor_terminal_receipts.swap(0, 1);
    resign_prompt_and_candidate(&mut shuffled);
    assert!(
        shuffled.validate().is_err(),
        "receipt order must match required predecessor order"
    );

    let (mut reordered, control, schedule) = source_bound_successor_fixture();
    reordered.request.required_predecessor_node_ids.swap(0, 1);
    reordered.request.predecessor_terminal_receipts.swap(0, 1);
    resign_prompt_and_candidate(&mut reordered);
    reordered
        .validate()
        .expect("the reordered pair remains intrinsically aligned");
    assert!(
        reordered
            .validate_against_control_and_schedule(&control, &schedule)
            .is_err(),
        "source binding must preserve the schedule's direct-predecessor order"
    );
}

#[test]
fn source_validation_rejects_resigned_successor_identity_drift() {
    for field in ["project", "member role", "agent profile"] {
        let (mut candidate, control, schedule) = source_bound_successor_fixture();
        match field {
            "project" => {
                candidate.node.project_id = "other-project".into();
                candidate.node.project_lane_sha256 =
                    group_agent_project_lane_sha256(&candidate.node.project_id);
            }
            "member role" => candidate.node.member_role = "other-role".into(),
            _ => candidate.node.agent_profile = "other-profile".into(),
        }
        resign_prompt_and_candidate(&mut candidate);
        assert_source_rejected(&candidate, &control, &schedule, field);
    }
}

#[test]
fn source_validation_rejects_resigned_successor_system_prompt_drift() {
    let (mut candidate, control, schedule) = source_bound_successor_fixture();
    candidate.request.system_prompt = "different manager instruction".into();
    resign_prompt_and_candidate(&mut candidate);
    assert_source_rejected(&candidate, &control, &schedule, "system Prompt");
}

#[test]
fn source_validation_preserves_output_but_rejects_source_task_drift() {
    let (mut candidate, control, schedule) = source_bound_successor_fixture();
    let source = &control.manifest.nodes[2];
    candidate.request.predecessor_content_included = true;
    candidate.request.user_prompt = group_agent_scheduled_node_user_prompt_with_output(
        &source.node_id,
        &source.task,
        &source.acceptance,
        "durable predecessor bytes",
    )
    .expect("source Prompt with output");
    resign_prompt_and_candidate(&mut candidate);
    candidate
        .validate_against_control_and_schedule(&control, &schedule)
        .expect("exact source Prompt preserves predecessor output");

    candidate.request.user_prompt = group_agent_scheduled_node_user_prompt_with_output(
        &source.node_id,
        "substituted task",
        &source.acceptance,
        "durable predecessor bytes",
    )
    .expect("substituted Prompt");
    resign_prompt_and_candidate(&mut candidate);
    assert_source_rejected(&candidate, &control, &schedule, "user Prompt task");
}

fn assert_source_rejected(
    candidate: &GroupAgentScheduledNodeContractCandidate,
    control: &GroupAgentGraphControlSnapshot,
    schedule: &GroupAgentGraphExecutionSchedule,
    subject: &str,
) {
    candidate.validate().expect("intrinsically exact forgery");
    assert!(
        candidate
            .validate_against_control_and_schedule(control, schedule)
            .is_err(),
        "source validation accepted drifted {subject}"
    );
}

fn source_bound_successor_fixture() -> (
    GroupAgentScheduledNodeContractCandidate,
    GroupAgentGraphControlSnapshot,
    GroupAgentGraphExecutionSchedule,
) {
    let (mut candidate, control, schedule) = fixture();
    let scheduled = schedule.nodes[2].clone();
    let source = control.manifest.nodes[2].clone();
    candidate.contract_scope = GroupAgentScheduledNodeContractScope::ScheduleSuccessorOnly;
    candidate.node.execution_ordinal = scheduled.execution_ordinal;
    candidate.node.node_id.clone_from(&scheduled.node_id);
    candidate.node.authored_node_index = scheduled.authored_node_index;
    candidate.node.topology_wave_index = scheduled.topology_wave_index;
    candidate.node.project_id = source.project_id;
    candidate.node.member_role = source.member_role;
    candidate.node.agent_profile = source.agent_profile;
    candidate.node.project_lane_sha256 = scheduled.project_lane_sha256;
    candidate.request.execution_ordinal = scheduled.execution_ordinal;
    candidate.request.node_id = scheduled.node_id;
    candidate.request.required_predecessor_node_ids = scheduled.direct_predecessor_node_ids;
    candidate.request.predecessor_terminal_receipts = vec![
        verified_predecessor_receipt("frontend", "a"),
        verified_predecessor_receipt("backend", "b"),
    ];
    candidate.request.user_prompt = group_agent_scheduled_node_user_prompt(
        &candidate.node.node_id,
        &source.task,
        &source.acceptance,
    )
    .expect("successor Prompt");
    resign_prompt_and_candidate(&mut candidate);
    (candidate, control, schedule)
}

fn verified_predecessor_receipt(
    node_id: &str,
    digest_character: &str,
) -> GroupAgentScheduledNodePredecessorReceipt {
    let digest = digest_character.repeat(64);
    GroupAgentScheduledNodePredecessorReceipt {
        predecessor_node_id: node_id.into(),
        predecessor_attempt: 1,
        terminal_event_seq: 0,
        terminal_event_sha256: String::new(),
        terminal_receipt_id: format!("scheduled-node-terminal-receipt-{digest}"),
        terminal_receipt_sha256: digest,
        node_outcome: GroupAgentScheduledNodePredecessorOutcome::Completed,
        provider_request_id: format!("scheduled-node-provider-request-{node_id}"),
        dispatch_id: format!("dispatch-{node_id}"),
    }
}
