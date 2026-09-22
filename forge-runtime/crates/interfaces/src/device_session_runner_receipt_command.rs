use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use crate::args::DeviceCommand;
use crate::device_json_unique::reject_duplicate_keys;
use forge_runtime_domain::execution::session_runner_receipt::SessionRunnerReceiptObservation;

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;

/// Consumes one canonical session-bound terminal receipt envelope from a
/// caller-supplied file or stdin. This command performs no network or Hub
/// operation and never turns the observation into execution authority.
pub(crate) fn execute(
    command: &DeviceCommand,
) -> Result<SessionRunnerReceiptObservation, Box<dyn Error>> {
    let DeviceCommand::SessionRunnerReceiptPreview { input: input_path } = command else {
        return Err("device session Runner receipt preview command is required".into());
    };
    let bytes = read_bounded_input(input_path)?;
    reject_duplicate_keys(&bytes).map_err(|error| {
        format!("session Runner receipt input contains duplicate JSON keys: {error}")
    })?;
    let observation: SessionRunnerReceiptObservation = serde_json::from_slice(&bytes)
        .map_err(|error| format!("session Runner receipt input is invalid JSON: {error}"))?;
    observation
        .validate()
        .map_err(|error| format!("session Runner receipt observation rejected: {error}"))?;
    Ok(observation)
}

pub(crate) fn write_output(
    output: &SessionRunnerReceiptObservation,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    writeln!(
        writer,
        "offline session Runner terminal receipt observation [{}]",
        output.schema_version
    )?;
    writeln!(
        writer,
        "owner={} conversation={} prompt={} run={}",
        output.owner.subject, output.conversation_id, output.prompt_id, output.run_id
    )?;
    let receipt = &output.receipt_observation;
    writeln!(
        writer,
        "receipt_command={} attempt={} target={} disposition={} observed_at_ms={} receipt_valid={} uncertain={}",
        receipt.command_id,
        receipt.attempt_id,
        receipt.target_id,
        receipt.disposition_kind,
        receipt.observed_at_ms,
        receipt.receipt_valid,
        receipt.uncertain
    )?;
    writeln!(
        writer,
        "binding: prompt_run_binding_valid={} receipt_binding_valid={} preview_only={} selected_target=none",
        output.prompt_run_binding_valid, output.receipt_binding_valid, output.preview_only
    )?;
    writeln!(
        writer,
        "follow_up={} reconciliation_required={} manual_review_required={} automatic_retry={}",
        receipt.follow_up,
        receipt.reconciliation_required,
        receipt.manual_review_required,
        receipt.automatic_retry
    )?;
    writeln!(
        writer,
        "authority: identity_verified=false receipt_persisted=false execution_authorized=false dispatch_performed=false audit_published=false"
    )
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
        return Err(format!("session Runner receipt input exceeds {MAX_INPUT_BYTES} bytes").into());
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "device_session_runner_receipt_command_tests.rs"]
mod tests;
