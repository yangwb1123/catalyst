use super::wire::Owner;
use super::{EVALUATION_MODE, MAX_SAFE_INTEGER, MAX_V2_RUNTIME_COUNT, SOURCE_SCHEMA_VERSION};
use forge_runtime_domain::{
    DeviceId, MAX_DEVICE_CPU_CORES, MAX_DEVICE_GPU_COUNT, MAX_DEVICE_RUNTIME_NAME_BYTES,
    MAX_PERSISTED_INVENTORY_OBSERVATIONS, MAX_SNAPSHOT_OWNER_BYTES,
    PersistedInventoryObservationOwner, PersistedInventoryObservationV2,
    PersistedInventoryObservationV2Candidate, PersistedInventoryObservationV2Device,
    RunnerInstanceId, SnapshotOwner, TenantId,
};
use std::{collections::HashSet, error::Error};
pub(super) fn validate_observation(
    observation: &PersistedInventoryObservationV2,
    owner: &SnapshotOwner,
) -> Result<(), Box<dyn Error>> {
    validate_observation_contract(observation, owner)?;
    let expected_owner = PersistedInventoryObservationOwner::from(owner);
    let mut devices = HashSet::with_capacity(observation.devices.len());
    let mut instances = HashSet::with_capacity(observation.devices.len());
    let mut previous = None;
    for candidate in &observation.devices {
        let key = (
            candidate.device.device_id.as_str(),
            candidate.instance_id.as_str(),
        );
        if previous.is_some_and(|value| value > key) {
            return Err("v2 placement-evaluation observation is not sorted".into());
        }
        previous = Some(key);
        if !devices.insert(candidate.device.device_id.clone())
            || !instances.insert(candidate.instance_id.clone())
        {
            return Err("v2 placement-evaluation observation has duplicate identity".into());
        }
        if candidate.revision == 0
            || candidate.generation == 0
            || candidate.heartbeat_sequence == 0
            || candidate.revision > MAX_SAFE_INTEGER
            || candidate.generation > MAX_SAFE_INTEGER
            || candidate.heartbeat_sequence > MAX_SAFE_INTEGER
        {
            return Err("v2 placement-evaluation observation has unsafe counters".into());
        }
        validate_device_declaration(candidate, &expected_owner)?;
        validate_device_tags(&candidate.device)?;
    }
    Ok(())
}

pub(super) fn validate_owner(value: &Owner) -> Result<(), Box<dyn Error>> {
    for part in [&value.issuer, &value.subject, &value.tenant_id] {
        if part.is_empty()
            || part.len() > MAX_SNAPSHOT_OWNER_BYTES
            || part.trim() != part
            || part.chars().any(char::is_control)
        {
            return Err("v2 placement-evaluation owner declaration is invalid".into());
        }
    }
    Ok(())
}

fn validate_runtimes(runtimes: &[String]) -> Result<(), Box<dyn Error>> {
    let mut previous = None;
    for runtime in runtimes {
        if previous.is_some_and(|value: &str| value >= runtime.as_str()) {
            return Err("v2 placement-evaluation runtimes are not sorted and unique".into());
        }
        previous = Some(runtime.as_str());
        validate_canonical_tag(runtime)?;
        if runtime != &runtime.to_ascii_lowercase() {
            return Err("v2 placement-evaluation runtime is not normalized".into());
        }
    }
    Ok(())
}

fn validate_gpus(
    gpus: &[forge_runtime_domain::PersistedInventoryObservationV2Gpu],
) -> Result<(), Box<dyn Error>> {
    let mut previous = None;
    let mut ids = HashSet::with_capacity(gpus.len());
    for gpu in gpus {
        if previous.is_some_and(|value: &str| value >= gpu.id.as_str())
            || !ids.insert(gpu.id.clone())
            || DeviceId::parse(gpu.id.clone()).is_err()
            || gpu.memory_bytes == 0
            || gpu.memory_bytes > MAX_SAFE_INTEGER
            || gpu.available_memory_bytes > gpu.memory_bytes
            || gpu.available_memory_bytes > MAX_SAFE_INTEGER
        {
            return Err("v2 placement-evaluation GPU declaration is invalid".into());
        }
        previous = Some(gpu.id.as_str());
        validate_label(&gpu.vendor)?;
    }
    Ok(())
}

fn validate_label(value: &str) -> Result<(), Box<dyn Error>> {
    if value.is_empty()
        || value.len() > MAX_DEVICE_RUNTIME_NAME_BYTES
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err("v2 placement-evaluation label is invalid".into());
    }
    Ok(())
}

fn validate_canonical_tag(value: &str) -> Result<(), Box<dyn Error>> {
    if value.is_empty()
        || value.len() > MAX_DEVICE_RUNTIME_NAME_BYTES
        || value != value.to_ascii_lowercase()
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'_' | b'-' | b'+')
        })
    {
        return Err("v2 placement-evaluation canonical tag is invalid".into());
    }
    Ok(())
}

fn validate_observation_contract(
    observation: &PersistedInventoryObservationV2,
    owner: &SnapshotOwner,
) -> Result<(), Box<dyn Error>> {
    if observation.schema_version != SOURCE_SCHEMA_VERSION
        || observation.evaluation_mode != EVALUATION_MODE
        || observation.evaluated_at_ms == 0
        || observation.evaluated_at_ms > MAX_SAFE_INTEGER
        || observation.owner_declaration != PersistedInventoryObservationOwner::from(owner)
        || !observation.owner_declaration_unverified
        || !observation.inventory_declarations_unverified
        || observation.notice
            != "Every owner, instance, state, timestamp, resource, GPU, reservation, residency, trust, sandbox, and concurrency value is an unverified caller declaration. This read-only observation selects no target and grants no execution authority."
        || observation.execution_authorized
        || observation.reservation_created
        || observation.dispatch_performed
        || observation.devices.len() > MAX_PERSISTED_INVENTORY_OBSERVATIONS
    {
        return Err(
            "v2 placement-evaluation observation is not a bounded read-only contract".into(),
        );
    }
    Ok(())
}

fn validate_device_declaration(
    candidate: &PersistedInventoryObservationV2Candidate,
    expected_owner: &PersistedInventoryObservationOwner,
) -> Result<(), Box<dyn Error>> {
    let device = &candidate.device;
    if device.owner != *expected_owner
        || DeviceId::parse(device.device_id.clone()).is_err()
        || RunnerInstanceId::parse(candidate.instance_id.clone()).is_err()
        || TenantId::parse(device.owner.tenant_id.clone()).is_err()
        || device.snapshot_observed_at_ms > MAX_SAFE_INTEGER
        || device.lease_expires_at_ms < device.snapshot_observed_at_ms
        || device.lease_expires_at_ms > MAX_SAFE_INTEGER
        || device.available_cpu_cores > MAX_DEVICE_CPU_CORES
        || device.available_memory_bytes > MAX_SAFE_INTEGER
        || device.available_storage_bytes > MAX_SAFE_INTEGER
        || device.runtimes.len() > MAX_V2_RUNTIME_COUNT
        || device.gpus.len() > MAX_DEVICE_GPU_COUNT
        || device.concurrency_limit != 0
        || device.active_concurrency != 0
        || !device.data_residency_zones.is_empty()
        || device.trust_zone != "unknown"
        || !device.sandbox_levels.is_empty()
        || !matches!(
            device.approval_state.as_str(),
            "pending" | "approved" | "revoked"
        )
        || !matches!(device.cordon_state.as_str(), "clear" | "cordoned")
        || !matches!(device.reservation_state.as_str(), "none" | "reserved")
        || !matches!(device.liveness.as_str(), "online" | "offline")
    {
        return Err(format!(
            "v2 placement-evaluation device {} declaration is invalid",
            device.device_id
        )
        .into());
    }
    Ok(())
}

fn validate_device_tags(
    device: &PersistedInventoryObservationV2Device,
) -> Result<(), Box<dyn Error>> {
    validate_canonical_tag(&device.os)?;
    validate_canonical_tag(&device.architecture)?;
    if device.os != device.os.to_ascii_lowercase()
        || device.architecture != device.architecture.to_ascii_lowercase()
    {
        return Err("v2 placement-evaluation device tags are not normalized".into());
    }
    validate_runtimes(&device.runtimes)?;
    validate_gpus(&device.gpus)?;
    Ok(())
}
