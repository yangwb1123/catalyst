//! Lossless v2 conversion from restored inventory to a read-only observation.
//!
//! v1 intentionally rejected reservations and GPUs because its shape could
//! not carry them. v2 carries the complete declared reservation state and GPU
//! list while keeping every authority bit false.

use std::{collections::HashSet, fmt};

use serde::{Deserialize, Serialize};

use super::{
    DEFAULT_STALE_AFTER_MS, DeviceApprovalState, PersistedInventoryState, PersistenceError,
    RunnerLiveness, SnapshotOwner, project_persisted_inventory, restore_persisted_inventory,
};

pub const PERSISTED_INVENTORY_OBSERVATION_V2_SCHEMA_VERSION: &str =
    "forge.device-inventory-observation/v2";
pub const PERSISTED_INVENTORY_OBSERVATION_V2_EVALUATION_MODE: &str = "offline_static_only";
pub const PERSISTED_INVENTORY_OBSERVATION_V2_NOTICE: &str = "Every owner, instance, state, timestamp, resource, GPU, reservation, residency, trust, sandbox, and concurrency value is an unverified caller declaration. This read-only observation selects no target and grants no execution authority.";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedInventoryObservationV2Gpu {
    pub id: String,
    pub vendor: String,
    pub memory_bytes: u64,
    pub available_memory_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedInventoryObservationV2Device {
    pub device_id: String,
    pub owner: super::PersistedInventoryObservationOwner,
    pub approval_state: String,
    pub cordon_state: String,
    pub reservation_state: String,
    pub liveness: String,
    pub snapshot_observed_at_ms: u64,
    pub lease_expires_at_ms: u64,
    pub os: String,
    pub architecture: String,
    pub available_cpu_cores: u32,
    pub available_memory_bytes: u64,
    pub available_storage_bytes: u64,
    pub runtimes: Vec<String>,
    pub gpus: Vec<PersistedInventoryObservationV2Gpu>,
    pub data_residency_zones: Vec<String>,
    pub trust_zone: String,
    pub sandbox_levels: Vec<String>,
    pub concurrency_limit: u16,
    pub active_concurrency: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedInventoryObservationV2Candidate {
    pub instance_id: String,
    pub revision: u64,
    pub generation: u64,
    pub heartbeat_sequence: u64,
    pub device: PersistedInventoryObservationV2Device,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
pub struct PersistedInventoryObservationV2 {
    pub schema_version: String,
    pub evaluation_mode: String,
    pub evaluated_at_ms: u64,
    pub owner_declaration: super::PersistedInventoryObservationOwner,
    pub owner_declaration_unverified: bool,
    pub inventory_declarations_unverified: bool,
    pub notice: String,
    pub devices: Vec<PersistedInventoryObservationV2Candidate>,
    pub execution_authorized: bool,
    pub reservation_created: bool,
    pub dispatch_performed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistedInventoryObservationV2Error {
    InvalidObservation,
    DuplicateDevice,
    DuplicateInstance,
    Persistence(PersistenceError),
}

impl fmt::Display for PersistedInventoryObservationV2Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidObservation => "invalid_persisted_inventory_observation_v2",
            Self::DuplicateDevice => "duplicate_persisted_inventory_device",
            Self::DuplicateInstance => "duplicate_persisted_inventory_instance",
            Self::Persistence(error) => return error.fmt(formatter),
        })
    }
}

impl std::error::Error for PersistedInventoryObservationV2Error {}

impl From<PersistenceError> for PersistedInventoryObservationV2Error {
    fn from(value: PersistenceError) -> Self {
        Self::Persistence(value)
    }
}

/// Converts restored inventory into the lossless, unverified v2 observation.
///
/// # Errors
///
/// Returns an error for invalid owner/time bounds, owner mismatch, unsafe
/// timestamps/resources, duplicate identities, or invalid persisted values.
/// Future timestamps fail projection; stale observations remain valid.
pub fn build_persisted_inventory_observation_v2(
    values: &[PersistedInventoryState],
    evaluation_owner: &SnapshotOwner,
    evaluated_at_ms: u64,
) -> Result<PersistedInventoryObservationV2, PersistedInventoryObservationV2Error> {
    if !valid_owner(evaluation_owner)
        || evaluated_at_ms == 0
        || evaluated_at_ms > super::MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER
        || values.len() > super::MAX_PERSISTED_INVENTORY_OBSERVATIONS
    {
        return Err(PersistedInventoryObservationV2Error::InvalidObservation);
    }
    let owner = super::PersistedInventoryObservationOwner::from(evaluation_owner);
    let devices = collect_observation_devices(values, evaluation_owner, evaluated_at_ms, &owner)?;

    Ok(PersistedInventoryObservationV2 {
        schema_version: PERSISTED_INVENTORY_OBSERVATION_V2_SCHEMA_VERSION.to_owned(),
        evaluation_mode: PERSISTED_INVENTORY_OBSERVATION_V2_EVALUATION_MODE.to_owned(),
        evaluated_at_ms,
        owner_declaration: owner,
        owner_declaration_unverified: true,
        inventory_declarations_unverified: true,
        notice: PERSISTED_INVENTORY_OBSERVATION_V2_NOTICE.to_owned(),
        devices,
        execution_authorized: false,
        reservation_created: false,
        dispatch_performed: false,
    })
}

fn collect_observation_devices(
    values: &[PersistedInventoryState],
    evaluation_owner: &SnapshotOwner,
    evaluated_at_ms: u64,
    owner: &super::PersistedInventoryObservationOwner,
) -> Result<Vec<PersistedInventoryObservationV2Candidate>, PersistedInventoryObservationV2Error> {
    let mut devices = Vec::with_capacity(values.len());
    let mut seen_devices = HashSet::with_capacity(values.len());
    let mut seen_instances = HashSet::with_capacity(values.len());
    for value in values {
        let canonical = canonical_observation(value, evaluation_owner, evaluated_at_ms)?;
        let device_id = canonical.device().device().id().as_str().to_owned();
        let instance_id = canonical.runner().instance_id().as_str().to_owned();
        if !seen_devices.insert(device_id.clone()) {
            return Err(PersistedInventoryObservationV2Error::DuplicateDevice);
        }
        if !seen_instances.insert(instance_id.clone()) {
            return Err(PersistedInventoryObservationV2Error::DuplicateInstance);
        }
        devices.push(PersistedInventoryObservationV2Candidate {
            instance_id,
            revision: value.revision(),
            generation: canonical.runner().generation(),
            heartbeat_sequence: canonical.runner().heartbeat_sequence(),
            device: observation_device(&canonical, device_id, owner),
        });
    }
    devices.sort_by(|left, right| {
        left.device
            .device_id
            .cmp(&right.device.device_id)
            .then_with(|| left.instance_id.cmp(&right.instance_id))
    });
    Ok(devices)
}

fn canonical_observation(
    value: &PersistedInventoryState,
    evaluation_owner: &SnapshotOwner,
    evaluated_at_ms: u64,
) -> Result<PersistedInventoryState, PersistedInventoryObservationV2Error> {
    let canonical = restore_persisted_inventory(
        value.revision(),
        value.device().clone(),
        value.runner().clone(),
    )?;
    if canonical.device().owner() != evaluation_owner {
        return Err(PersistedInventoryObservationV2Error::Persistence(
            PersistenceError::InventoryOwnerMismatch,
        ));
    }
    validate_observation_bounds(&canonical, value.revision())?;
    project_persisted_inventory(
        &canonical,
        evaluation_owner,
        evaluated_at_ms,
        DEFAULT_STALE_AFTER_MS,
    )?;
    Ok(canonical)
}

fn validate_observation_bounds(
    canonical: &PersistedInventoryState,
    revision: u64,
) -> Result<(), PersistedInventoryObservationV2Error> {
    if canonical.runner().server_observed_at_ms()
        > super::MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER
        || canonical.runner().capability_lease_expires_at_ms()
            > super::MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER
        || revision > super::MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER
        || canonical.runner().generation() > super::MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER
        || canonical.runner().heartbeat_sequence()
            > super::MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER
        || canonical.runner().capabilities().available_memory_bytes()
            > super::MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER
        || canonical.runner().capabilities().available_storage_bytes()
            > super::MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER
        || canonical.runner().capabilities().runtimes().len() > 32
        || canonical.runner().capabilities().gpus().iter().any(|gpu| {
            gpu.memory_bytes() > super::MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER
                || gpu.available_memory_bytes()
                    > super::MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER
        })
    {
        return Err(PersistedInventoryObservationV2Error::InvalidObservation);
    }
    Ok(())
}

fn observation_device(
    canonical: &PersistedInventoryState,
    device_id: String,
    owner: &super::PersistedInventoryObservationOwner,
) -> PersistedInventoryObservationV2Device {
    let capabilities = canonical.runner().capabilities();
    let gpus = observation_gpus(canonical);
    PersistedInventoryObservationV2Device {
        device_id,
        owner: owner.clone(),
        approval_state: approval_state(canonical.device().device().approval()).to_owned(),
        cordon_state: cordon_state(canonical.device().device().is_cordoned()).to_owned(),
        reservation_state: reservation_state(canonical.device().reserved()).to_owned(),
        liveness: liveness(canonical.runner().liveness()).to_owned(),
        snapshot_observed_at_ms: canonical.runner().server_observed_at_ms(),
        lease_expires_at_ms: canonical.runner().capability_lease_expires_at_ms(),
        os: capabilities.operating_system().to_owned(),
        architecture: capabilities.architecture().to_owned(),
        available_cpu_cores: capabilities.available_cpu_cores(),
        available_memory_bytes: capabilities.available_memory_bytes(),
        available_storage_bytes: capabilities.available_storage_bytes(),
        runtimes: capabilities.runtimes().to_vec(),
        gpus,
        data_residency_zones: Vec::new(),
        trust_zone: "unknown".to_owned(),
        sandbox_levels: Vec::new(),
        concurrency_limit: 0,
        active_concurrency: 0,
    }
}

fn observation_gpus(
    canonical: &PersistedInventoryState,
) -> Vec<PersistedInventoryObservationV2Gpu> {
    let capabilities = canonical.runner().capabilities();
    capabilities
        .gpus()
        .iter()
        .map(|gpu| PersistedInventoryObservationV2Gpu {
            id: gpu.id().to_owned(),
            vendor: gpu.vendor().to_owned(),
            memory_bytes: gpu.memory_bytes(),
            available_memory_bytes: gpu.available_memory_bytes(),
        })
        .collect()
}

const fn reservation_state(reserved: bool) -> &'static str {
    if reserved { "reserved" } else { "none" }
}

const fn cordon_state(cordoned: bool) -> &'static str {
    if cordoned { "cordoned" } else { "clear" }
}

fn valid_owner(owner: &SnapshotOwner) -> bool {
    [
        owner.issuer.as_str(),
        owner.subject.as_str(),
        owner.tenant_id.as_str(),
    ]
    .into_iter()
    .all(|value| {
        !value.is_empty()
            && value.len() <= super::snapshot::MAX_SNAPSHOT_OWNER_BYTES
            && value.trim() == value
            && !value.chars().any(char::is_control)
    })
}

const fn approval_state(value: DeviceApprovalState) -> &'static str {
    match value {
        DeviceApprovalState::Approved => "approved",
        DeviceApprovalState::Pending => "pending",
        DeviceApprovalState::Revoked => "revoked",
    }
}

const fn liveness(value: RunnerLiveness) -> &'static str {
    match value {
        RunnerLiveness::Online => "online",
        RunnerLiveness::Offline => "offline",
    }
}
