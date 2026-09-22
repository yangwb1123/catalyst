use std::{
    error::Error,
    fs::File,
    io::{self, Read},
    path::Path,
    process::ExitCode,
};

use serde::Deserialize;
use serde_json::Value;

use crate::args::{DeviceCommand, DeviceInventoryCommand};

use super::{
    Authority, EVALUATION_MODE, INVENTORY_SCHEMA, InventoryObservation, MAX_DECLARATIONS, NOTICE,
    Owner, PLACEMENT_SCHEMA, PlacementObservation, ResourceSummaryOutput, from_summary,
    summarize_declarations, write_output,
};

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const SCHEMA_VERSION: &str = "forge.session-device-observation/v1";
const INVENTORY_NOTICE: &str = "Every owner, instance, state, timestamp, resource, residency, trust, sandbox, and concurrency value is an unverified caller declaration. This read-only observation selects no target and grants no execution authority.";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionDeviceObservation {
    schema_version: String,
    evaluation_mode: String,
    owner: Owner,
    conversation_id: String,
    run_id: String,
    evaluated_at_ms: u64,
    owner_declaration_unverified: bool,
    inventory: InventoryObservation,
    placement_observation: PlacementObservation,
    resource_summary: ResourceSummaryObservation,
    #[serde(deserialize_with = "super::deserialize_required_nullable")]
    selected_device_id: Option<String>,
    #[serde(deserialize_with = "super::deserialize_required_nullable")]
    selected_instance_id: Option<String>,
    authority: Authority,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
struct ResourceSummaryObservation {
    schema_version: String,
    evaluation_mode: String,
    owner: Owner,
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
    #[serde(deserialize_with = "super::deserialize_required_nullable")]
    selected_device_id: Option<String>,
    #[serde(deserialize_with = "super::deserialize_required_nullable")]
    selected_instance_id: Option<String>,
    authority: Authority,
}

pub(crate) fn execute(command: &DeviceCommand) -> Result<ResourceSummaryOutput, Box<dyn Error>> {
    let DeviceCommand::Inventory(DeviceInventoryCommand::SessionObservation { input }) = command
    else {
        return Err("device inventory session-observation command is required".into());
    };
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes)
        .map_err(|error| format!("session device observation input is invalid JSON: {error}"))?;
    let value: SessionDeviceObservation = serde_json::from_slice(&bytes)
        .map_err(|error| format!("session device observation input is invalid JSON: {error}"))?;
    evaluate(&value)
}

/// Validates a canonical session observation returned by an authenticated
/// read route. The value is recomputed from its caller supplied declarations
/// before any consumer is allowed to render it.
pub(crate) fn validate_value(value: &Value) -> Result<(), Box<dyn Error>> {
    let observation: SessionDeviceObservation = serde_json::from_value(value.clone())
        .map_err(|error| format!("session device observation response is invalid: {error}"))?;
    evaluate(&observation).map(|_| ())
}

pub(crate) fn run(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Session device observation command failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = write_output(&output, json, &mut io::stdout().lock()) {
        eprintln!("failed to write session device observation output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn evaluate(value: &SessionDeviceObservation) -> Result<ResourceSummaryOutput, Box<dyn Error>> {
    validate_envelope(value)?;
    let summary = summarize_declarations(
        &value.owner,
        &value.inventory.devices,
        &value.placement_observation,
    )?;
    if !summary_matches(&summary, &value.resource_summary) {
        return Err("session device observation resource summary mismatch".into());
    }
    Ok(from_summary(summary))
}

fn validate_envelope(value: &SessionDeviceObservation) -> Result<(), Box<dyn Error>> {
    if value.schema_version != SCHEMA_VERSION
        || value.evaluation_mode != EVALUATION_MODE
        || value.evaluated_at_ms == 0
        || !value.owner_declaration_unverified
        || value.selected_device_id.is_some()
        || value.selected_instance_id.is_some()
        || value.authority != Authority::default()
    {
        return Err("session device observation is not an offline read-only envelope".into());
    }
    let inventory = &value.inventory;
    let placement = &value.placement_observation;
    if inventory.schema_version != INVENTORY_SCHEMA
        || inventory.evaluation_mode != EVALUATION_MODE
        || inventory.evaluated_at_ms != value.evaluated_at_ms
        || inventory.owner_declaration != value.owner
        || !inventory.owner_declaration_unverified
        || !inventory.inventory_declarations_unverified
        || inventory.notice != INVENTORY_NOTICE
        || inventory.execution_authorized
        || inventory.reservation_created
        || inventory.dispatch_performed
        || inventory.devices.len() > MAX_DECLARATIONS
        || placement.schema_version != PLACEMENT_SCHEMA
        || placement.evaluation_mode != EVALUATION_MODE
        || placement.owner != value.owner
        || placement.conversation_id != value.conversation_id
        || placement.run_id != value.run_id
        || placement.evaluated_at_ms != value.evaluated_at_ms
        || !placement.owner_declaration_unverified
        || !placement.device_attributes_unverified
        || placement.selected_device_id.is_some()
        || placement.selected_instance_id.is_some()
        || placement.authority != Authority::default()
        || placement.decisions.len() != inventory.devices.len()
    {
        return Err("session device observation binding mismatch".into());
    }
    validate_ordering(value)
}

fn validate_ordering(value: &SessionDeviceObservation) -> Result<(), Box<dyn Error>> {
    let devices = &value.inventory.devices;
    if devices.windows(2).any(|pair| {
        (&pair[0].device.device_id, &pair[0].instance_id)
            >= (&pair[1].device.device_id, &pair[1].instance_id)
    }) || devices
        .iter()
        .any(|candidate| candidate.device.owner != value.owner)
    {
        return Err("session device inventory ordering or owner binding is invalid".into());
    }
    let decisions = &value.placement_observation.decisions;
    if decisions.windows(2).any(|pair| {
        (&pair[0].device_id, &pair[0].instance_id) >= (&pair[1].device_id, &pair[1].instance_id)
    }) {
        return Err("session placement decision ordering is invalid".into());
    }
    for (candidate, decision) in devices.iter().zip(decisions) {
        if decision.device_id != candidate.device.device_id
            || decision.instance_id != candidate.instance_id
            || decision.matches_requirements != decision.exclusion_reasons.is_empty()
            || decision.exclusion_reasons.len() > 64
            || decision
                .exclusion_reasons
                .iter()
                .any(|reason| !valid_token(reason))
            || decision
                .exclusion_reasons
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err("session placement decision binding is invalid".into());
        }
    }
    Ok(())
}

fn valid_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'+' | b'/' | b'-')
        })
}

fn summary_matches(actual: &super::ComputedSummary, value: &ResourceSummaryObservation) -> bool {
    value.schema_version == actual.schema_version
        && value.evaluation_mode == actual.evaluation_mode
        && value.owner.issuer == actual.owner.issuer
        && value.owner.subject == actual.owner.subject
        && value.owner.tenant_id == actual.owner.tenant_id
        && value.conversation_id == actual.conversation_id
        && value.run_id == actual.run_id
        && value.evaluated_at_ms == actual.evaluated_at_ms
        && value.owner_declaration_unverified == actual.owner_declaration_unverified
        && value.inventory_declarations_unverified == actual.inventory_declarations_unverified
        && value.placement_declaration_unverified == actual.placement_declaration_unverified
        && value.notice == NOTICE
        && value.notice == actual.notice
        && value.device_count == actual.device_count
        && value.runner_instance_count == actual.runner_instance_count
        && value.available_cpu_cores == actual.available_cpu_cores
        && value.available_memory_bytes == actual.available_memory_bytes
        && value.available_storage_bytes == actual.available_storage_bytes
        && value.available_gpu_count == actual.available_gpu_count
        && value.available_gpu_memory_bytes == actual.available_gpu_memory_bytes
        && value.eligible_device_count == actual.eligible_device_count
        && value.eligible_instance_count == actual.eligible_instance_count
        && value.selected_device_id == actual.selected_device_id
        && value.selected_instance_id == actual.selected_instance_id
        && value.authority == actual.authority
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
        return Err(format!("session device observation exceeds {MAX_INPUT_BYTES} bytes").into());
    }
    Ok(bytes)
}
