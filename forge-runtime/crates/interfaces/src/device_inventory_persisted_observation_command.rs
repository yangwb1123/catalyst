use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
    process::ExitCode,
};

use forge_runtime_domain::{
    CapabilitySnapshot, Device, DeviceApprovalState, DeviceId, PersistedInventoryDevice,
    PersistedInventoryObservation, PersistedInventoryObservationError, PersistedInventoryState,
    RunnerInstance, RunnerInstanceId, RunnerLiveness, SnapshotOwner, TenantId,
    build_persisted_inventory_observation, restore_persisted_inventory,
};
use serde::Deserialize;

use crate::args::{DeviceCommand, DeviceInventoryCommand};

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_STATES: usize = 128;
const SCHEMA_VERSION: &str = "forge.device-inventory-persisted-observation/v1";
const EVALUATION_MODE: &str = "pure_persisted_inventory_to_observation";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    evaluation_owner: Owner,
    evaluated_at_ms: u64,
    authority: Authority,
    states: Vec<State>,
    expected: PersistedInventoryObservation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct Owner {
    issuer: String,
    subject: String,
    tenant_id: String,
}

impl Owner {
    fn snapshot(&self) -> SnapshotOwner {
        SnapshotOwner {
            issuer: self.issuer.clone(),
            subject: self.subject.clone(),
            tenant_id: self.tenant_id.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    revision: u64,
    device: DeviceState,
    runner: RunnerState,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceState {
    device_id: String,
    owner: Owner,
    approval_state: String,
    cordon_state: String,
    reservation_state: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunnerState {
    device_id: String,
    instance_id: String,
    generation: u64,
    heartbeat_sequence: u64,
    server_observed_at_ms: u64,
    capability_lease_expires_at_ms: u64,
    liveness: String,
    capabilities: Capabilities,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Capabilities {
    os: String,
    architecture: String,
    cpu_cores: u32,
    available_cpu_cores: u32,
    memory_bytes: u64,
    available_memory_bytes: u64,
    storage_bytes: u64,
    available_storage_bytes: u64,
    gpus: Vec<Gpu>,
    runtimes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Gpu {
    id: String,
    vendor: String,
    memory_bytes: u64,
    available_memory_bytes: u64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
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

pub(crate) fn execute(
    command: &DeviceCommand,
) -> Result<PersistedInventoryObservation, Box<dyn Error>> {
    let DeviceCommand::Inventory(DeviceInventoryCommand::PersistedObservation { input }) = command
    else {
        return Err("device inventory persisted-observation command is required".into());
    };
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes).map_err(|error| {
        format!("device inventory persisted-observation input has duplicate JSON keys: {error}")
    })?;
    let fixture: Fixture = serde_json::from_slice(&bytes).map_err(|error| {
        format!("device inventory persisted-observation input is invalid JSON: {error}")
    })?;
    evaluate(fixture)
}

pub(crate) fn run(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device inventory persisted-observation failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = write_output(&output, json, &mut io::stdout().lock()) {
        eprintln!("failed to write device inventory persisted-observation output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

pub(crate) fn write_output(
    output: &PersistedInventoryObservation,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    writeln!(
        writer,
        "offline persisted inventory observation [{}] at {} devices={}",
        output.schema_version,
        output.evaluated_at_ms,
        output.devices.len()
    )?;
    writeln!(
        writer,
        "authority: identity_verified=false heartbeat_persisted=false inventory_authoritative=false reservation_created=false execution_authorized=false dispatch_performed=false"
    )?;
    for candidate in &output.devices {
        let device = &candidate.device;
        writeln!(
            writer,
            "{} / {}: approval={} cordon={} liveness={} cpu={} memory={} storage={} gpu={} runtimes={}",
            device.device_id,
            candidate.instance_id,
            device.approval_state,
            device.cordon_state,
            device.liveness,
            device.available_cpu_cores,
            device.available_memory_bytes,
            device.available_storage_bytes,
            device.gpu.present,
            device.runtimes.join(",")
        )?;
    }
    Ok(())
}

fn evaluate(fixture: Fixture) -> Result<PersistedInventoryObservation, Box<dyn Error>> {
    if fixture.schema_version != SCHEMA_VERSION
        || fixture.evaluation_mode != EVALUATION_MODE
        || fixture.states.len() > MAX_STATES
        || fixture.authority != Authority::default()
    {
        return Err("persisted-observation input is not a bounded pure read-only contract".into());
    }
    let owner = fixture.evaluation_owner.snapshot();
    let values = fixture
        .states
        .iter()
        .map(parse_state)
        .collect::<Result<Vec<_>, _>>()?;
    let actual = build_persisted_inventory_observation(&values, &owner, fixture.evaluated_at_ms)
        .map_err(format_observation_error)?;
    if fixture.expected != actual {
        return Err("persisted-observation expected envelope mismatch".into());
    }
    Ok(actual)
}

fn parse_state(value: &State) -> Result<PersistedInventoryState, Box<dyn Error>> {
    let owner = value.device.owner.snapshot();
    let device = Device::restore(
        DeviceId::parse(value.device.device_id.clone())?,
        TenantId::parse(owner.tenant_id.clone())?,
        parse_approval(&value.device.approval_state)?,
        parse_cordon(&value.device.cordon_state)?,
    );
    let persisted_device = PersistedInventoryDevice::restore(
        device,
        owner,
        parse_reservation(&value.device.reservation_state)?,
    );
    let runner = parse_runner(&value.runner)?;
    restore_persisted_inventory(value.revision, persisted_device, runner)
        .map_err(|error| error.to_string().into())
}

fn parse_runner(value: &RunnerState) -> Result<RunnerInstance, Box<dyn Error>> {
    let capabilities = value
        .capabilities
        .gpus
        .iter()
        .map(|gpu| {
            forge_runtime_domain::GpuCapability::new(
                gpu.id.clone(),
                gpu.vendor.clone(),
                gpu.memory_bytes,
                gpu.available_memory_bytes,
            )
            .map_err(|error| -> Box<dyn Error> { error.into() })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let capabilities = CapabilitySnapshot::new(
        value.capabilities.os.clone(),
        value.capabilities.architecture.clone(),
        value.capabilities.cpu_cores,
        value.capabilities.available_cpu_cores,
        value.capabilities.memory_bytes,
        value.capabilities.available_memory_bytes,
        value.capabilities.storage_bytes,
        value.capabilities.available_storage_bytes,
        capabilities,
        value.capabilities.runtimes.clone(),
    )?;
    RunnerInstance::restore(
        DeviceId::parse(value.device_id.clone())?,
        RunnerInstanceId::parse(value.instance_id.clone())?,
        value.generation,
        value.heartbeat_sequence,
        value.server_observed_at_ms,
        value.capability_lease_expires_at_ms,
        parse_liveness(&value.liveness)?,
        capabilities,
    )
    .map_err(|error| error.into())
}

fn parse_approval(value: &str) -> Result<DeviceApprovalState, Box<dyn Error>> {
    match value {
        "pending" => Ok(DeviceApprovalState::Pending),
        "approved" => Ok(DeviceApprovalState::Approved),
        "revoked" => Ok(DeviceApprovalState::Revoked),
        _ => Err(format!("unsupported device approval state {value:?}").into()),
    }
}

fn parse_cordon(value: &str) -> Result<bool, Box<dyn Error>> {
    match value {
        "clear" => Ok(false),
        "cordoned" => Ok(true),
        _ => Err(format!("unsupported device cordon state {value:?}").into()),
    }
}

fn parse_reservation(value: &str) -> Result<bool, Box<dyn Error>> {
    match value {
        "none" => Ok(false),
        "reserved" => Ok(true),
        _ => Err(format!("unsupported device reservation state {value:?}").into()),
    }
}

fn parse_liveness(value: &str) -> Result<RunnerLiveness, Box<dyn Error>> {
    match value {
        "online" => Ok(RunnerLiveness::Online),
        "offline" => Ok(RunnerLiveness::Offline),
        _ => Err(format!("unsupported Runner liveness {value:?}").into()),
    }
}

fn format_observation_error(error: PersistedInventoryObservationError) -> Box<dyn Error> {
    error.to_string().into()
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
            "device inventory persisted-observation input exceeds {MAX_INPUT_BYTES} bytes"
        )
        .into());
    }
    Ok(bytes)
}
