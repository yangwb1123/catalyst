use super::{
    RUNNER_EXECUTION_INTENT_EVALUATION_MODE, RUNNER_EXECUTION_INTENT_SCHEMA_VERSION,
    RunnerExecutionIntentAuthority, RunnerExecutionIntentRequest, observe_runner_execution_intent,
};
use serde::Deserialize;

const FIXTURE: &str =
    include_str!("../../../../../docs/contracts/fixtures/forge-runner-execution-intent-v1.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    authority: RunnerExecutionIntentAuthority,
    owner: super::RunnerExecutionOwner,
    conversation_id: String,
    prompt_receipt: super::RunnerExecutionPromptReceipt,
    run_reference: super::RunnerExecutionRunReference,
    execution_intent: super::RunnerExecutionIntentBinding,
    command: super::super::runner_command::RunnerCommand,
    expected: Expected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    prompt_run_binding_valid: bool,
    runner_command_binding_valid: bool,
    preview_only: bool,
    command_sha256: String,
    selected_target_id: Option<String>,
    authority: RunnerExecutionIntentAuthority,
}

#[test]
fn runner_execution_intent_contract_fixture() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).expect("strict fixture");
    assert_eq!(
        fixture.schema_version,
        RUNNER_EXECUTION_INTENT_SCHEMA_VERSION
    );
    assert_eq!(
        fixture.evaluation_mode,
        RUNNER_EXECUTION_INTENT_EVALUATION_MODE
    );
    assert!(authority_is_false(fixture.authority));
    assert!(authority_is_false(fixture.expected.authority));
    let request = RunnerExecutionIntentRequest {
        owner: fixture.owner,
        conversation_id: fixture.conversation_id,
        prompt_receipt: fixture.prompt_receipt,
        run_reference: fixture.run_reference,
        execution_intent: fixture.execution_intent,
        command: fixture.command,
    };
    assert_eq!(
        request.command.command_sha256().expect("command digest"),
        fixture.expected.command_sha256
    );
    let observation = observe_runner_execution_intent(request).expect("binding");
    assert!(observation.prompt_run_binding_valid == fixture.expected.prompt_run_binding_valid);
    assert!(
        observation.runner_command_binding_valid == fixture.expected.runner_command_binding_valid
    );
    assert!(observation.preview_only == fixture.expected.preview_only);
    assert_eq!(observation.command_sha256, fixture.expected.command_sha256);
    assert_eq!(
        observation.selected_target_id,
        fixture.expected.selected_target_id
    );
    assert!(authority_is_false(observation.authority));
}

#[test]
fn confused_prompt_run_or_target_bindings_fail_closed() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).expect("strict fixture");
    let request = || RunnerExecutionIntentRequest {
        owner: fixture.owner.clone(),
        conversation_id: fixture.conversation_id.clone(),
        prompt_receipt: fixture.prompt_receipt.clone(),
        run_reference: fixture.run_reference.clone(),
        execution_intent: fixture.execution_intent.clone(),
        command: fixture.command.clone(),
    };
    let mut foreign_run = request();
    foreign_run.execution_intent.run_id = "run-foreign".into();
    assert!(observe_runner_execution_intent(foreign_run).is_err());

    let mut foreign_target = request();
    foreign_target.command.lease_proof.target_id = "runner-foreign".into();
    assert!(observe_runner_execution_intent(foreign_target).is_err());
}

fn authority_is_false(authority: RunnerExecutionIntentAuthority) -> bool {
    !authority.device_identity_verified
        && !authority.command_persisted
        && !authority.reservation_created
        && !authority.execution_authorized
        && !authority.dispatch_performed
        && !authority.audit_published
}
