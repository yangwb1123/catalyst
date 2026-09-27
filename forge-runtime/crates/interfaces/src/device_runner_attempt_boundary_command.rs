//! Bounded CLI consumer for the pure Runner Attempt lifecycle boundary.
//!
//! This command only decodes and revalidates an observation. It does not
//! persist an Attempt, inspect or mutate a lease, contact a Runner, or grant
//! dispatch authority.

use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use forge_runtime_domain::execution::runner_attempt_boundary::{
    RunnerAttemptBoundaryObservation, decode,
};

use crate::{args::DeviceCommand, device_json_unique::reject_duplicate_keys};

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;

pub(crate) fn execute(
    command: &DeviceCommand,
) -> Result<RunnerAttemptBoundaryObservation, Box<dyn Error>> {
    let DeviceCommand::RunnerAttemptBoundaryPreview { input: input_path } = command else {
        return Err("device Runner Attempt boundary preview command is required".into());
    };
    let bytes = read_bounded_input(input_path)?;
    reject_duplicate_keys(&bytes).map_err(|error| {
        format!("Runner Attempt boundary input contains duplicate JSON keys: {error}")
    })?;
    let observation = decode(&bytes)
        .map_err(|error| format!("Runner Attempt boundary input is invalid JSON: {error}"))?;
    observation
        .validate()
        .map_err(|error| format!("Runner Attempt boundary observation is invalid: {error}"))?;
    Ok(observation)
}

pub(crate) fn write_output(
    output: &RunnerAttemptBoundaryObservation,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    writeln!(
        writer,
        "offline Runner Attempt boundary preview [{}]",
        output.schema_version
    )?;
    writeln!(
        writer,
        "owner={}/{}/{} conversation={} run={} attempt={} command={} target={} lease_epoch={}",
        output.owner.issuer,
        output.owner.subject,
        output.owner.tenant_id,
        output.conversation_id,
        output.run_id,
        output.attempt_id,
        output.command_id,
        output.target_id,
        output.lease_epoch
    )?;
    writeln!(
        writer,
        "lifecycle: {} -> {} transition={} execution_boundary_ready={} transition_valid={} transition_dispatchable={} attempt_boundary_ready={}",
        output.current_attempt_state,
        output.next_attempt_state,
        output.transition,
        output.execution_boundary_ready,
        output.attempt_transition_valid,
        output.attempt_transition_dispatchable,
        output.attempt_boundary_ready
    )?;
    writeln!(
        writer,
        "rejection_reasons={}",
        if output.rejection_reasons.is_empty() {
            "none".to_owned()
        } else {
            output.rejection_reasons.join(",")
        }
    )?;
    writeln!(
        writer,
        "preview_only=true authority: attempt_persisted=false reservation_created=false execution_authorized=false dispatch_performed=false audit_published=false"
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
            format!("Runner Attempt boundary input exceeds {MAX_INPUT_BYTES} bytes").into(),
        );
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "device_runner_attempt_boundary_command_tests.rs"]
mod tests;
