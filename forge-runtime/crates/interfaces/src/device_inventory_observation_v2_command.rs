use serde_json::Value;
use std::{
    collections::HashSet,
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
    process::ExitCode,
};

use forge_runtime_domain::{
    DeviceId, MAX_DEVICE_CAPABILITY_LEASE_TTL_MS, MAX_DEVICE_GPU_COUNT,
    MAX_DEVICE_RUNTIME_NAME_BYTES, MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER,
    MAX_PERSISTED_INVENTORY_OBSERVATIONS, MAX_SNAPSHOT_OWNER_BYTES,
    MIN_DEVICE_CAPABILITY_LEASE_TTL_MS, PERSISTED_INVENTORY_OBSERVATION_V2_EVALUATION_MODE,
    PERSISTED_INVENTORY_OBSERVATION_V2_NOTICE, PERSISTED_INVENTORY_OBSERVATION_V2_SCHEMA_VERSION,
    PersistedInventoryObservationV2, PersistedInventoryObservationV2Device,
    PersistedInventoryObservationV2Gpu, RunnerInstanceId, TenantId,
};

use crate::args::{DeviceCommand, DeviceInventoryCommand};

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_V2_ARRAY_ITEMS: usize = 32;

/// Consumes one lossless v2 inventory envelope as a bounded, read-only local
/// preview. This command never contacts Forge Core and never changes the v1
/// inventory consumer.
pub(crate) fn execute(
    command: &DeviceCommand,
) -> Result<PersistedInventoryObservationV2, Box<dyn Error>> {
    let DeviceCommand::Inventory(DeviceInventoryCommand::PersistedObservationV2 { input }) =
        command
    else {
        return Err("device inventory persisted-observation-v2 command is required".into());
    };
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes).map_err(|error| {
        format!("device inventory persisted-observation-v2 input has duplicate JSON keys: {error}")
    })?;
    let observation: PersistedInventoryObservationV2 =
        serde_json::from_slice(&bytes).map_err(|error| {
            format!("device inventory persisted-observation-v2 input is invalid JSON: {error}")
        })?;
    validate_observation(&observation)?;
    Ok(observation)
}

/// Validates the authenticated lossless v2 inventory candidate without
/// constructing a durable local inventory projection. The remote endpoint is
/// a test/injected read seam; authority bits remain false and malformed
/// values fail closed before rendering.
pub(crate) fn validate_remote_response(value: &Value) -> Result<(), Box<dyn Error>> {
    let observation: PersistedInventoryObservationV2 = serde_json::from_value(value.clone())
        .map_err(|error| format!("device inventory v2 response is invalid: {error}"))?;
    validate_observation(&observation)
}

pub(crate) fn run(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device inventory persisted-observation-v2 failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = write_output(&output, json, &mut io::stdout().lock()) {
        eprintln!("failed to write device inventory persisted-observation-v2 output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

pub(crate) fn write_output(
    output: &PersistedInventoryObservationV2,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    writeln!(
        writer,
        "offline persisted inventory observation v2 [{}] at {} devices={}",
        output.schema_version,
        output.evaluated_at_ms,
        output.devices.len()
    )?;
    writeln!(
        writer,
        "authority: execution_authorized=false reservation_created=false dispatch_performed=false"
    )?;
    for candidate in &output.devices {
        let device = &candidate.device;
        let gpu_summary = device
            .gpus
            .iter()
            .map(|gpu| format!("{}:{}", gpu.id, gpu.available_memory_bytes))
            .collect::<Vec<_>>()
            .join(",");
        writeln!(
            writer,
            "{} / {}: revision={} generation={} heartbeat={} approval={} cordon={} reservation={} liveness={} cpu={} memory={} storage={} gpus=[{}] runtimes={}",
            device.device_id,
            candidate.instance_id,
            candidate.revision,
            candidate.generation,
            candidate.heartbeat_sequence,
            device.approval_state,
            device.cordon_state,
            device.reservation_state,
            device.liveness,
            device.available_cpu_cores,
            device.available_memory_bytes,
            device.available_storage_bytes,
            gpu_summary,
            device.runtimes.join(",")
        )?;
    }
    Ok(())
}

fn validate_observation(
    observation: &PersistedInventoryObservationV2,
) -> Result<(), Box<dyn Error>> {
    if observation.schema_version != PERSISTED_INVENTORY_OBSERVATION_V2_SCHEMA_VERSION
        || observation.evaluation_mode != PERSISTED_INVENTORY_OBSERVATION_V2_EVALUATION_MODE
        || observation.evaluated_at_ms == 0
        || observation.evaluated_at_ms > MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER
        || observation.devices.len() > MAX_PERSISTED_INVENTORY_OBSERVATIONS
        || !observation.owner_declaration_unverified
        || !observation.inventory_declarations_unverified
        || observation.notice != PERSISTED_INVENTORY_OBSERVATION_V2_NOTICE
        || observation.execution_authorized
        || observation.reservation_created
        || observation.dispatch_performed
    {
        return Err("device inventory v2 input is not an unverified read-only observation".into());
    }
    validate_owner(
        observation.owner_declaration.issuer.as_str(),
        observation.owner_declaration.subject.as_str(),
        observation.owner_declaration.tenant_id.as_str(),
    )?;

    let mut seen_devices = HashSet::with_capacity(observation.devices.len());
    let mut seen_instances = HashSet::with_capacity(observation.devices.len());
    let mut previous_key: Option<(&str, &str)> = None;
    for candidate in &observation.devices {
        if candidate.revision == 0
            || candidate.generation == 0
            || candidate.heartbeat_sequence == 0
            || candidate.revision > MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER
            || candidate.generation > MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER
            || candidate.heartbeat_sequence > MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER
        {
            return Err(format!(
                "device {} has an unsafe or zero revision/generation/heartbeat counter",
                candidate.device.device_id
            )
            .into());
        }
        let device = &candidate.device;
        let key = (device.device_id.as_str(), candidate.instance_id.as_str());
        if previous_key.is_some_and(|previous| previous > key) {
            return Err(
                "device inventory v2 candidates are not sorted by device then instance".into(),
            );
        }
        previous_key = Some(key);
        if !seen_devices.insert(device.device_id.clone()) {
            return Err(format!("duplicate device inventory device {}", device.device_id).into());
        }
        if !seen_instances.insert(candidate.instance_id.clone()) {
            return Err(format!(
                "duplicate device inventory Runner instance {}",
                candidate.instance_id
            )
            .into());
        }
        validate_device(observation, candidate.instance_id.as_str(), device)?;
    }
    Ok(())
}

fn validate_device(
    observation: &PersistedInventoryObservationV2,
    instance_id: &str,
    device: &PersistedInventoryObservationV2Device,
) -> Result<(), Box<dyn Error>> {
    if device.owner != observation.owner_declaration {
        return Err(format!(
            "candidate {} owner does not match the envelope owner declaration",
            device.device_id
        )
        .into());
    }
    validate_owner(
        device.owner.issuer.as_str(),
        device.owner.subject.as_str(),
        device.owner.tenant_id.as_str(),
    )?;
    DeviceId::parse(device.device_id.clone())?;
    RunnerInstanceId::parse(instance_id.to_owned())?;
    TenantId::parse(device.owner.tenant_id.clone())?;
    if device.snapshot_observed_at_ms > MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER
        || device.lease_expires_at_ms > MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER
        || !device
            .lease_expires_at_ms
            .checked_sub(device.snapshot_observed_at_ms)
            .is_some_and(|ttl| {
                (MIN_DEVICE_CAPABILITY_LEASE_TTL_MS..=MAX_DEVICE_CAPABILITY_LEASE_TTL_MS)
                    .contains(&ttl)
            })
        || device.available_memory_bytes > MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER
        || device.available_storage_bytes > MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER
        || device.available_cpu_cores > 4_096
        || device.data_residency_zones.len() > 64
        || device.sandbox_levels.len() > 64
        || device.concurrency_limit != 0
        || device.active_concurrency != 0
    {
        return Err(format!(
            "device {} contains an unsafe v2 resource declaration",
            device.device_id
        )
        .into());
    }
    if !matches!(
        device.approval_state.as_str(),
        "pending" | "approved" | "revoked"
    ) || !matches!(device.cordon_state.as_str(), "clear" | "cordoned")
        || !matches!(device.reservation_state.as_str(), "none" | "reserved")
        || !matches!(device.liveness.as_str(), "online" | "offline")
        || device.os.is_empty()
        || device.architecture.is_empty()
        || device.os != device.os.to_ascii_lowercase()
        || device.architecture != device.architecture.to_ascii_lowercase()
        || device.trust_zone != "unknown"
        || !device.data_residency_zones.is_empty()
        || !device.sandbox_levels.is_empty()
    {
        return Err(format!(
            "device {} contains an unsupported v2 declaration",
            device.device_id
        )
        .into());
    }
    validate_canonical_tag(&device.os, MAX_DEVICE_RUNTIME_NAME_BYTES)?;
    validate_canonical_tag(&device.architecture, MAX_DEVICE_RUNTIME_NAME_BYTES)?;
    validate_runtimes(&device.runtimes)?;
    validate_gpus(&device.gpus)?;
    Ok(())
}

fn validate_gpus(gpus: &[PersistedInventoryObservationV2Gpu]) -> Result<(), Box<dyn Error>> {
    if gpus.len() > MAX_DEVICE_GPU_COUNT {
        return Err("device inventory v2 GPU count exceeds the bounded limit".into());
    }
    let mut previous_id: Option<&str> = None;
    let mut seen = HashSet::with_capacity(gpus.len());
    for gpu in gpus {
        if previous_id.is_some_and(|previous| previous >= gpu.id.as_str()) {
            return Err("device inventory v2 GPUs are not sorted by unique id".into());
        }
        previous_id = Some(gpu.id.as_str());
        if !seen.insert(gpu.id.clone()) {
            return Err(format!("duplicate GPU {}", gpu.id).into());
        }
        DeviceId::parse(gpu.id.clone())?;
        validate_label(&gpu.vendor, MAX_DEVICE_RUNTIME_NAME_BYTES)?;
        if gpu.memory_bytes == 0
            || gpu.memory_bytes > MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER
            || gpu.available_memory_bytes > gpu.memory_bytes
            || gpu.available_memory_bytes > MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER
        {
            return Err(format!("GPU {} has an invalid memory declaration", gpu.id).into());
        }
    }
    Ok(())
}

fn validate_runtimes(runtimes: &[String]) -> Result<(), Box<dyn Error>> {
    if runtimes.len() > MAX_V2_ARRAY_ITEMS {
        return Err("device inventory v2 runtime count exceeds the bounded limit".into());
    }
    let mut previous: Option<&str> = None;
    for runtime in runtimes {
        if previous.is_some_and(|value| value >= runtime.as_str()) {
            return Err("device inventory v2 runtimes are not sorted and unique".into());
        }
        previous = Some(runtime.as_str());
        if runtime != &runtime.to_ascii_lowercase() {
            return Err(format!("runtime {runtime:?} is not normalized").into());
        }
        validate_canonical_tag(runtime, MAX_DEVICE_RUNTIME_NAME_BYTES)?;
    }
    Ok(())
}

fn validate_owner(issuer: &str, subject: &str, tenant_id: &str) -> Result<(), Box<dyn Error>> {
    for value in [issuer, subject, tenant_id] {
        if value.is_empty()
            || value.len() > MAX_SNAPSHOT_OWNER_BYTES
            || value.trim() != value
            || value.chars().any(char::is_control)
        {
            return Err("device inventory v2 owner declaration is invalid".into());
        }
    }
    Ok(())
}

fn validate_label(value: &str, max_bytes: usize) -> Result<(), Box<dyn Error>> {
    if value.is_empty()
        || value.len() > max_bytes
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(format!("v2 label {value:?} is invalid").into());
    }
    Ok(())
}

fn validate_canonical_tag(value: &str, max_bytes: usize) -> Result<(), Box<dyn Error>> {
    if value.is_empty()
        || value.len() > max_bytes
        || value != value.to_ascii_lowercase()
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'_' | b'-' | b'+')
        })
    {
        return Err(format!("v2 canonical tag {value:?} is invalid").into());
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
        return Err(format!(
            "device inventory persisted-observation-v2 input exceeds {MAX_INPUT_BYTES} bytes"
        )
        .into());
    }
    Ok(bytes)
}
