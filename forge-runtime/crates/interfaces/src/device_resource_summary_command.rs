use std::{
    error::Error,
    fs::File,
    io::{self, Read},
    path::Path,
    process::ExitCode,
};

use serde::{Deserialize, Serialize};

use crate::args::{DeviceCommand, DeviceInventoryCommand};

mod output;
mod raw;
pub(crate) mod session_observation;
pub(crate) use output::write_output;
use output::{ResourceSummaryOutput, from_summary};
use raw::{ComputedSummary, NOTICE, summarize_declarations};

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_DECLARATIONS: usize = 128;
const API_VERSION: &str = "forgeos.device-resource-summary-contract/v1";
const INVENTORY_FIXTURE: &str = "forge-device-inventory-observation-v1";
const PLACEMENT_FIXTURE: &str = "forge-session-placement-observation-v1";
const INVENTORY_SCHEMA: &str = "forge.device-inventory-observation/v1";
const PLACEMENT_SCHEMA: &str = "forge.session-placement-observation/v1";
const EVALUATION_MODE: &str = "offline_static_only";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Owner {
    issuer: String,
    subject: String,
    tenant_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceSummaryFixture {
    api_version: String,
    inventory_contract_fixture: String,
    placement_contract_fixture: String,
    owner: Owner,
    inventory: InventoryObservation,
    placement_observation: PlacementObservation,
    expected: ExpectedSummary,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
struct InventoryObservation {
    schema_version: String,
    evaluation_mode: String,
    evaluated_at_ms: u64,
    owner_declaration: Owner,
    owner_declaration_unverified: bool,
    inventory_declarations_unverified: bool,
    notice: String,
    devices: Vec<InventoryCandidate>,
    execution_authorized: bool,
    reservation_created: bool,
    dispatch_performed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InventoryCandidate {
    instance_id: String,
    device: DeviceDeclaration,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceDeclaration {
    device_id: String,
    owner: Owner,
    approval_state: String,
    cordon_state: String,
    liveness: String,
    snapshot_observed_at_ms: u64,
    lease_expires_at_ms: u64,
    os: String,
    architecture: String,
    available_cpu_cores: u32,
    available_memory_bytes: u64,
    available_storage_bytes: u64,
    runtimes: Vec<String>,
    gpu: GpuDeclaration,
    data_residency_zones: Vec<String>,
    trust_zone: String,
    sandbox_levels: Vec<String>,
    concurrency_limit: u16,
    active_concurrency: u16,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GpuDeclaration {
    present: bool,
    memory_bytes: u64,
    runtime: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlacementObservation {
    schema_version: String,
    evaluation_mode: String,
    owner: Owner,
    conversation_id: String,
    run_id: String,
    evaluated_at_ms: u64,
    owner_declaration_unverified: bool,
    device_attributes_unverified: bool,
    decisions: Vec<PlacementDecision>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    selected_device_id: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    selected_instance_id: Option<String>,
    authority: Authority,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlacementDecision {
    device_id: String,
    instance_id: String,
    matches_requirements: bool,
    exclusion_reasons: Vec<String>,
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
struct ExpectedSummary {
    schema_version: String,
    evaluation_mode: String,
    conversation_id: String,
    run_id: String,
    evaluated_at_ms: u64,
    owner_declaration_unverified: bool,
    inventory_declarations_unverified: bool,
    placement_declaration_unverified: bool,
    notice: String,
    device_count: usize,
    runner_instance_count: usize,
    available_cpu_cores: u64,
    available_memory_bytes: u64,
    available_storage_bytes: u64,
    available_gpu_count: usize,
    available_gpu_memory_bytes: u64,
    eligible_device_count: usize,
    eligible_instance_count: usize,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    selected_device_id: Option<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    selected_instance_id: Option<String>,
    authority: Authority,
}

fn deserialize_required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

pub(crate) fn execute(command: &DeviceCommand) -> Result<ResourceSummaryOutput, Box<dyn Error>> {
    let DeviceCommand::Inventory(DeviceInventoryCommand::ResourceSummary { input }) = command
    else {
        return Err("device inventory resource-summary command is required".into());
    };
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes)
        .map_err(|error| format!("device resource summary input is invalid JSON: {error}"))?;
    let fixture: ResourceSummaryFixture = serde_json::from_slice(&bytes)
        .map_err(|error| format!("device resource summary input is invalid JSON: {error}"))?;
    evaluate(&fixture)
}

pub(crate) fn run(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device resource summary command failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = write_output(&output, json, &mut io::stdout().lock()) {
        eprintln!("failed to write device resource summary output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn evaluate(fixture: &ResourceSummaryFixture) -> Result<ResourceSummaryOutput, Box<dyn Error>> {
    validate_fixture_shape(fixture)?;
    if fixture.inventory.owner_declaration != fixture.owner
        || fixture.placement_observation.owner != fixture.owner
        || fixture.inventory.evaluated_at_ms != fixture.placement_observation.evaluated_at_ms
    {
        return Err("resource summary owner or observation binding mismatch".into());
    }
    let summary = summarize_declarations(
        &fixture.owner,
        &fixture.inventory.devices,
        &fixture.placement_observation,
    )?;
    validate_expected(&summary, &fixture.expected)?;
    Ok(from_summary(summary))
}

fn validate_fixture_shape(fixture: &ResourceSummaryFixture) -> Result<(), Box<dyn Error>> {
    if fixture.api_version != API_VERSION
        || fixture.inventory_contract_fixture != INVENTORY_FIXTURE
        || fixture.placement_contract_fixture != PLACEMENT_FIXTURE
        || fixture.inventory.schema_version != INVENTORY_SCHEMA
        || fixture.inventory.evaluation_mode != EVALUATION_MODE
        || fixture.placement_observation.schema_version != PLACEMENT_SCHEMA
        || fixture.placement_observation.evaluation_mode != EVALUATION_MODE
        || fixture.expected.schema_version != "forge.device-resource-summary/v1"
        || fixture.expected.evaluation_mode != EVALUATION_MODE
        || fixture.inventory.devices.len() > MAX_DECLARATIONS
        || fixture.inventory.evaluated_at_ms == 0
        || fixture.placement_observation.evaluated_at_ms == 0
        || !fixture.inventory.owner_declaration_unverified
        || !fixture.inventory.inventory_declarations_unverified
        || fixture.inventory.notice.trim().is_empty()
        || fixture.inventory.execution_authorized
        || fixture.inventory.reservation_created
        || fixture.inventory.dispatch_performed
        || !fixture.placement_observation.owner_declaration_unverified
        || !fixture.placement_observation.device_attributes_unverified
        || fixture.placement_observation.selected_device_id.is_some()
        || fixture.placement_observation.selected_instance_id.is_some()
        || fixture.placement_observation.authority != Authority::default()
    {
        return Err("device resource summary input is not an offline read-only observation".into());
    }
    Ok(())
}

fn validate_expected(
    actual: &ComputedSummary,
    expected: &ExpectedSummary,
) -> Result<(), Box<dyn Error>> {
    if actual.schema_version != expected.schema_version
        || actual.evaluation_mode != expected.evaluation_mode
        || actual.conversation_id != expected.conversation_id
        || actual.run_id != expected.run_id
        || actual.evaluated_at_ms != expected.evaluated_at_ms
        || actual.owner_declaration_unverified != expected.owner_declaration_unverified
        || actual.inventory_declarations_unverified != expected.inventory_declarations_unverified
        || actual.placement_declaration_unverified != expected.placement_declaration_unverified
        || actual.notice != expected.notice
        || actual.device_count != expected.device_count
        || actual.runner_instance_count != expected.runner_instance_count
        || actual.available_cpu_cores != expected.available_cpu_cores
        || actual.available_memory_bytes != expected.available_memory_bytes
        || actual.available_storage_bytes != expected.available_storage_bytes
        || actual.available_gpu_count != expected.available_gpu_count
        || actual.available_gpu_memory_bytes != expected.available_gpu_memory_bytes
        || actual.eligible_device_count != expected.eligible_device_count
        || actual.eligible_instance_count != expected.eligible_instance_count
        || actual.selected_device_id != expected.selected_device_id
        || actual.selected_instance_id != expected.selected_instance_id
        || actual.authority != expected.authority
    {
        return Err("device resource summary fixture expectation mismatch".into());
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
            format!("device resource summary input exceeds {MAX_INPUT_BYTES} bytes").into(),
        );
    }
    Ok(bytes)
}
