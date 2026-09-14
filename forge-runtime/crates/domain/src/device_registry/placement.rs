use std::collections::HashSet;

use super::heartbeat::MAX_DEVICE_CAPABILITY_LEASE_TTL_MS;
use super::model::{CapabilitySnapshot, DeviceApprovalState};
use super::runner::{RunnerInstance, RunnerLiveness};

mod candidate;
mod requirements;

pub use candidate::{
    DevicePlacementCandidate, DevicePlacementDecision, DevicePlacementDisposition,
    DevicePlacementExclusion, DevicePlacementRequest,
};
pub use requirements::{
    DevicePlacementAttributes, DevicePlacementPolicy, DevicePlacementRequirements,
    DevicePlacementValidationError, DeviceSandboxLevel, DeviceTrustZone,
};

pub const MAX_DEVICE_PLACEMENT_CANDIDATES: usize = 10_000;
pub const MAX_DEVICE_PLACEMENT_POLICY_ITEMS: usize = 32;
pub const MAX_DEVICE_RESIDENCY_ZONE_BYTES: usize = 64;
pub const DEVICE_HEARTBEAT_STALE_AFTER_MS: u64 = 90_000;

/// Evaluates a bounded inventory without reserving capacity or creating execution authority.
///
/// # Errors
///
/// Returns an error for an oversized or duplicated inventory or a mismatched device pair.
pub fn dry_run_device_placement(
    candidates: &[DevicePlacementCandidate],
    request: &DevicePlacementRequest,
    server_now_ms: u64,
) -> Result<Vec<DevicePlacementDecision>, DevicePlacementValidationError> {
    validate_candidates(candidates)?;
    let mut decisions = candidates
        .iter()
        .map(|candidate| evaluate_candidate(candidate, request, server_now_ms))
        .collect::<Vec<_>>();
    decisions.sort_by(|left, right| {
        left.device_id
            .cmp(&right.device_id)
            .then_with(|| left.instance_id.cmp(&right.instance_id))
    });
    Ok(decisions)
}

fn validate_candidates(
    candidates: &[DevicePlacementCandidate],
) -> Result<(), DevicePlacementValidationError> {
    if candidates.len() > MAX_DEVICE_PLACEMENT_CANDIDATES {
        return Err(DevicePlacementValidationError::TooManyCandidates);
    }
    let mut device_ids = HashSet::with_capacity(candidates.len());
    let mut instance_ids = HashSet::with_capacity(candidates.len());
    for candidate in candidates {
        if !device_ids.insert(candidate.device.id()) {
            return Err(DevicePlacementValidationError::DuplicateDevice);
        }
        if !instance_ids.insert(candidate.instance.instance_id()) {
            return Err(DevicePlacementValidationError::DuplicateInstance);
        }
    }
    Ok(())
}

fn evaluate_candidate(
    candidate: &DevicePlacementCandidate,
    request: &DevicePlacementRequest,
    server_now_ms: u64,
) -> DevicePlacementDecision {
    let mut reasons = Vec::new();
    let device = &candidate.device;
    let instance = &candidate.instance;
    let capabilities = instance.capabilities();
    if device.tenant_id() != &request.tenant_id {
        reasons.push(DevicePlacementExclusion::TenantMismatch);
    }
    match device.approval() {
        DeviceApprovalState::Pending => reasons.push(DevicePlacementExclusion::ApprovalPending),
        DeviceApprovalState::Approved => {}
        DeviceApprovalState::Revoked => reasons.push(DevicePlacementExclusion::DeviceRevoked),
    }
    if device.is_cordoned() {
        reasons.push(DevicePlacementExclusion::DeviceCordoned);
    }
    if instance.liveness() == RunnerLiveness::Offline {
        reasons.push(DevicePlacementExclusion::RunnerOffline);
    }
    if instance.server_observed_at_ms() > server_now_ms {
        reasons.push(DevicePlacementExclusion::HeartbeatObservedInFuture);
    } else if server_now_ms - instance.server_observed_at_ms() > DEVICE_HEARTBEAT_STALE_AFTER_MS {
        reasons.push(DevicePlacementExclusion::HeartbeatStale);
    }
    validate_capability_lease(instance, server_now_ms, &mut reasons);
    evaluate_requirements(capabilities, &request.requirements, &mut reasons);
    evaluate_policy(
        &candidate.attributes,
        &request.requirements.policy,
        &mut reasons,
    );
    reasons.sort_by_key(|reason| reason.as_str());
    let disposition = if reasons.is_empty() {
        DevicePlacementDisposition::Eligible
    } else {
        DevicePlacementDisposition::Excluded(reasons)
    };
    DevicePlacementDecision {
        device_id: device.id().clone(),
        instance_id: instance.instance_id().clone(),
        disposition,
    }
}

fn evaluate_policy(
    attributes: &DevicePlacementAttributes,
    policy: &DevicePlacementPolicy,
    reasons: &mut Vec<DevicePlacementExclusion>,
) {
    if policy
        .allowed_data_residency_zones
        .as_ref()
        .is_some_and(|allowed| {
            !allowed
                .iter()
                .any(|zone| attributes.data_residency_zones.binary_search(zone).is_ok())
        })
    {
        reasons.push(DevicePlacementExclusion::DataResidencyZoneMismatch);
    }
    if let Some(minimum) = policy.minimum_trust_zone {
        match (attributes.trust_zone.rank(), minimum.rank()) {
            (None, _) => reasons.push(DevicePlacementExclusion::TrustZoneUnconfirmed),
            (Some(actual), Some(minimum)) if actual < minimum => {
                reasons.push(DevicePlacementExclusion::TrustZoneBelowMinimum);
            }
            (Some(_), Some(_)) => {}
            (_, None) => unreachable!("policy minimum trust zone is validated"),
        }
    }
    if policy.sandbox_floor.is_some_and(|floor| {
        !attributes
            .sandbox_levels
            .iter()
            .any(|level| level.rank() >= floor.rank())
    }) {
        reasons.push(DevicePlacementExclusion::SandboxFloorUnmet);
    }
    if policy.concurrency_slots.is_some_and(|slots| {
        attributes.active_concurrency > attributes.concurrency_limit
            || slots > attributes.concurrency_limit - attributes.active_concurrency
    }) {
        reasons.push(DevicePlacementExclusion::ConcurrencyCapacityInsufficient);
    }
}

fn validate_capability_lease(
    instance: &RunnerInstance,
    server_now_ms: u64,
    reasons: &mut Vec<DevicePlacementExclusion>,
) {
    if instance.capability_lease_expires_at_ms() <= server_now_ms {
        reasons.push(DevicePlacementExclusion::CapabilityLeaseExpired);
    }
    let lease_ttl = instance
        .capability_lease_expires_at_ms()
        .checked_sub(instance.server_observed_at_ms());
    if lease_ttl.is_none_or(|ttl| ttl > MAX_DEVICE_CAPABILITY_LEASE_TTL_MS) {
        reasons.push(DevicePlacementExclusion::CapabilityLeaseInvalid);
    }
}

fn evaluate_requirements(
    capabilities: &CapabilitySnapshot,
    requirements: &DevicePlacementRequirements,
    reasons: &mut Vec<DevicePlacementExclusion>,
) {
    if requirements
        .operating_system
        .as_deref()
        .is_some_and(|os| os != capabilities.operating_system())
    {
        reasons.push(DevicePlacementExclusion::OperatingSystemMismatch);
    }
    if requirements
        .architecture
        .as_deref()
        .is_some_and(|arch| arch != capabilities.architecture())
    {
        reasons.push(DevicePlacementExclusion::ArchitectureMismatch);
    }
    if capabilities.available_cpu_cores() < requirements.minimum_cpu_cores {
        reasons.push(DevicePlacementExclusion::CpuCapacityInsufficient);
    }
    if capabilities.available_memory_bytes() < requirements.minimum_memory_bytes {
        reasons.push(DevicePlacementExclusion::MemoryCapacityInsufficient);
    }
    if capabilities.available_storage_bytes() < requirements.minimum_storage_bytes {
        reasons.push(DevicePlacementExclusion::StorageCapacityInsufficient);
    }
    if !requirements
        .required_runtimes
        .iter()
        .all(|runtime| capabilities.runtimes().binary_search(runtime).is_ok())
    {
        reasons.push(DevicePlacementExclusion::RuntimeUnavailable);
    }
    if capabilities.gpus().len() < requirements.minimum_gpu_count {
        reasons.push(DevicePlacementExclusion::GpuCountInsufficient);
    } else if capabilities
        .gpus()
        .iter()
        .filter(|gpu| gpu.available_memory_bytes() >= requirements.minimum_gpu_memory_bytes)
        .count()
        < requirements.minimum_gpu_count
    {
        reasons.push(DevicePlacementExclusion::GpuMemoryInsufficient);
    }
}
