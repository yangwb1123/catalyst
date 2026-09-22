use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use forge_runtime_domain::execution::run_attempt_lease_dispatch_preflight::{
    RunAttemptLeaseDispatchPreflightObservation, decode,
};

use crate::args::DeviceCommand;
use crate::device_json_unique::reject_duplicate_keys;

const MAX_INPUT_BYTES: usize = 512 * 1024;

/// Reads one caller-supplied Run/Attempt/lease preflight value. This command
/// performs no network, storage, clock, lease, reservation, or dispatch work.
pub(crate) fn execute(
    command: &DeviceCommand,
) -> Result<RunAttemptLeaseDispatchPreflightObservation, Box<dyn Error>> {
    let DeviceCommand::RunAttemptLeaseDispatchPreflightPreview { input } = command else {
        return Err("device Run/Attempt/lease preflight command is required".into());
    };
    let bytes = read_bounded_input(input)?;
    reject_duplicate_keys(&bytes).map_err(|error| {
        format!("Run/Attempt/lease preflight input has duplicate keys: {error}")
    })?;
    decode(&bytes)
        .map_err(|error| format!("Run/Attempt/lease preflight input is invalid: {error}").into())
}

pub(crate) fn write_output(
    output: &RunAttemptLeaseDispatchPreflightObservation,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    writeln!(
        writer,
        "offline Run/Attempt/lease preflight [{}]",
        output.schema_version
    )?;
    writeln!(
        writer,
        "conversation={} run={} status={} run_state_admissible={}",
        output.conversation_id, output.run_id, output.run_status, output.run_state_admissible
    )?;
    writeln!(
        writer,
        "attempt={} state={} attempt_state_admissible={} command={} target={} lease_epoch={} lease_active={}",
        output.attempt_id,
        output.attempt_state,
        output.attempt_state_admissible,
        output.command_id,
        output.intent_target_id,
        output.lease_epoch,
        output.lease_active
    )?;
    writeln!(
        writer,
        "candidates={} ready={} declarative_preflight_ready={} selected_target_id=null evaluated_at_ms={}",
        output.candidate_count,
        output.declarative_ready_count,
        output.declarative_preflight_ready,
        output.evaluated_at_ms
    )?;
    writeln!(writer, "rejection_reasons={:?}", output.rejection_reasons)?;
    writeln!(
        writer,
        "preview_only=true authority: identity_verified=false run_authoritative=false attempt_persisted=false lease_issued=false reservation_created=false execution_authorized=false dispatch_performed=false audit_published=false"
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
        return Err(
            format!("Run/Attempt/lease preflight input exceeds {MAX_INPUT_BYTES} bytes").into(),
        );
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "device_run_attempt_lease_dispatch_preflight_command_tests.rs"]
mod tests;
