use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use forge_runtime_domain::run_execution_evidence::RunExecutionEvidence;

use crate::args::DeviceCommand;
use crate::device_json_unique::reject_duplicate_keys;

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;

/// Consumes one canonical, content-free Run execution-evidence projection
/// from a caller-supplied file or stdin. This command has no network, storage,
/// target-selection, or process-execution effect.
pub(crate) fn execute(command: &DeviceCommand) -> Result<RunExecutionEvidence, Box<dyn Error>> {
    let DeviceCommand::RunExecutionEvidencePreview { input: input_path } = command else {
        return Err("device Run execution-evidence preview command is required".into());
    };
    let bytes = read_bounded_input(input_path)?;
    reject_duplicate_keys(&bytes).map_err(|error| {
        format!("Run execution-evidence input contains duplicate JSON keys: {error}")
    })?;
    let observation: RunExecutionEvidence = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Run execution-evidence input is invalid JSON: {error}"))?;
    observation
        .validate()
        .map_err(|error| format!("Run execution-evidence observation rejected: {error}"))?;
    Ok(observation)
}

pub(crate) fn write_output(
    output: &RunExecutionEvidence,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    writeln!(
        writer,
        "offline Run execution-evidence preview [{}]",
        output.api_version
    )?;
    writeln!(
        writer,
        "owner_ref={} conversation={} prompt={} run={} status={}",
        output.owner_ref,
        output.conversation_id,
        output.prompt_id,
        output.run_id,
        output.run_status
    )?;
    writeln!(
        writer,
        "receipt: attempt={} target={} command={} digest={} disposition={} observed_at_ms={}",
        output.attempt_id,
        output.target_id,
        output.command_id,
        output.command_sha256,
        output.disposition_kind,
        output.receipt_observed_at_ms
    )?;
    writeln!(
        writer,
        "metadata_only: metadata_observed={} content_included={} uncertain={} reconciliation_required={}",
        output.metadata_observed,
        output.content_included,
        output.uncertain,
        output.reconciliation_required
    )?;
    writeln!(
        writer,
        "authority: identity_verified=false owner_authorized=false run_authoritative=false receipt_persisted=false reservation_created=false execution_authorized=false dispatch_performed=false audit_published=false"
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
        return Err(format!("Run execution-evidence input exceeds {MAX_INPUT_BYTES} bytes").into());
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "device_run_execution_evidence_command_tests.rs"]
mod tests;
