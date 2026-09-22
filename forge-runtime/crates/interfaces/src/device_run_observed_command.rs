use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use forge_runtime_domain::run_observed::RunObserved;

use crate::args::DeviceCommand;
use crate::device_json_unique::reject_duplicate_keys;

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;

/// Consumes one canonical, content-free Run observation from a caller-supplied
/// file or stdin. This command has no network, storage, target-selection, or
/// process-execution effect.
pub(crate) fn execute(command: &DeviceCommand) -> Result<RunObserved, Box<dyn Error>> {
    let DeviceCommand::RunObservedPreview { input: input_path } = command else {
        return Err("device Run observed preview command is required".into());
    };
    let bytes = read_bounded_input(input_path)?;
    reject_duplicate_keys(&bytes)
        .map_err(|error| format!("Run observed input contains duplicate JSON keys: {error}"))?;
    let observation: RunObserved = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Run observed input is invalid JSON: {error}"))?;
    observation
        .validate()
        .map_err(|error| format!("Run observed observation rejected: {error}"))?;
    Ok(observation)
}

pub(crate) fn write_output(
    output: &RunObserved,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    writeln!(
        writer,
        "offline Run observed preview [{}]",
        output.api_version
    )?;
    writeln!(
        writer,
        "owner_ref={} conversation={} prompt={} run={} status={} latest_sequence={}",
        output.owner_ref,
        output.conversation_id,
        output.prompt_id,
        output.run_id,
        output.status,
        output.latest_sequence
    )?;
    writeln!(
        writer,
        "metadata_only: metadata_observed={} content_included={}",
        output.metadata_observed, output.content_included
    )?;
    writeln!(
        writer,
        "authority: identity_verified=false owner_authorized=false run_authoritative=false persistence_attested=false content_provenance_verified=false reservation_created=false execution_authorized=false dispatch_performed=false"
    )
}

/// Renders the same validated metadata-only projection after it has arrived
/// through the authenticated owner-bound remote candidate.
pub(crate) fn write_remote_output(output: &RunObserved, writer: &mut impl Write) -> io::Result<()> {
    writeln!(writer, "remote Run observed [{}]", output.api_version)?;
    writeln!(
        writer,
        "owner_ref={} conversation={} prompt={} run={} status={} latest_sequence={}",
        output.owner_ref,
        output.conversation_id,
        output.prompt_id,
        output.run_id,
        output.status,
        output.latest_sequence
    )?;
    writeln!(
        writer,
        "metadata_only: metadata_observed={} content_included={}",
        output.metadata_observed, output.content_included
    )?;
    writeln!(
        writer,
        "authority: identity_verified={} owner_authorized={} run_authoritative={} persistence_attested={} content_provenance_verified={} reservation_created={} execution_authorized={} dispatch_performed={}",
        output.authority.identity_verified,
        output.authority.owner_authorized,
        output.authority.run_authoritative,
        output.authority.persistence_attested,
        output.authority.content_provenance_verified,
        output.authority.reservation_created,
        output.authority.execution_authorized,
        output.authority.dispatch_performed,
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
        return Err(format!("Run observed input exceeds {MAX_INPUT_BYTES} bytes").into());
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "device_run_observed_command_tests.rs"]
mod tests;
