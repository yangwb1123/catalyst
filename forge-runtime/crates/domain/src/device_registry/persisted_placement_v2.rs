//! Lossless, offline placement comparison over the v2 persisted inventory
//! observation.  The comparison keeps reservations and all GPU declarations
//! visible while deliberately producing no target, lease, or authority.

use std::collections::{BTreeMap, HashSet};

use super::heartbeat::{MAX_DEVICE_CAPABILITY_LEASE_TTL_MS, MIN_DEVICE_CAPABILITY_LEASE_TTL_MS};
use super::model::{
    CapabilitySnapshot, Device, DeviceApprovalState, DeviceId, GpuCapability, MAX_DEVICE_GPU_COUNT,
    MAX_DEVICE_RUNTIME_NAME_BYTES, TenantId,
};
use super::persisted_observation::PersistedInventoryObservationOwner;
use super::persisted_observation_v2::{
    PERSISTED_INVENTORY_OBSERVATION_V2_EVALUATION_MODE, PERSISTED_INVENTORY_OBSERVATION_V2_NOTICE,
    PERSISTED_INVENTORY_OBSERVATION_V2_SCHEMA_VERSION, PersistedInventoryObservationV2,
    PersistedInventoryObservationV2Candidate,
};
use super::placement::{
    DevicePlacementCandidate, DevicePlacementDisposition, DevicePlacementExclusion,
    DevicePlacementRequirements, PersistedInventoryPlacementBatchAuthority,
    dry_run_device_placement,
};
use super::runner::{RunnerInstance, RunnerLiveness};
use super::snapshot::SnapshotOwner;

pub const PERSISTED_INVENTORY_PLACEMENT_V2_SCHEMA_VERSION: &str =
    "forge.device-inventory-placement-evaluation/v2";
pub const PERSISTED_INVENTORY_PLACEMENT_V2_EVALUATION_MODE: &str = "offline_static_only";
pub const PERSISTED_INVENTORY_PLACEMENT_V2_NOTICE: &str = "Every owner, state, timestamp, resource, GPU, reservation, residency, trust, sandbox, and concurrency value is an unverified caller declaration. This read-only comparison selects no target and grants no execution authority.";
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_CANDIDATES: usize = 128;
// The v2 observation wire contract caps runtime declarations at the shared
// observation limit.  The generic placement model has a larger legacy bound;
// do not widen this lossless v2 adapter by reusing that bound.
const MAX_V2_RUNTIME_COUNT: usize = 32;

/// One decision projected from the lossless v2 observation.  Revision,
/// generation, heartbeat, reservation, and GPU totals remain attached to the
/// decision so a later authorized scheduler can bind its input explicitly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedInventoryPlacementV2Decision {
    pub revision: u64,
    pub generation: u64,
    pub heartbeat_sequence: u64,
    pub device_id: String,
    pub instance_id: String,
    pub reservation_state: String,
    pub gpu_count: usize,
    pub available_gpu_memory_bytes: u64,
    pub matches_requirements: bool,
    pub exclusion_reasons: Vec<String>,
    pub owner_declaration_unverified: bool,
    pub device_attributes_unverified: bool,
}

/// A deterministic comparison across every v2 inventory candidate.  It is a
/// value projection only: no target is selected and no reservation, lease,
/// dispatch, or execution authority is created.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedInventoryPlacementV2Evaluation {
    pub schema_version: &'static str,
    pub evaluation_mode: &'static str,
    pub source_schema_version: &'static str,
    pub notice: &'static str,
    pub owner: SnapshotOwner,
    pub evaluated_at_ms: u64,
    pub decisions: Vec<PersistedInventoryPlacementV2Decision>,
    pub eligible_candidate_count: usize,
    pub selected_device_id: Option<String>,
    pub selected_instance_id: Option<String>,
    pub authority: PersistedInventoryPlacementBatchAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistedInventoryPlacementV2Error {
    InvalidObservation,
    OwnerMismatch,
    InvalidCandidate,
    DuplicateDevice,
    DuplicateInstance,
    InvalidEvaluationTime,
    Placement,
}

impl std::fmt::Display for PersistedInventoryPlacementV2Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidObservation => "invalid_persisted_inventory_observation_v2",
            Self::OwnerMismatch => "owner_mismatch",
            Self::InvalidCandidate => "invalid_persisted_inventory_placement_v2_candidate",
            Self::DuplicateDevice => "duplicate_persisted_inventory_device",
            Self::DuplicateInstance => "duplicate_persisted_inventory_instance",
            Self::InvalidEvaluationTime => "invalid_persisted_inventory_placement_v2_time",
            Self::Placement => "invalid_persisted_inventory_placement_v2_requirements",
        })
    }
}

impl std::error::Error for PersistedInventoryPlacementV2Error {}

/// Compares a complete v2 observation at a caller-supplied fixed time.
///
/// The source is treated as unverified data even when it originated from a
/// persistence adapter.  In particular, a `reserved` declaration excludes a
/// candidate, but this function does not create or alter that reservation.
pub fn evaluate_persisted_inventory_observation_v2(
    observation: &PersistedInventoryObservationV2,
    owner: &SnapshotOwner,
    requirements: DevicePlacementRequirements,
    evaluated_at_ms: u64,
) -> Result<PersistedInventoryPlacementV2Evaluation, PersistedInventoryPlacementV2Error> {
    validate_observation(observation, owner)?;
    if evaluated_at_ms == 0 || evaluated_at_ms > MAX_SAFE_INTEGER {
        return Err(PersistedInventoryPlacementV2Error::InvalidEvaluationTime);
    }
    if observation.devices.len() > MAX_CANDIDATES {
        return Err(PersistedInventoryPlacementV2Error::InvalidObservation);
    }

    let mut candidates = Vec::with_capacity(observation.devices.len());
    let mut metadata = BTreeMap::new();
    let mut devices = HashSet::with_capacity(observation.devices.len());
    let mut instances = HashSet::with_capacity(observation.devices.len());
    for candidate in &observation.devices {
        if !devices.insert(candidate.device.device_id.clone()) {
            return Err(PersistedInventoryPlacementV2Error::DuplicateDevice);
        }
        if !instances.insert(candidate.instance_id.clone()) {
            return Err(PersistedInventoryPlacementV2Error::DuplicateInstance);
        }
        let (placement_candidate, metadata_value) = build_candidate(candidate, owner)?;
        metadata.insert(candidate.device.device_id.clone(), metadata_value);
        candidates.push(placement_candidate);
    }

    let request = super::placement::DevicePlacementRequest::new(
        TenantId::parse(owner.tenant_id.clone())
            .map_err(|_| PersistedInventoryPlacementV2Error::OwnerMismatch)?,
        requirements,
    );
    let placement_decisions = dry_run_device_placement(&candidates, &request, evaluated_at_ms)
        .map_err(|_| PersistedInventoryPlacementV2Error::Placement)?;

    let mut decisions = Vec::with_capacity(placement_decisions.len());
    let mut eligible_candidate_count = 0;
    for decision in placement_decisions {
        let device_id = decision.device_id().as_str().to_owned();
        let metadata_value = metadata
            .remove(&device_id)
            .ok_or(PersistedInventoryPlacementV2Error::InvalidCandidate)?;
        let (mut matches_requirements, mut exclusion_reasons) = match decision.disposition() {
            DevicePlacementDisposition::Eligible => (true, Vec::new()),
            DevicePlacementDisposition::Excluded(reasons) => (
                false,
                reasons
                    .iter()
                    .map(|reason| exclusion_name(*reason).to_owned())
                    .collect::<Vec<_>>(),
            ),
        };
        if metadata_value.reservation_state == "reserved" {
            matches_requirements = false;
            exclusion_reasons.push("device_reserved".to_owned());
        }
        exclusion_reasons.sort();
        if matches_requirements {
            eligible_candidate_count += 1;
        }
        decisions.push(PersistedInventoryPlacementV2Decision {
            revision: metadata_value.revision,
            generation: metadata_value.generation,
            heartbeat_sequence: metadata_value.heartbeat_sequence,
            device_id,
            instance_id: metadata_value.instance_id,
            reservation_state: metadata_value.reservation_state,
            gpu_count: metadata_value.gpu_count,
            available_gpu_memory_bytes: metadata_value.available_gpu_memory_bytes,
            matches_requirements,
            exclusion_reasons,
            owner_declaration_unverified: true,
            device_attributes_unverified: true,
        });
    }

    Ok(PersistedInventoryPlacementV2Evaluation {
        schema_version: PERSISTED_INVENTORY_PLACEMENT_V2_SCHEMA_VERSION,
        evaluation_mode: PERSISTED_INVENTORY_PLACEMENT_V2_EVALUATION_MODE,
        source_schema_version: PERSISTED_INVENTORY_OBSERVATION_V2_SCHEMA_VERSION,
        notice: PERSISTED_INVENTORY_PLACEMENT_V2_NOTICE,
        owner: owner.clone(),
        evaluated_at_ms,
        decisions,
        eligible_candidate_count,
        selected_device_id: None,
        selected_instance_id: None,
        authority: PersistedInventoryPlacementBatchAuthority::default(),
    })
}

#[derive(Clone, Debug)]
struct CandidateMetadata {
    revision: u64,
    generation: u64,
    heartbeat_sequence: u64,
    instance_id: String,
    reservation_state: String,
    gpu_count: usize,
    available_gpu_memory_bytes: u64,
}

fn validate_observation(
    observation: &PersistedInventoryObservationV2,
    owner: &SnapshotOwner,
) -> Result<(), PersistedInventoryPlacementV2Error> {
    if !valid_owner(owner)
        || observation.schema_version != PERSISTED_INVENTORY_OBSERVATION_V2_SCHEMA_VERSION
        || observation.evaluation_mode != PERSISTED_INVENTORY_OBSERVATION_V2_EVALUATION_MODE
        || observation.evaluated_at_ms == 0
        || observation.evaluated_at_ms > MAX_SAFE_INTEGER
        || observation.owner_declaration != PersistedInventoryObservationOwner::from(owner)
        || !observation.owner_declaration_unverified
        || !observation.inventory_declarations_unverified
        || observation.notice != PERSISTED_INVENTORY_OBSERVATION_V2_NOTICE
        || observation.execution_authorized
        || observation.reservation_created
        || observation.dispatch_performed
        || observation.devices.len() > MAX_CANDIDATES
    {
        return Err(PersistedInventoryPlacementV2Error::InvalidObservation);
    }
    for (index, candidate) in observation.devices.iter().enumerate() {
        if index > 0 && observation.devices[index - 1].device.device_id > candidate.device.device_id
        {
            return Err(PersistedInventoryPlacementV2Error::InvalidObservation);
        }
    }
    Ok(())
}

fn build_candidate(
    candidate: &PersistedInventoryObservationV2Candidate,
    owner: &SnapshotOwner,
) -> Result<(DevicePlacementCandidate, CandidateMetadata), PersistedInventoryPlacementV2Error> {
    let value = &candidate.device;
    let lease_ttl_ms = value
        .lease_expires_at_ms
        .checked_sub(value.snapshot_observed_at_ms);
    if candidate.revision == 0
        || candidate.generation == 0
        || candidate.heartbeat_sequence == 0
        || candidate.revision > MAX_SAFE_INTEGER
        || candidate.generation > MAX_SAFE_INTEGER
        || candidate.heartbeat_sequence > MAX_SAFE_INTEGER
        || value.owner != PersistedInventoryObservationOwner::from(owner)
        || !matches!(value.reservation_state.as_str(), "none" | "reserved")
        || !matches!(value.cordon_state.as_str(), "clear" | "cordoned")
        || !valid_canonical_tag(&value.os)
        || !valid_canonical_tag(&value.architecture)
        || value.runtimes.len() > MAX_V2_RUNTIME_COUNT
        || value
            .runtimes
            .iter()
            .any(|runtime| !valid_canonical_tag(runtime))
        || value.runtimes.windows(2).any(|pair| pair[0] >= pair[1])
        || !value.data_residency_zones.is_empty()
        || value.trust_zone != "unknown"
        || !value.sandbox_levels.is_empty()
        || value.concurrency_limit != 0
        || value.active_concurrency != 0
        || value.snapshot_observed_at_ms > MAX_SAFE_INTEGER
        || value.lease_expires_at_ms > MAX_SAFE_INTEGER
        || !lease_ttl_ms.is_some_and(|ttl| {
            (MIN_DEVICE_CAPABILITY_LEASE_TTL_MS..=MAX_DEVICE_CAPABILITY_LEASE_TTL_MS).contains(&ttl)
        })
        || value.available_memory_bytes > MAX_SAFE_INTEGER
        || value.available_storage_bytes > MAX_SAFE_INTEGER
        || value.available_cpu_cores > super::model::MAX_DEVICE_CPU_CORES
        || value.gpus.len() > MAX_DEVICE_GPU_COUNT
        || value.gpus.windows(2).any(|pair| pair[0].id >= pair[1].id)
    {
        return Err(PersistedInventoryPlacementV2Error::InvalidCandidate);
    }

    let device_id = DeviceId::parse(value.device_id.clone())
        .map_err(|_| PersistedInventoryPlacementV2Error::InvalidCandidate)?;
    let tenant_id = TenantId::parse(value.owner.tenant_id.clone())
        .map_err(|_| PersistedInventoryPlacementV2Error::InvalidCandidate)?;
    let approval = match value.approval_state.as_str() {
        "pending" => DeviceApprovalState::Pending,
        "approved" => DeviceApprovalState::Approved,
        "revoked" => DeviceApprovalState::Revoked,
        _ => return Err(PersistedInventoryPlacementV2Error::InvalidCandidate),
    };
    let device = Device::restore(
        device_id.clone(),
        tenant_id,
        approval,
        value.cordon_state == "cordoned",
    );
    let gpus = value
        .gpus
        .iter()
        .map(|gpu| {
            if gpu.memory_bytes == 0
                || gpu.memory_bytes > MAX_SAFE_INTEGER
                || gpu.available_memory_bytes > gpu.memory_bytes
                || gpu.available_memory_bytes > MAX_SAFE_INTEGER
            {
                return Err(PersistedInventoryPlacementV2Error::InvalidCandidate);
            }
            GpuCapability::new(
                gpu.id.clone(),
                gpu.vendor.clone(),
                gpu.memory_bytes,
                gpu.available_memory_bytes,
            )
            .map_err(|_| PersistedInventoryPlacementV2Error::InvalidCandidate)
        })
        .collect::<Result<Vec<_>, _>>()?;
    // The v2 wire shape carries available values but intentionally omits the
    // total CPU/memory capacities. Use a minimal synthetic total when an
    // exhausted resource is zero; placement still compares the available
    // value and therefore reports the correct insufficiency reason.
    let cpu_capacity = value.available_cpu_cores.max(1);
    let memory_capacity = value.available_memory_bytes.max(1);
    let capabilities = CapabilitySnapshot::new(
        value.os.clone(),
        value.architecture.clone(),
        cpu_capacity,
        value.available_cpu_cores,
        memory_capacity,
        value.available_memory_bytes,
        value.available_storage_bytes,
        value.available_storage_bytes,
        gpus,
        value.runtimes.clone(),
    )
    .map_err(|_| PersistedInventoryPlacementV2Error::InvalidCandidate)?;
    let liveness = match value.liveness.as_str() {
        "online" => RunnerLiveness::Online,
        "offline" => RunnerLiveness::Offline,
        _ => return Err(PersistedInventoryPlacementV2Error::InvalidCandidate),
    };
    let instance_id = super::model::RunnerInstanceId::parse(candidate.instance_id.clone())
        .map_err(|_| PersistedInventoryPlacementV2Error::InvalidCandidate)?;
    let runner = RunnerInstance::restore(
        device_id,
        instance_id,
        candidate.generation,
        candidate.heartbeat_sequence,
        value.snapshot_observed_at_ms,
        value.lease_expires_at_ms,
        liveness,
        capabilities,
    )
    .map_err(|_| PersistedInventoryPlacementV2Error::InvalidCandidate)?;
    let placement_candidate = DevicePlacementCandidate::new(device, runner)
        .map_err(|_| PersistedInventoryPlacementV2Error::InvalidCandidate)?;

    let available_gpu_memory_bytes = value.gpus.iter().try_fold(0_u64, |total, gpu| {
        total
            .checked_add(gpu.available_memory_bytes)
            .filter(|value| *value <= MAX_SAFE_INTEGER)
            .ok_or(PersistedInventoryPlacementV2Error::InvalidCandidate)
    })?;
    Ok((
        placement_candidate,
        CandidateMetadata {
            revision: candidate.revision,
            generation: candidate.generation,
            heartbeat_sequence: candidate.heartbeat_sequence,
            instance_id: candidate.instance_id.clone(),
            reservation_state: value.reservation_state.clone(),
            gpu_count: value.gpus.len(),
            available_gpu_memory_bytes,
        },
    ))
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

fn valid_canonical_tag(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_DEVICE_RUNTIME_NAME_BYTES
        && value == value.to_ascii_lowercase()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'+'))
}

fn exclusion_name(reason: DevicePlacementExclusion) -> &'static str {
    match reason {
        DevicePlacementExclusion::RunnerOffline => "declared_offline",
        DevicePlacementExclusion::HeartbeatObservedInFuture => "snapshot_declared_from_future",
        DevicePlacementExclusion::HeartbeatStale => "snapshot_stale",
        DevicePlacementExclusion::CapabilityLeaseExpired => "declared_lease_expired",
        DevicePlacementExclusion::CapabilityLeaseInvalid => "declared_lease_invalid",
        DevicePlacementExclusion::GpuCountInsufficient => "gpu_missing",
        DevicePlacementExclusion::MemoryCapacityInsufficient => "memory_insufficient",
        DevicePlacementExclusion::StorageCapacityInsufficient => "storage_insufficient",
        reason => reason.as_str(),
    }
}
