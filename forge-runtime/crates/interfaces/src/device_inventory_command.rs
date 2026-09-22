use std::{
    collections::HashSet,
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use forge_runtime_domain::{DeviceId, RunnerInstanceId, TenantId};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::args::{DeviceCommand, DeviceInventoryCommand};

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_INVENTORY_ROWS: usize = 128;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const OBSERVATION_SCHEMA: &str = "forge.device-inventory-observation/v1";
const EVALUATION_MODE: &str = "offline_static_only";

#[derive(Debug, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
#[serde(deny_unknown_fields)]
struct InventoryFixture {
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Owner {
    issuer: String,
    subject: String,
    tenant_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InventoryCandidate {
    instance_id: String,
    device: DeviceDeclaration,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct GpuDeclaration {
    present: bool,
    memory_bytes: u64,
    runtime: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct InventoryShowOutput {
    schema_version: String,
    evaluation_mode: &'static str,
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

pub(crate) fn execute(command: &DeviceCommand) -> Result<InventoryShowOutput, Box<dyn Error>> {
    let DeviceCommand::Inventory(DeviceInventoryCommand::Show { input }) = command else {
        return Err("device inventory command is required".into());
    };
    let bytes = read_bounded_input(input)?;
    let fixture: InventoryFixture = serde_json::from_slice(&bytes)
        .map_err(|error| format!("device inventory input is invalid JSON: {error}"))?;
    normalize(fixture)
}

/// Validates an authenticated inventory response without creating a local
/// inventory projection. The remote candidate carries the same unverified,
/// read-only envelope as the offline command, so keep one strict shape and
/// identity validator for both boundaries.
pub(crate) fn validate_remote_response(value: &Value) -> Result<(), Box<dyn Error>> {
    let fixture: InventoryFixture = serde_json::from_value(value.clone())
        .map_err(|error| format!("device inventory response is invalid: {error}"))?;
    validate_fixture(&fixture)
}

pub(crate) fn write_output(
    output: &InventoryShowOutput,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    writeln!(
        writer,
        "offline device inventory [{}] at {}",
        output.schema_version, output.evaluated_at_ms
    )?;
    writeln!(
        writer,
        "declarations: owner_unverified={} inventory_unverified={}",
        output.owner_declaration_unverified, output.inventory_declarations_unverified
    )?;
    writeln!(
        writer,
        "authority: identity_verified=false heartbeat_persisted=false inventory_authoritative=false reservation_created=false execution_authorized=false dispatch_performed=false"
    )?;
    for candidate in &output.devices {
        let device = &candidate.device;
        writeln!(
            writer,
            "{} / {}: approval={} cordon={} liveness={} cpu={} memory={} storage={} gpu={} gpu_memory={} runtimes={}",
            device.device_id,
            candidate.instance_id,
            device.approval_state,
            device.cordon_state,
            device.liveness,
            device.available_cpu_cores,
            device.available_memory_bytes,
            device.available_storage_bytes,
            device.gpu.present,
            device.gpu.memory_bytes,
            device.runtimes.join(",")
        )?;
    }
    Ok(())
}

fn normalize(fixture: InventoryFixture) -> Result<InventoryShowOutput, Box<dyn Error>> {
    validate_fixture(&fixture)?;
    let mut devices = fixture.devices;
    devices.sort_by(|left, right| {
        left.device
            .device_id
            .cmp(&right.device.device_id)
            .then_with(|| left.instance_id.cmp(&right.instance_id))
    });
    Ok(InventoryShowOutput {
        schema_version: fixture.schema_version,
        evaluation_mode: EVALUATION_MODE,
        evaluated_at_ms: fixture.evaluated_at_ms,
        owner_declaration: fixture.owner_declaration,
        owner_declaration_unverified: fixture.owner_declaration_unverified,
        inventory_declarations_unverified: fixture.inventory_declarations_unverified,
        notice: fixture.notice,
        devices,
        execution_authorized: false,
        reservation_created: false,
        dispatch_performed: false,
    })
}

fn validate_fixture(fixture: &InventoryFixture) -> Result<(), Box<dyn Error>> {
    validate_fixture_shape(fixture)?;
    validate_owner(&fixture.owner_declaration)?;
    TenantId::parse(fixture.owner_declaration.tenant_id.clone())?;
    let mut seen_device_ids = HashSet::with_capacity(fixture.devices.len());
    let mut seen_instance_ids = HashSet::with_capacity(fixture.devices.len());
    for candidate in &fixture.devices {
        validate_candidate(
            candidate,
            &fixture.owner_declaration,
            &mut seen_device_ids,
            &mut seen_instance_ids,
        )?;
    }
    Ok(())
}

fn validate_fixture_shape(fixture: &InventoryFixture) -> Result<(), Box<dyn Error>> {
    if fixture.schema_version != OBSERVATION_SCHEMA {
        return Err(format!(
            "unsupported device inventory schema {:?}; expected {OBSERVATION_SCHEMA:?}",
            fixture.schema_version
        )
        .into());
    }
    if fixture.evaluation_mode != EVALUATION_MODE {
        return Err(format!(
            "unsupported device inventory evaluation mode {:?}",
            fixture.evaluation_mode
        )
        .into());
    }
    if fixture.evaluated_at_ms == 0
        || fixture.evaluated_at_ms > MAX_SAFE_INTEGER
        || fixture.devices.len() > MAX_INVENTORY_ROWS
        || !fixture.owner_declaration_unverified
        || !fixture.inventory_declarations_unverified
        || fixture.notice.trim().is_empty()
        || fixture.execution_authorized
        || fixture.reservation_created
        || fixture.dispatch_performed
    {
        return Err("device inventory input is not an unverified read-only observation".into());
    }
    Ok(())
}

fn validate_owner(owner: &Owner) -> Result<(), Box<dyn Error>> {
    if owner.issuer.trim().is_empty()
        || owner.subject.trim().is_empty()
        || owner.tenant_id.trim().is_empty()
    {
        return Err("device inventory owner declaration is incomplete".into());
    }
    Ok(())
}

fn validate_candidate(
    candidate: &InventoryCandidate,
    owner: &Owner,
    seen_device_ids: &mut HashSet<String>,
    seen_instance_ids: &mut HashSet<String>,
) -> Result<(), Box<dyn Error>> {
    let device = &candidate.device;
    if !same_owner(&device.owner, owner) {
        return Err(format!(
            "candidate {} owner does not match the input owner declaration",
            device.device_id
        )
        .into());
    }
    validate_owner(&device.owner)?;
    DeviceId::parse(device.device_id.clone())?;
    RunnerInstanceId::parse(candidate.instance_id.clone())?;
    TenantId::parse(device.owner.tenant_id.clone())?;
    if !seen_device_ids.insert(device.device_id.clone()) {
        return Err(format!("duplicate device inventory device {}", device.device_id).into());
    }
    if !seen_instance_ids.insert(candidate.instance_id.clone()) {
        return Err(format!(
            "duplicate device inventory Runner instance {}",
            candidate.instance_id
        )
        .into());
    }
    if device.snapshot_observed_at_ms > MAX_SAFE_INTEGER
        || device.lease_expires_at_ms > MAX_SAFE_INTEGER
        || device.available_memory_bytes > MAX_SAFE_INTEGER
        || device.available_storage_bytes > MAX_SAFE_INTEGER
        || device.gpu.memory_bytes > MAX_SAFE_INTEGER
    {
        return Err(format!(
            "device {} contains an integer outside the cross-client safe range",
            device.device_id
        )
        .into());
    }
    if !device.gpu.present && device.gpu.memory_bytes != 0 {
        return Err(format!(
            "device {} declares GPU memory while GPU is absent",
            device.device_id
        )
        .into());
    }
    Ok(())
}

fn same_owner(left: &Owner, right: &Owner) -> bool {
    left.issuer == right.issuer
        && left.subject == right.subject
        && left.tenant_id == right.tenant_id
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
        return Err(format!("device inventory input exceeds {MAX_INPUT_BYTES} bytes").into());
    }
    Ok(bytes)
}
