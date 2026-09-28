//! Validation and reconstruction of lossless v2 placement candidates.

use super::super::heartbeat::{
    MAX_DEVICE_CAPABILITY_LEASE_TTL_MS, MIN_DEVICE_CAPABILITY_LEASE_TTL_MS,
};
use super::super::model::{
    CapabilitySnapshot, Device, DeviceApprovalState, DeviceId, GpuCapability, MAX_DEVICE_GPU_COUNT,
    MAX_DEVICE_RUNTIME_NAME_BYTES, TenantId,
};
use super::super::persisted_observation::PersistedInventoryObservationOwner;
use super::super::persisted_observation_v2::{
    PersistedInventoryObservationV2Candidate, PersistedInventoryObservationV2Device,
};
use super::super::placement::DevicePlacementCandidate;
use super::super::runner::{RunnerInstance, RunnerLiveness};
use super::super::snapshot::SnapshotOwner;
use super::{
    CandidateMetadata, MAX_SAFE_INTEGER, MAX_V2_RUNTIME_COUNT, PersistedInventoryPlacementV2Error,
};

pub(super) fn build_candidate(
    candidate: &PersistedInventoryObservationV2Candidate,
    owner: &SnapshotOwner,
) -> Result<(DevicePlacementCandidate, CandidateMetadata), PersistedInventoryPlacementV2Error> {
    validate_candidate_attributes(candidate, owner)?;
    let value = &candidate.device;
    let (device_id, device) = restore_device(value)?;
    let gpus = restore_gpus(value)?;
    let capabilities = restore_capabilities(value, gpus)?;
    let runner = restore_runner(candidate, device_id, capabilities)?;
    let placement_candidate = DevicePlacementCandidate::new(device, runner)
        .map_err(|_| PersistedInventoryPlacementV2Error::InvalidCandidate)?;
    let metadata = candidate_metadata(candidate)?;
    Ok((placement_candidate, metadata))
}

fn validate_candidate_attributes(
    candidate: &PersistedInventoryObservationV2Candidate,
    owner: &SnapshotOwner,
) -> Result<(), PersistedInventoryPlacementV2Error> {
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
        || value.available_cpu_cores > super::super::model::MAX_DEVICE_CPU_CORES
        || value.gpus.len() > MAX_DEVICE_GPU_COUNT
        || value.gpus.windows(2).any(|pair| pair[0].id >= pair[1].id)
    {
        return Err(PersistedInventoryPlacementV2Error::InvalidCandidate);
    }

    Ok(())
}

fn restore_device(
    value: &PersistedInventoryObservationV2Device,
) -> Result<(DeviceId, Device), PersistedInventoryPlacementV2Error> {
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
    Ok((device_id, device))
}

fn restore_gpus(
    value: &PersistedInventoryObservationV2Device,
) -> Result<Vec<GpuCapability>, PersistedInventoryPlacementV2Error> {
    value
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
        .collect()
}

fn restore_capabilities(
    value: &PersistedInventoryObservationV2Device,
    gpus: Vec<GpuCapability>,
) -> Result<CapabilitySnapshot, PersistedInventoryPlacementV2Error> {
    // The v2 wire shape carries available values but intentionally omits the
    // total CPU/memory capacities. Use a minimal synthetic total when an
    // exhausted resource is zero; placement still compares the available
    // value and therefore reports the correct insufficiency reason.
    let cpu_capacity = value.available_cpu_cores.max(1);
    let memory_capacity = value.available_memory_bytes.max(1);
    CapabilitySnapshot::new(
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
    .map_err(|_| PersistedInventoryPlacementV2Error::InvalidCandidate)
}

fn restore_runner(
    candidate: &PersistedInventoryObservationV2Candidate,
    device_id: DeviceId,
    capabilities: CapabilitySnapshot,
) -> Result<RunnerInstance, PersistedInventoryPlacementV2Error> {
    let value = &candidate.device;
    let liveness = match value.liveness.as_str() {
        "online" => RunnerLiveness::Online,
        "offline" => RunnerLiveness::Offline,
        _ => return Err(PersistedInventoryPlacementV2Error::InvalidCandidate),
    };
    let instance_id = super::super::model::RunnerInstanceId::parse(candidate.instance_id.clone())
        .map_err(|_| PersistedInventoryPlacementV2Error::InvalidCandidate)?;
    RunnerInstance::restore(
        device_id,
        instance_id,
        candidate.generation,
        candidate.heartbeat_sequence,
        value.snapshot_observed_at_ms,
        value.lease_expires_at_ms,
        liveness,
        capabilities,
    )
    .map_err(|_| PersistedInventoryPlacementV2Error::InvalidCandidate)
}

fn candidate_metadata(
    candidate: &PersistedInventoryObservationV2Candidate,
) -> Result<CandidateMetadata, PersistedInventoryPlacementV2Error> {
    let value = &candidate.device;
    let available_gpu_memory_bytes = value.gpus.iter().try_fold(0_u64, |total, gpu| {
        total
            .checked_add(gpu.available_memory_bytes)
            .filter(|value| *value <= MAX_SAFE_INTEGER)
            .ok_or(PersistedInventoryPlacementV2Error::InvalidCandidate)
    })?;
    Ok(CandidateMetadata {
        revision: candidate.revision,
        generation: candidate.generation,
        heartbeat_sequence: candidate.heartbeat_sequence,
        instance_id: candidate.instance_id.clone(),
        reservation_state: value.reservation_state.clone(),
        gpu_count: value.gpus.len(),
        available_gpu_memory_bytes,
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
