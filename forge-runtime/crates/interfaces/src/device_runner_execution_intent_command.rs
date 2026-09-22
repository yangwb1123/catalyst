use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use forge_runtime_domain::execution::runner_command::RunnerCommand;
use forge_runtime_domain::execution::runner_execution_intent::{
    RUNNER_EXECUTION_INTENT_EVALUATION_MODE, RUNNER_EXECUTION_INTENT_SCHEMA_VERSION,
    RunnerExecutionIntentAuthority, RunnerExecutionIntentBinding, RunnerExecutionIntentObservation,
    RunnerExecutionIntentRequest, RunnerExecutionOwner, RunnerExecutionPromptReceipt,
    RunnerExecutionRunReference, observe_runner_execution_intent,
};
use serde::Deserialize;

use crate::args::DeviceCommand;

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IntentFixture {
    schema_version: String,
    evaluation_mode: String,
    authority: RunnerExecutionIntentAuthority,
    owner: RunnerExecutionOwner,
    conversation_id: String,
    prompt_receipt: RunnerExecutionPromptReceipt,
    run_reference: RunnerExecutionRunReference,
    execution_intent: RunnerExecutionIntentBinding,
    command: RunnerCommand,
    expected: IntentExpected,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IntentExpected {
    prompt_run_binding_valid: bool,
    runner_command_binding_valid: bool,
    preview_only: bool,
    command_sha256: String,
    selected_target_id: Option<String>,
    authority: RunnerExecutionIntentAuthority,
}

pub(crate) fn execute(
    command: &DeviceCommand,
) -> Result<RunnerExecutionIntentObservation, Box<dyn Error>> {
    let DeviceCommand::RunnerExecutionIntentPreview { input: input_path } = command else {
        return Err("device Runner execution intent preview command is required".into());
    };
    let fixture: IntentFixture = serde_json::from_slice(&read_bounded_input(input_path)?)
        .map_err(|error| format!("Runner execution intent input is invalid JSON: {error}"))?;
    validate_fixture(&fixture)?;
    let observation = observe_runner_execution_intent(RunnerExecutionIntentRequest {
        owner: fixture.owner,
        conversation_id: fixture.conversation_id,
        prompt_receipt: fixture.prompt_receipt,
        run_reference: fixture.run_reference,
        execution_intent: fixture.execution_intent,
        command: fixture.command,
    })?;
    validate_expected(&observation, &fixture.expected)?;
    Ok(observation)
}

pub(crate) fn write_output(
    output: &RunnerExecutionIntentObservation,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    writeln!(
        writer,
        "offline Runner execution intent preview [{}]",
        output.schema_version
    )?;
    writeln!(
        writer,
        "owner={} conversation={} prompt={} run={} attempt={} command={} target={}",
        output.owner.subject,
        output.conversation_id,
        output.prompt_id,
        output.run_id,
        output.attempt_id,
        output.command_id,
        output.target_id
    )?;
    writeln!(
        writer,
        "binding: prompt_run_binding_valid={} runner_command_binding_valid={} preview_only={} selected_target=none",
        output.prompt_run_binding_valid, output.runner_command_binding_valid, output.preview_only
    )?;
    writeln!(
        writer,
        "authority: device_identity_verified=false command_persisted=false reservation_created=false execution_authorized=false dispatch_performed=false audit_published=false"
    )
}

fn validate_fixture(fixture: &IntentFixture) -> Result<(), Box<dyn Error>> {
    if fixture.schema_version != RUNNER_EXECUTION_INTENT_SCHEMA_VERSION
        || fixture.evaluation_mode != RUNNER_EXECUTION_INTENT_EVALUATION_MODE
        || !authority_is_false(fixture.authority)
    {
        return Err("Runner execution intent input is not a pure read-only observation".into());
    }
    if fixture.expected.command_sha256.len() != 64
        || !fixture
            .expected
            .command_sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err("Runner execution intent expected digest is invalid".into());
    }
    Ok(())
}

fn validate_expected(
    actual: &RunnerExecutionIntentObservation,
    expected: &IntentExpected,
) -> Result<(), Box<dyn Error>> {
    if actual.prompt_run_binding_valid != expected.prompt_run_binding_valid
        || actual.runner_command_binding_valid != expected.runner_command_binding_valid
        || actual.preview_only != expected.preview_only
        || actual.command_sha256 != expected.command_sha256
        || actual.selected_target_id != expected.selected_target_id
        || actual.authority != expected.authority
    {
        return Err("Runner execution intent fixture expectation mismatch".into());
    }
    Ok(())
}

fn authority_is_false(authority: RunnerExecutionIntentAuthority) -> bool {
    !authority.device_identity_verified
        && !authority.command_persisted
        && !authority.reservation_created
        && !authority.execution_authorized
        && !authority.dispatch_performed
        && !authority.audit_published
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::new();
    if input == "-" {
        io::stdin()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
    } else {
        File::open(Path::new(input))?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(
            format!("Runner execution intent input exceeds {MAX_INPUT_BYTES} bytes").into(),
        );
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "device_runner_execution_intent_command_tests.rs"]
mod tests;
