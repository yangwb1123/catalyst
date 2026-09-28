use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use forge_runtime_domain::execution::session_runner_receipt_history::SessionRunnerReceiptHistoryObservation;

use crate::args::DeviceCommand;
use crate::device_json_unique::reject_duplicate_keys;

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;

/// Consumes one canonical session-bound Runner receipt history from a
/// caller-supplied file or stdin. This is a local, read-only reduction and
/// never contacts a Hub, persists a receipt, retries an attempt, or requests
/// a device.
pub(crate) fn execute(
    command: &DeviceCommand,
) -> Result<SessionRunnerReceiptHistoryObservation, Box<dyn Error>> {
    let DeviceCommand::SessionRunnerReceiptHistoryPreview { input: input_path } = command else {
        return Err("device session Runner receipt history preview command is required".into());
    };
    let bytes = read_bounded_input(input_path)?;
    reject_duplicate_keys(&bytes).map_err(|error| {
        format!("session Runner receipt history input contains duplicate JSON keys: {error}")
    })?;
    let observation: SessionRunnerReceiptHistoryObservation = serde_json::from_slice(&bytes)
        .map_err(|error| {
            format!("session Runner receipt history input is invalid JSON: {error}")
        })?;
    observation
        .validate()
        .map_err(|error| format!("session Runner receipt history rejected: {error}"))?;
    Ok(observation)
}

pub(crate) fn write_output(
    output: &SessionRunnerReceiptHistoryObservation,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }

    writeln!(
        writer,
        "offline session Runner receipt history [{}]",
        output.schema_version
    )?;
    writeln!(
        writer,
        "owner={} conversation={} prompt={} run={}",
        output.owner.subject, output.conversation_id, output.prompt_id, output.run_id
    )?;
    writeln!(writer, "attempt_count={}", output.attempt_count)?;
    write_attempts(output, writer)?;
    write_latest(output, writer)
}

fn write_latest(
    output: &SessionRunnerReceiptHistoryObservation,
    writer: &mut impl Write,
) -> io::Result<()> {
    writeln!(
        writer,
        "latest: command={} attempt={} target={} disposition={} observed_at_ms={}",
        output.latest_command_id,
        output.latest_attempt_id,
        output.latest_target_id,
        output.latest_disposition_kind,
        output.latest_observed_at_ms
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
        "binding: preview_only={} selected_target=none",
        output.preview_only
    )?;
    writeln!(
        writer,
        "authority: identity_verified={} receipt_persisted={} execution_authorized={} dispatch_performed={} audit_published={}",
        output.authority.identity_verified,
        output.authority.receipt_persisted,
        output.authority.execution_authorized,
        output.authority.dispatch_performed,
        output.authority.audit_published
    )
}

fn write_attempts(
    output: &SessionRunnerReceiptHistoryObservation,
    writer: &mut impl Write,
) -> io::Result<()> {
    for (index, receipt) in output.receipts.iter().enumerate() {
        let terminal = &receipt.receipt_observation;
        writeln!(
            writer,
            "attempt[{}]: command={} attempt={} target={} disposition={} observed_at_ms={} uncertain={}",
            index + 1,
            terminal.command_id,
            terminal.attempt_id,
            terminal.target_id,
            terminal.disposition_kind,
            terminal.observed_at_ms,
            terminal.uncertain
        )?;
    }
    Ok(())
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
        return Err(format!(
            "session Runner receipt history input exceeds {MAX_INPUT_BYTES} bytes"
        )
        .into());
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "device_session_runner_receipt_history_command_tests.rs"]
mod tests;
