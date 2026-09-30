use crate::args::{DeviceCommand, DeviceInventoryCommand};
use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
    process::ExitCode,
};
#[path = "device_inventory_persistence_command/evaluation.rs"]
mod evaluation;
#[path = "device_inventory_persistence_command/state.rs"]
mod state;
#[path = "device_inventory_persistence_command/wire.rs"]
mod wire;

use evaluation::evaluate;
pub(crate) use wire::PersistenceOutput;
use wire::{CaseOutput, Fixture};

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_CASES: usize = 128;
const MAX_CASE_NAME_BYTES: usize = 128;
const SCHEMA_VERSION: &str = "forge.device-inventory-persistence/v1";
const EVALUATION_MODE: &str = "pure_persisted_inventory_cas_projection";

pub(crate) fn execute(command: &DeviceCommand) -> Result<PersistenceOutput, Box<dyn Error>> {
    let DeviceCommand::Inventory(DeviceInventoryCommand::PersistencePreview { input }) = command
    else {
        return Err("device inventory persistence preview command is required".into());
    };
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes).map_err(|error| {
        format!("device inventory persistence input has duplicate JSON keys: {error}")
    })?;
    let fixture: Fixture = serde_json::from_slice(&bytes)
        .map_err(|error| format!("device inventory persistence input is invalid JSON: {error}"))?;
    evaluate(fixture)
}

pub(crate) fn run(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device inventory persistence preview failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = write_output(&output, json, &mut io::stdout().lock()) {
        eprintln!("failed to write device inventory persistence preview output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

pub(crate) fn write_output(
    output: &PersistenceOutput,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    writeln!(
        writer,
        "offline device inventory persistence preview [{}] revision={} device={} instance={}",
        output.schema_version,
        output.state.revision,
        output.state.device_id,
        output.state.instance_id
    )?;
    writeln!(
        writer,
        "evaluation: {} (pure CAS/projection model; no storage write, clock read, network, or authority)",
        output.evaluation_mode
    )?;
    for case in &output.cases {
        write_case(case, writer)?;
    }
    writeln!(
        writer,
        "authority: identity_verified=false heartbeat_persisted=false inventory_authoritative=false reservation_created=false execution_authorized=false dispatch_performed=false"
    )
}

fn write_case(case: &CaseOutput, writer: &mut impl Write) -> io::Result<()> {
    if let Some(error) = &case.error {
        writeln!(writer, "{}: accepted=false error={error}", case.name)?;
    } else if case.operation == "project" {
        writeln!(
            writer,
            "{}: accepted=true revision={} status={} fresh={} declared_eligible={}",
            case.name,
            case.revision.unwrap_or_default(),
            case.status.as_deref().unwrap_or("unknown"),
            case.fresh.unwrap_or(false),
            case.declared_eligible.unwrap_or(false)
        )?;
    } else {
        writeln!(
            writer,
            "{}: accepted=true revision={} sequence={} observed_at_ms={} lease_expires_at_ms={}",
            case.name,
            case.revision.unwrap_or_default(),
            case.heartbeat_sequence.unwrap_or_default(),
            case.server_observed_at_ms.unwrap_or_default(),
            case.capability_lease_expires_at_ms.unwrap_or_default()
        )?;
    }
    Ok(())
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::with_capacity(MAX_INPUT_BYTES.min(64 * 1024));
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
            format!("device inventory persistence input exceeds {MAX_INPUT_BYTES} bytes").into(),
        );
    }
    Ok(bytes)
}
