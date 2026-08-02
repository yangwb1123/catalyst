use crate::runtime_domain::{
    GroupAgentGraphRunInspection, GroupAgentGraphRunStatus, GroupAgentNodeLifecycleInspection,
    GroupAgentNodeTerminalControl,
};
use forge_runtime_application::ExecuteGroupAgentNodeDispatchResult;
use serde::Deserialize;

use super::{GroupAgentNodeDispatchExecutionCliOutput, write_output};

const PRIVATE_RESULT: &str = "private-result-must-stay-hidden\n\u{1b}[2J";

#[derive(Deserialize)]
struct SharedFixture {
    canonical_terminal_control_json: String,
}

#[test]
fn default_output_is_metadata_only_and_omits_private_result() {
    let output = GroupAgentNodeDispatchExecutionCliOutput::from_result(
        ExecuteGroupAgentNodeDispatchResult::AlreadyClaimed(inspection()),
        false,
    );

    assert!(output.metadata_only);
    assert!(output.result_text.is_none());
    assert!(!output.dispatch_performed_this_invocation);
    assert!(!output.database_written_this_invocation);
    for json in [true, false] {
        let rendered = render(&output, json);
        assert!(!rendered.contains(PRIVATE_RESULT));
        assert!(!rendered.contains("private-result-must-stay-hidden"));
        assert!(!rendered.contains("result_text"));
        assert!(!rendered.contains("result:"));
    }
}

#[test]
fn explicit_result_is_json_exact_and_human_terminal_escaped() {
    let output = GroupAgentNodeDispatchExecutionCliOutput::from_result(
        ExecuteGroupAgentNodeDispatchResult::Terminalized(inspection()),
        true,
    );

    assert!(!output.metadata_only);
    assert_eq!(output.result_text.as_deref(), Some(PRIVATE_RESULT));
    assert!(output.dispatch_performed_this_invocation);
    assert!(output.database_written_this_invocation);
    let json: serde_json::Value = serde_json::from_str(&render(&output, true)).unwrap();
    assert_eq!(json["result_text"], PRIVATE_RESULT);
    assert_eq!(json["metadata_only"], false);
    let human = render(&output, false);
    assert!(human.contains("result: private-result-must-stay-hidden\\n\\x1b[2J"));
    assert!(!human.contains('\u{1b}'));
}

#[test]
fn quarantined_claim_remains_metadata_only_even_when_result_was_requested() {
    let mut inspection = inspection();
    inspection.graph_run.run.status = GroupAgentGraphRunStatus::DispatchUnknown;
    inspection.artifact = None;
    inspection.artifact_json = None;
    let output = GroupAgentNodeDispatchExecutionCliOutput::from_result(
        ExecuteGroupAgentNodeDispatchResult::AlreadyClaimed(inspection),
        true,
    );

    assert!(output.metadata_only);
    assert!(output.result_text.is_none());
    assert!(!output.dispatch_performed_this_invocation);
    assert!(!output.database_written_this_invocation);
    let json: serde_json::Value = serde_json::from_str(&render(&output, true)).unwrap();
    assert_eq!(json["metadata_only"], true);
    assert!(json.get("result_text").is_none());
}

#[test]
fn every_graph_terminal_status_has_stable_json_and_human_text() {
    for (status, expected) in [
        (
            GroupAgentGraphRunStatus::DispatchUnknown,
            "dispatch_unknown",
        ),
        (GroupAgentGraphRunStatus::Completed, "completed"),
        (GroupAgentGraphRunStatus::Failed, "failed"),
        (
            GroupAgentGraphRunStatus::FailedUncertain,
            "failed_uncertain",
        ),
    ] {
        let mut inspection = inspection();
        inspection.graph_run.run.status = status;
        let output = GroupAgentNodeDispatchExecutionCliOutput::from_result(
            ExecuteGroupAgentNodeDispatchResult::AlreadyClaimed(inspection),
            false,
        );
        let json: serde_json::Value = serde_json::from_str(&render(&output, true)).unwrap();
        assert_eq!(json["graph_status"], expected);
        assert!(render(&output, false).starts_with(&format!("graph dispatch {expected} ·")));
    }
}

fn render(output: &GroupAgentNodeDispatchExecutionCliOutput, json: bool) -> String {
    let mut bytes = Vec::new();
    write_output(output, json, &mut bytes).expect("render execution output");
    String::from_utf8(bytes).expect("UTF-8 output")
}

fn inspection() -> GroupAgentNodeLifecycleInspection {
    let fixture: SharedFixture = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-node-terminal-lifecycle-v1.json"
    )))
    .expect("shared terminal fixture");
    let mut control = GroupAgentNodeTerminalControl::decode_exact(
        fixture.canonical_terminal_control_json.as_bytes(),
    )
    .expect("shared terminal control");
    control.artifact.output_text = PRIVATE_RESULT.into();
    let graph_run = GroupAgentGraphRunInspection {
        v: control.graph_run.v,
        run: control.graph_run,
        plan_json: control.plan.canonical_json().unwrap(),
        plan: control.plan,
        event_jsons: control
            .journal_events
            .iter()
            .map(|event| event.canonical_json().unwrap())
            .collect(),
        events: control.journal_events,
    };
    GroupAgentNodeLifecycleInspection {
        v: 1,
        graph_run,
        claim_json: control.claim.canonical_json().unwrap(),
        claim: control.claim,
        active_lane_json: Some(control.active_lane.canonical_json().unwrap()),
        active_lane: Some(control.active_lane),
        artifact_json: None,
        artifact: Some(control.artifact),
        terminal_receipt: None,
        terminal_receipt_json: None,
    }
}
