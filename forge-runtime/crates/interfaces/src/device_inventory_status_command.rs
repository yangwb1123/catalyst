use std::{
    collections::HashSet,
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
    process::ExitCode,
};

use forge_runtime_domain::{
    InventoryStatusObservation, InventoryStatusProjection, project_inventory_status,
};
use serde::{Deserialize, Serialize};

use crate::args::{DeviceCommand, DeviceInventoryCommand};

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_CASES: usize = 128;
const SCHEMA_VERSION: &str = "forge.device-inventory-status-contract/v1";
const EVALUATION_MODE: &str = "pure_projection_only";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusFixture {
    schema_version: String,
    evaluation_mode: String,
    stale_after_ms: u64,
    authority: Authority,
    cases: Vec<StatusCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusCase {
    name: String,
    input: StatusObservation,
    expected: ExpectedStatus,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusObservation {
    approval_state: String,
    cordon_state: String,
    liveness: String,
    reservation_state: String,
    snapshot_observed_at_ms: u64,
    lease_expires_at_ms: u64,
    evaluated_at_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedStatus {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    fresh: Option<bool>,
    #[serde(default)]
    declared_eligible: Option<bool>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
struct Authority {
    identity_verified: bool,
    heartbeat_persisted: bool,
    inventory_authoritative: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct InventoryStatusOutput {
    v: u16,
    #[serde(rename = "type")]
    output_type: &'static str,
    schema_version: &'static str,
    evaluation_mode: &'static str,
    stale_after_ms: u64,
    cases: Vec<StatusCaseOutput>,
    authority: Authority,
}

#[derive(Debug, Serialize)]
struct StatusCaseOutput {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fresh: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    declared_eligible: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

pub(crate) fn execute(command: &DeviceCommand) -> Result<InventoryStatusOutput, Box<dyn Error>> {
    let DeviceCommand::Inventory(DeviceInventoryCommand::Status { input }) = command else {
        return Err("device inventory status command is required".into());
    };
    let bytes = read_bounded_input(input)?;
    let fixture: StatusFixture = serde_json::from_slice(&bytes)
        .map_err(|error| format!("device inventory status input is invalid JSON: {error}"))?;
    evaluate(fixture)
}

pub(crate) fn run(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device inventory status command failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = write_output(&output, json, &mut io::stdout().lock()) {
        eprintln!("failed to write device inventory status output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

pub(crate) fn write_output(
    output: &InventoryStatusOutput,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    writeln!(
        writer,
        "offline device inventory status [{}] stale_after_ms={}",
        output.schema_version, output.stale_after_ms
    )?;
    for case in &output.cases {
        if let Some(error) = &case.error {
            writeln!(writer, "{}: error={error}", case.name)?;
        } else {
            writeln!(
                writer,
                "{}: status={} fresh={} declared_eligible={}",
                case.name,
                case.status.unwrap_or("unknown"),
                case.fresh.unwrap_or(false),
                case.declared_eligible.unwrap_or(false)
            )?;
        }
    }
    writeln!(
        writer,
        "authority: identity_verified=false heartbeat_persisted=false inventory_authoritative=false reservation_created=false execution_authorized=false dispatch_performed=false"
    )
}

fn evaluate(fixture: StatusFixture) -> Result<InventoryStatusOutput, Box<dyn Error>> {
    validate_fixture_shape(&fixture)?;
    let mut names = HashSet::with_capacity(fixture.cases.len());
    let mut cases = Vec::with_capacity(fixture.cases.len());
    for case in fixture.cases {
        if case.name.trim().is_empty() || case.name.len() > 128 {
            return Err("device inventory status case name is empty or too long".into());
        }
        if !names.insert(case.name.clone()) {
            return Err(format!("duplicate device inventory status case {:?}", case.name).into());
        }
        cases.push(evaluate_case(case, fixture.stale_after_ms)?);
    }
    Ok(InventoryStatusOutput {
        v: 1,
        output_type: "device_inventory_status",
        schema_version: SCHEMA_VERSION,
        evaluation_mode: EVALUATION_MODE,
        stale_after_ms: fixture.stale_after_ms,
        cases,
        authority: Authority::default(),
    })
}

fn validate_fixture_shape(fixture: &StatusFixture) -> Result<(), Box<dyn Error>> {
    if fixture.schema_version != SCHEMA_VERSION
        || fixture.evaluation_mode != EVALUATION_MODE
        || fixture.cases.is_empty()
        || fixture.cases.len() > MAX_CASES
        || fixture.authority != Authority::default()
    {
        return Err(
            "device inventory status input is not a bounded pure read-only contract".into(),
        );
    }
    Ok(())
}

fn evaluate_case(
    case: StatusCase,
    stale_after_ms: u64,
) -> Result<StatusCaseOutput, Box<dyn Error>> {
    let observation = InventoryStatusObservation {
        approval_state: case.input.approval_state,
        cordon_state: case.input.cordon_state,
        liveness: case.input.liveness,
        reservation_state: case.input.reservation_state,
        snapshot_observed_at_ms: case.input.snapshot_observed_at_ms,
        lease_expires_at_ms: case.input.lease_expires_at_ms,
        evaluated_at_ms: case.input.evaluated_at_ms,
    };
    let actual = project_inventory_status(&observation, stale_after_ms);
    let expected_error = case.expected.error.clone();
    match (expected_error, actual) {
        (Some(expected_error), Err(error)) => {
            if expected_error != error.to_string()
                || case.expected.status.is_some()
                || case.expected.fresh.is_some()
                || case.expected.declared_eligible.is_some()
            {
                return Err(
                    format!("inventory status case {:?} expectation mismatch", case.name).into(),
                );
            }
            Ok(StatusCaseOutput {
                name: case.name,
                status: None,
                fresh: None,
                declared_eligible: None,
                error: Some(expected_error),
            })
        }
        (None, Ok(projection)) => output_projection(case.name, case.expected, projection),
        (Some(expected_error), Ok(_)) => Err(format!(
            "inventory status case {:?} expected error {expected_error:?} but projection succeeded",
            case.name
        )
        .into()),
        (None, Err(error)) => Err(format!(
            "inventory status case {:?} rejected unexpectedly: {error}",
            case.name
        )
        .into()),
    }
}

fn output_projection(
    name: String,
    expected: ExpectedStatus,
    projection: InventoryStatusProjection,
) -> Result<StatusCaseOutput, Box<dyn Error>> {
    let expected_status = expected.status.ok_or("missing expected status")?;
    let expected_fresh = expected.fresh.ok_or("missing expected fresh")?;
    let expected_eligible = expected
        .declared_eligible
        .ok_or("missing expected declared_eligible")?;
    let actual_status = projection.status.as_str();
    if expected_status != actual_status
        || expected_fresh != projection.fresh
        || expected_eligible != projection.declared_eligible
    {
        return Err(format!("inventory status case {name:?} expectation mismatch").into());
    }
    Ok(StatusCaseOutput {
        name,
        status: Some(actual_status),
        fresh: Some(projection.fresh),
        declared_eligible: Some(projection.declared_eligible),
        error: None,
    })
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
            format!("device inventory status input exceeds {MAX_INPUT_BYTES} bytes").into(),
        );
    }
    Ok(bytes)
}
