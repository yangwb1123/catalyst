use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use forge_runtime_domain::execution::{
    lease::LeaseGrant,
    runner_command::{
        RUNNER_TERMINAL_RECEIPT_EVALUATION_MODE, RUNNER_TERMINAL_RECEIPT_SCHEMA_VERSION,
        RunnerCommand, RunnerTerminalReceipt, RunnerTerminalReceiptAuthority,
        RunnerTerminalReceiptObservation, observe_runner_terminal_receipt,
    },
};
use serde::{Deserialize, Serialize};

use crate::args::DeviceCommand;
use crate::device_json_unique::reject_duplicate_keys;

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptFixture {
    schema_version: String,
    evaluation_mode: String,
    authority: RunnerTerminalReceiptAuthority,
    grant: LeaseGrant,
    command: RunnerCommand,
    receipt: RunnerTerminalReceipt,
    expected: ReceiptExpected,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptExpected {
    command_sha256: String,
    receipt_valid: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct RunnerReceiptPreviewOutput {
    schema_version: &'static str,
    evaluation_mode: &'static str,
    command_id: String,
    command_sha256: String,
    attempt_id: String,
    target_id: String,
    disposition_kind: String,
    observed_at_ms: u64,
    receipt_valid: bool,
    preview_only: bool,
    uncertain: bool,
    reconciliation_required: bool,
    manual_review_required: bool,
    automatic_retry: bool,
    follow_up: String,
    authority: RunnerTerminalReceiptAuthority,
}

pub(crate) fn execute(
    command: &DeviceCommand,
) -> Result<RunnerReceiptPreviewOutput, Box<dyn Error>> {
    let DeviceCommand::RunnerReceiptPreview { input: input_path } = command else {
        return Err("device Runner terminal receipt preview command is required".into());
    };
    let bytes = read_bounded_input(input_path)?;
    reject_duplicate_keys(&bytes).map_err(|error| {
        format!("Runner terminal receipt input contains duplicate JSON keys: {error}")
    })?;
    let fixture: ReceiptFixture = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Runner terminal receipt input is invalid JSON: {error}"))?;
    validate_fixture(&fixture)?;
    let observation =
        observe_runner_terminal_receipt(&fixture.command, &fixture.grant, &fixture.receipt)?;
    if !fixture.expected.receipt_valid
        || fixture.expected.command_sha256 != observation.command_sha256
    {
        return Err("Runner terminal receipt fixture expectation mismatch".into());
    }
    Ok(RunnerReceiptPreviewOutput::from_observation(observation))
}

pub(crate) fn write_output(
    output: &RunnerReceiptPreviewOutput,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    writeln!(
        writer,
        "offline Runner terminal receipt preview [{}]",
        output.schema_version
    )?;
    writeln!(
        writer,
        "command={} attempt={} target={} disposition={} observed_at_ms={} receipt_valid={} uncertain={}",
        output.command_id,
        output.attempt_id,
        output.target_id,
        output.disposition_kind,
        output.observed_at_ms,
        output.receipt_valid,
        output.uncertain
    )?;
    writeln!(
        writer,
        "follow_up={} reconciliation_required={} manual_review_required={} automatic_retry={}",
        output.follow_up,
        output.reconciliation_required,
        output.manual_review_required,
        output.automatic_retry
    )?;
    writeln!(
        writer,
        "authority: device_identity_verified=false command_persisted=false reservation_created=false execution_authorized=false dispatch_performed=false audit_published=false"
    )
}

fn validate_fixture(fixture: &ReceiptFixture) -> Result<(), Box<dyn Error>> {
    if fixture.schema_version != RUNNER_TERMINAL_RECEIPT_SCHEMA_VERSION
        || fixture.evaluation_mode != RUNNER_TERMINAL_RECEIPT_EVALUATION_MODE
        || !authority_is_false(fixture.authority)
    {
        return Err("Runner terminal receipt input is not a pure read-only observation".into());
    }
    let command_sha256 = fixture.command.command_sha256()?;
    if fixture.expected.command_sha256 != command_sha256
        || fixture.receipt.command_sha256 != command_sha256
    {
        return Err("Runner terminal receipt command digest expectation mismatch".into());
    }
    Ok(())
}

fn authority_is_false(authority: RunnerTerminalReceiptAuthority) -> bool {
    !authority.device_identity_verified
        && !authority.command_persisted
        && !authority.reservation_created
        && !authority.execution_authorized
        && !authority.dispatch_performed
        && !authority.audit_published
}

impl RunnerReceiptPreviewOutput {
    fn from_observation(value: RunnerTerminalReceiptObservation) -> Self {
        Self {
            schema_version: value.schema_version,
            evaluation_mode: value.evaluation_mode,
            command_id: value.command_id,
            command_sha256: value.command_sha256,
            attempt_id: value.attempt_id,
            target_id: value.target_id,
            disposition_kind: value.disposition_kind,
            observed_at_ms: value.observed_at_ms,
            receipt_valid: value.receipt_valid,
            preview_only: value.preview_only,
            uncertain: value.uncertain,
            reconciliation_required: value.reconciliation_required,
            manual_review_required: value.manual_review_required,
            automatic_retry: value.automatic_retry,
            follow_up: value.follow_up,
            authority: value.authority,
        }
    }
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
            format!("Runner terminal receipt input exceeds {MAX_INPUT_BYTES} bytes").into(),
        );
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "device_runner_receipt_command_tests.rs"]
mod tests;
