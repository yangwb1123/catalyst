use crate::args::{DeviceCommand, DeviceInventoryCommand};
use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
    process::ExitCode,
};
#[path = "device_inventory_placement_evaluation_v2_command/evaluation.rs"]
mod evaluation;
#[path = "device_inventory_placement_evaluation_v2_command/validation.rs"]
mod validation;
#[path = "device_inventory_placement_evaluation_v2_command/wire.rs"]
mod wire;

use evaluation::evaluate;
use wire::Fixture;
pub(crate) use wire::Output;

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_DECISIONS: usize = 128;
// The v2 observation wire contract is narrower than the legacy capability
// model. Keep the CLI's preflight bound aligned with Go, Flutter, and the
// domain evaluator before attempting the pure comparison.
const MAX_V2_RUNTIME_COUNT: usize = 32;
const SCHEMA_VERSION: &str = "forge.device-inventory-placement-evaluation/v2";
const EVALUATION_MODE: &str = "offline_static_only";
const SOURCE_SCHEMA_VERSION: &str = "forge.device-inventory-observation/v2";
const NOTICE: &str = "Every owner, state, timestamp, resource, GPU, reservation, residency, trust, sandbox, and concurrency value is an unverified caller declaration. This read-only comparison selects no target and grants no execution authority.";

pub(crate) fn execute(command: &DeviceCommand) -> Result<Output, Box<dyn Error>> {
    let DeviceCommand::Inventory(DeviceInventoryCommand::PlacementEvaluationV2 { input }) = command
    else {
        return Err("device inventory placement-evaluation-v2 command is required".into());
    };
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes).map_err(|error| {
        format!("device inventory placement-evaluation-v2 input has duplicate JSON keys: {error}")
    })?;
    let fixture: Fixture = serde_json::from_slice(&bytes).map_err(|error| {
        format!("device inventory placement-evaluation-v2 input is invalid JSON: {error}")
    })?;
    evaluate(fixture)
}

pub(crate) fn run(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device inventory placement-evaluation-v2 failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = write_output(&output, json, &mut io::stdout().lock()) {
        eprintln!("failed to write device inventory placement-evaluation-v2 output: {error}");
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
        "offline placement evaluation [{}] decisions={} eligible={} selected=none",
        output.evaluation_mode,
        output.decisions.len(),
        output.eligible_candidate_count
    )?;
    for decision in &output.decisions {
        writeln!(
            writer,
            "{} / {}: revision={} generation={} heartbeat={} reservation={} gpus={} available_gpu_memory={} matches={} reasons={}",
            decision.device_id,
            decision.instance_id,
            decision.revision,
            decision.generation,
            decision.heartbeat_sequence,
            decision.reservation_state,
            decision.gpu_count,
            decision.available_gpu_memory_bytes,
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
        return Err(format!(
            "device inventory placement-evaluation-v2 input exceeds {MAX_INPUT_BYTES} bytes"
        )
        .into());
    }
    Ok(bytes)
}
