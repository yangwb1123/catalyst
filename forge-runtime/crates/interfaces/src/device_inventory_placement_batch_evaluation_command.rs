use crate::args::{DeviceCommand, DeviceInventoryCommand};
use std::{
    error::Error,
    fs,
    io::{self, Read, Write},
    path::Path,
    process::ExitCode,
};
#[path = "device_inventory_placement_batch_evaluation_command/evaluation.rs"]
mod evaluation;
#[path = "device_inventory_placement_batch_evaluation_command/input.rs"]
mod input;
#[path = "device_inventory_placement_batch_evaluation_command/validation.rs"]
mod validation;
#[path = "device_inventory_placement_batch_evaluation_command/wire.rs"]
mod wire;

use evaluation::evaluate;
pub(crate) use wire::Output;
use wire::{Fixture, SourceFixture};

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const BATCH_SCHEMA: &str = "forge.device-inventory-placement-batch-evaluation/v1";
const SOURCE_FIXTURE: &str = "forge-device-inventory-placement-input-v1.json";
const BATCH_MODE: &str = "pure_persisted_inventory_placement_dry_run";
const SOURCE_FIXTURE_TEXT: &str = include_str!(
    "../../../../docs/contracts/fixtures/forge-device-inventory-placement-input-v1.json"
);

pub(crate) fn execute(command: &DeviceCommand) -> Result<Output, Box<dyn Error>> {
    let DeviceCommand::Inventory(DeviceInventoryCommand::PlacementBatchEvaluation { input }) =
        command
    else {
        return Err("device inventory placement-batch-evaluation command is required".into());
    };
    let (fixture_bytes, source_bytes) = if input == "-" {
        (
            read_bounded_input(input)?,
            SOURCE_FIXTURE_TEXT.as_bytes().to_vec(),
        )
    } else {
        let bytes = read_bounded_input(input)?;
        let fixture: Fixture = serde_json::from_slice(&bytes)
            .map_err(|e| format!("placement batch input is invalid JSON: {e}"))?;
        if fixture.source_fixture != SOURCE_FIXTURE {
            return Err("placement batch source_fixture is not the canonical fixture".into());
        }
        let source_path = Path::new(input)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(SOURCE_FIXTURE);
        (
            bytes,
            fs::read(source_path)
                .map_err(|e| format!("placement batch source fixture is unavailable: {e}"))?,
        )
    };
    let fixture: Fixture = serde_json::from_slice(&fixture_bytes)
        .map_err(|e| format!("placement batch input is invalid JSON: {e}"))?;
    crate::device_json_unique::reject_duplicate_keys(&fixture_bytes)
        .map_err(|e| format!("placement batch input has duplicate JSON keys: {e}"))?;
    crate::device_json_unique::reject_duplicate_keys(&source_bytes)
        .map_err(|e| format!("placement batch source has duplicate JSON keys: {e}"))?;
    if source_bytes != SOURCE_FIXTURE_TEXT.as_bytes() {
        return Err("placement batch source fixture drifted from the canonical fixture".into());
    }
    let source: SourceFixture = serde_json::from_slice(&source_bytes)
        .map_err(|e| format!("placement batch source fixture is invalid: {e}"))?;
    evaluate(fixture, &source)
}

pub(crate) fn run(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device inventory placement batch evaluation failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = write_output(&output, json, &mut io::stdout().lock()) {
        eprintln!("failed to write device inventory placement batch evaluation output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

pub(crate) fn write_output(output: &Output, json: bool, writer: &mut impl Write) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    writeln!(
        writer,
        "offline placement batch evaluation [{}] decisions={} selected=none",
        output.evaluation_mode,
        output.decisions.len()
    )?;
    for decision in &output.decisions {
        writeln!(
            writer,
            "{} / {}: matches={} reasons={}",
            decision.device_id,
            decision.instance_id,
            decision.matches_requirements,
            decision.exclusion_reasons.join(",")
        )?;
    }
    writeln!(
        writer,
        "authority: identity_verified=false heartbeat_persisted=false inventory_authoritative=false placement_selected=false reservation_created=false execution_authorized=false dispatch_performed=false"
    )
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::new();
    if input == "-" {
        io::stdin()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
    } else {
        fs::File::open(input)?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err("placement batch input exceeds 2 MiB".into());
    }
    Ok(bytes)
}
