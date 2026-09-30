use super::*;

#[test]
fn remote_runner_dispatch_plan_preview_requires_one_bounded_input() {
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "runner-dispatch-plan-preview",
            "--input",
            "request.json",
        ])
        .unwrap(),
        RemoteCommand::RunnerDispatchPlanPreview {
            input: "request.json".into(),
            instance_id: None,
            instance_view: None,
        }
    );
    assert!(parse_remote_command(&["remote", "runner-dispatch-plan-preview"]).is_err());
    assert!(
        parse_remote_command(&[
            "remote",
            "runner-dispatch-plan-preview",
            "--input",
            "a.json",
            "--input",
            "b.json",
        ])
        .is_err()
    );
    assert!(
        parse_remote_command(&["remote", "runner-dispatch-plan-preview", "--input", " "]).is_err()
    );
}

#[test]
fn remote_runner_dispatch_plan_preview_accepts_an_instance_projection() {
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "runner-dispatch-plan-preview",
            "--input",
            "request.json",
            "--instance",
            "client-web-001",
            "--instance-view",
            "client-view.json",
        ])
        .unwrap(),
        RemoteCommand::RunnerDispatchPlanPreview {
            input: "request.json".into(),
            instance_id: Some("client-web-001".into()),
            instance_view: Some("client-view.json".into()),
        }
    );
    assert!(
        parse_remote_command(&[
            "remote",
            "runner-dispatch-plan-preview",
            "--input",
            "request.json",
            "--instance-view",
            "client-view.json",
        ])
        .is_err()
    );
    assert!(
        parse_remote_command(&[
            "remote",
            "runner-dispatch-plan-preview",
            "--input",
            "request.json",
            "--instance",
            "client-web-001",
            "--instance",
            "client-cli-001",
        ])
        .is_err()
    );
}

#[test]
fn remote_runner_execution_intent_preview_requires_one_bounded_input() {
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "runner-execution-intent-preview",
            "--input",
            "request.json",
        ])
        .unwrap(),
        RemoteCommand::RunnerExecutionIntentPreview {
            input: "request.json".into(),
            instance_id: None,
            instance_view: None,
        }
    );
    assert!(parse_remote_command(&["remote", "runner-execution-intent-preview"]).is_err());
    assert!(
        parse_remote_command(&["remote", "runner-execution-intent-preview", "--input", " ",])
            .is_err()
    );
}

#[test]
fn remote_runner_execution_intent_preview_accepts_an_instance_projection() {
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "runner-execution-intent-preview",
            "--input",
            "request.json",
            "--instance",
            "client-web-001",
            "--instance-view",
            "client-view.json",
        ])
        .unwrap(),
        RemoteCommand::RunnerExecutionIntentPreview {
            input: "request.json".into(),
            instance_id: Some("client-web-001".into()),
            instance_view: Some("client-view.json".into()),
        }
    );
    assert!(
        parse_remote_command(&[
            "remote",
            "runner-execution-intent-preview",
            "--input",
            "request.json",
            "--instance-view",
            "client-view.json",
        ])
        .is_err()
    );
    assert!(
        parse_remote_command(&[
            "remote",
            "runner-execution-intent-preview",
            "--input",
            "request.json",
            "--instance",
            "client-web-001",
            "--instance",
            "client-cli-001",
        ])
        .is_err()
    );
}
