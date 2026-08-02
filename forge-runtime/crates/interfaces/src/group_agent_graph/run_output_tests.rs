use crate::runtime_domain::{
    GroupAgentGraphRunInspection, GroupAgentGraphRunStatus, GroupAgentNodeTerminalControl,
};
use serde::Deserialize;

use super::{GroupAgentGraphRunCliOutput, write_output};

#[derive(Deserialize)]
struct SharedFixture {
    canonical_terminal_control_json: String,
}

#[test]
fn terminal_graph_statuses_are_stable_in_json_and_human_output() {
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
        inspection.run.status = status;
        let output = GroupAgentGraphRunCliOutput::run(inspection, false);
        let json: serde_json::Value = serde_json::from_str(&render(&output, true)).unwrap();
        assert_eq!(json["inspection"]["run"]["status"], expected);
        assert!(render(&output, false).contains(&format!("status={expected}")));
    }
}

fn render(output: &GroupAgentGraphRunCliOutput, json: bool) -> String {
    let mut bytes = Vec::new();
    write_output(output, json, &mut bytes).expect("render Graph Run output");
    String::from_utf8(bytes).expect("UTF-8 output")
}

fn inspection() -> GroupAgentGraphRunInspection {
    let fixture: SharedFixture = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-node-terminal-lifecycle-v1.json"
    )))
    .expect("shared terminal fixture");
    let control = GroupAgentNodeTerminalControl::decode_exact(
        fixture.canonical_terminal_control_json.as_bytes(),
    )
    .expect("shared terminal control");
    GroupAgentGraphRunInspection {
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
    }
}
