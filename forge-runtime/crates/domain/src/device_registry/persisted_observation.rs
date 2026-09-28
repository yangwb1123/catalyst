//! Pure conversion from restored inventory state to the shared, read-only
//! device inventory observation envelope.
//!
//! This adapter deliberately keeps the owner declaration and capabilities
//! unverified. It has no storage, clock, listener, credential, reservation,
//! target-selection, or execution authority.

use std::{collections::HashSet, fmt};

use serde::{Deserialize, Serialize};

use super::{
    DEFAULT_STALE_AFTER_MS, DeviceApprovalState, PersistedInventoryState, PersistenceError,
    RunnerLiveness, SnapshotOwner, project_persisted_inventory, restore_persisted_inventory,
};

/// The inventory envelope shared by offline clients and read-only previews.
pub const PERSISTED_INVENTORY_OBSERVATION_SCHEMA_VERSION: &str =
    "forge.device-inventory-observation/v1";
/// The adapter never reads a clock; the caller supplies this evaluation mode
/// and the timestamp used for the fixed-time projection.
pub const PERSISTED_INVENTORY_OBSERVATION_EVALUATION_MODE: &str = "offline_static_only";
pub const PERSISTED_INVENTORY_OBSERVATION_NOTICE: &str = "Every owner, instance, state, timestamp, resource, residency, trust, sandbox, and concurrency value is an unverified caller declaration. This read-only observation selects no target and grants no execution authority.";
pub const MAX_PERSISTED_INVENTORY_OBSERVATIONS: usize = 128;
pub const MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// Owner metadata in the observation envelope. It is a declaration and is
/// intentionally separate from authenticated identity state.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedInventoryObservationOwner {
    pub issuer: String,
    pub subject: String,
    pub tenant_id: String,
}

impl From<&SnapshotOwner> for PersistedInventoryObservationOwner {
    fn from(value: &SnapshotOwner) -> Self {
        Self {
            issuer: value.issuer.clone(),
            subject: value.subject.clone(),
            tenant_id: value.tenant_id.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedInventoryObservationGpu {
    pub present: bool,
    pub memory_bytes: u64,
    pub runtime: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedInventoryObservationDevice {
    pub device_id: String,
    pub owner: PersistedInventoryObservationOwner,
    pub approval_state: String,
    pub cordon_state: String,
    pub liveness: String,
    pub snapshot_observed_at_ms: u64,
    pub lease_expires_at_ms: u64,
    pub os: String,
    pub architecture: String,
    pub available_cpu_cores: u32,
    pub available_memory_bytes: u64,
    pub available_storage_bytes: u64,
    pub runtimes: Vec<String>,
    pub gpu: PersistedInventoryObservationGpu,
    pub data_residency_zones: Vec<String>,
    pub trust_zone: String,
    pub sandbox_levels: Vec<String>,
    pub concurrency_limit: u16,
    pub active_concurrency: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedInventoryObservationCandidate {
    pub instance_id: String,
    pub device: PersistedInventoryObservationDevice,
}

/// A shared `forge.device-inventory-observation/v1` envelope produced from
/// already-restored values. Every authority bit is permanently false here.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
pub struct PersistedInventoryObservation {
    pub schema_version: String,
    pub evaluation_mode: String,
    pub evaluated_at_ms: u64,
    pub owner_declaration: PersistedInventoryObservationOwner,
    pub owner_declaration_unverified: bool,
    pub inventory_declarations_unverified: bool,
    pub notice: String,
    pub devices: Vec<PersistedInventoryObservationCandidate>,
    pub execution_authorized: bool,
    pub reservation_created: bool,
    pub dispatch_performed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistedInventoryObservationError {
    InvalidObservation,
    UnsupportedObservation,
    DuplicateDevice,
    DuplicateInstance,
    Persistence(PersistenceError),
}

impl fmt::Display for PersistedInventoryObservationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidObservation => "invalid_persisted_inventory_observation",
            Self::UnsupportedObservation => "unsupported_persisted_inventory_observation",
            Self::DuplicateDevice => "duplicate_persisted_inventory_device",
            Self::DuplicateInstance => "duplicate_persisted_inventory_instance",
            Self::Persistence(error) => return error.fmt(formatter),
        })
    }
}

impl std::error::Error for PersistedInventoryObservationError {}

impl From<PersistenceError> for PersistedInventoryObservationError {
    fn from(value: PersistenceError) -> Self {
        Self::Persistence(value)
    }
}

/// Converts already-restored values into the owner-bound, display-only
/// inventory envelope. The explicit evaluation time is the only time input.
/// No storage, clock, transport, reservation, target selection, or execution
/// operation occurs.
///
/// The current observation envelope has one aggregate GPU declaration and no
/// reservation field. Values containing either declaration are rejected until
/// a lossless versioned envelope exists.
///
/// # Errors
///
/// Returns an error for invalid owner/time bounds, owner mismatch, future or
/// unsafe timestamps, duplicate identities, unsupported reservation/GPU
/// declarations, or invalid persisted values. Stale values remain valid
/// observations and are classified by the fixed-time projection.
pub fn build_persisted_inventory_observation(
    values: &[PersistedInventoryState],
    evaluation_owner: &SnapshotOwner,
    evaluated_at_ms: u64,
) -> Result<PersistedInventoryObservation, PersistedInventoryObservationError> {
    if !valid_owner(evaluation_owner)
        || evaluated_at_ms == 0
        || evaluated_at_ms > MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER
        || values.len() > MAX_PERSISTED_INVENTORY_OBSERVATIONS
    {
        return Err(PersistedInventoryObservationError::InvalidObservation);
    }
    let owner = PersistedInventoryObservationOwner::from(evaluation_owner);
    let devices = collect_observation_devices(values, evaluation_owner, evaluated_at_ms, &owner)?;

    Ok(PersistedInventoryObservation {
        schema_version: PERSISTED_INVENTORY_OBSERVATION_SCHEMA_VERSION.to_owned(),
        evaluation_mode: PERSISTED_INVENTORY_OBSERVATION_EVALUATION_MODE.to_owned(),
        evaluated_at_ms,
        owner_declaration: owner,
        owner_declaration_unverified: true,
        inventory_declarations_unverified: true,
        notice: PERSISTED_INVENTORY_OBSERVATION_NOTICE.to_owned(),
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
    owner: &PersistedInventoryObservationOwner,
) -> Result<Vec<PersistedInventoryObservationCandidate>, PersistedInventoryObservationError> {
    let mut devices = Vec::with_capacity(values.len());
    let mut seen_devices = HashSet::with_capacity(values.len());
    let mut seen_instances = HashSet::with_capacity(values.len());
    for value in values {
        let canonical = canonical_observation(value, evaluation_owner, evaluated_at_ms)?;
        let device_id = canonical.device().device().id().as_str().to_owned();
        let instance_id = canonical.runner().instance_id().as_str().to_owned();
        if !seen_devices.insert(device_id.clone()) {
            return Err(PersistedInventoryObservationError::DuplicateDevice);
        }
        if !seen_instances.insert(instance_id.clone()) {
            return Err(PersistedInventoryObservationError::DuplicateInstance);
        }
        devices.push(PersistedInventoryObservationCandidate {
            instance_id,
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
) -> Result<PersistedInventoryState, PersistedInventoryObservationError> {
    let canonical = restore_persisted_inventory(
        value.revision(),
        value.device().clone(),
        value.runner().clone(),
    )?;
    if canonical.device().owner() != evaluation_owner {
        return Err(PersistedInventoryObservationError::Persistence(
            PersistenceError::InventoryOwnerMismatch,
        ));
    }
    if canonical.device().reserved() || !canonical.runner().capabilities().gpus().is_empty() {
        return Err(PersistedInventoryObservationError::UnsupportedObservation);
    }
    validate_observation_bounds(&canonical)?;
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
) -> Result<(), PersistedInventoryObservationError> {
    if canonical.runner().server_observed_at_ms() > MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER
        || canonical.runner().capability_lease_expires_at_ms()
            > MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER
        || canonical.runner().capabilities().available_memory_bytes()
            > MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER
        || canonical.runner().capabilities().available_storage_bytes()
            > MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER
    {
        return Err(PersistedInventoryObservationError::InvalidObservation);
    }
    Ok(())
}

fn observation_device(
    canonical: &PersistedInventoryState,
    device_id: String,
    owner: &PersistedInventoryObservationOwner,
) -> PersistedInventoryObservationDevice {
    let capabilities = canonical.runner().capabilities();
    PersistedInventoryObservationDevice {
        device_id,
        owner: owner.clone(),
        approval_state: approval_state(canonical.device().device().approval()).to_owned(),
        cordon_state: cordon_state(canonical.device().device().is_cordoned()).to_owned(),
        liveness: liveness(canonical.runner().liveness()).to_owned(),
        snapshot_observed_at_ms: canonical.runner().server_observed_at_ms(),
        lease_expires_at_ms: canonical.runner().capability_lease_expires_at_ms(),
        os: capabilities.operating_system().to_owned(),
        architecture: capabilities.architecture().to_owned(),
        available_cpu_cores: capabilities.available_cpu_cores(),
        available_memory_bytes: capabilities.available_memory_bytes(),
        available_storage_bytes: capabilities.available_storage_bytes(),
        runtimes: capabilities.runtimes().to_vec(),
        gpu: PersistedInventoryObservationGpu {
            present: false,
            memory_bytes: 0,
            runtime: String::new(),
        },
        data_residency_zones: Vec::new(),
        trust_zone: "unknown".to_owned(),
        sandbox_levels: Vec::new(),
        concurrency_limit: 0,
        active_concurrency: 0,
    }
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
