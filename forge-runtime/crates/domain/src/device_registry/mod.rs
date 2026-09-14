//! Bounded reference contracts for Runner heartbeats and placement eligibility.
//!
//! This module is a pure model only. It does not persist the coordinator's
//! authoritative device registry or grant dispatch authority; Forge Core Go
//! remains the owner of that state and its leases.

mod heartbeat;
mod model;
mod placement;
mod runner;

pub use heartbeat::{
    DeviceHeartbeatError, MAX_DEVICE_CAPABILITY_LEASE_TTL_MS, MIN_DEVICE_CAPABILITY_LEASE_TTL_MS,
    apply_device_heartbeat,
};
pub use model::{
    CapabilitySnapshot, Device, DeviceApprovalState, DeviceId, DeviceRegistryValidationError,
    DeviceStateError, GpuCapability, MAX_DEVICE_CAPABILITY_BYTES, MAX_DEVICE_CPU_CORES,
    MAX_DEVICE_GPU_COUNT, MAX_DEVICE_IDENTIFIER_BYTES, MAX_DEVICE_RUNTIME_COUNT,
    MAX_DEVICE_RUNTIME_NAME_BYTES, RunnerInstanceId, TenantId,
};
pub use placement::{
    DEVICE_HEARTBEAT_STALE_AFTER_MS, DevicePlacementAttributes, DevicePlacementCandidate,
    DevicePlacementDecision, DevicePlacementDisposition, DevicePlacementExclusion,
    DevicePlacementPolicy, DevicePlacementRequest, DevicePlacementRequirements,
    DevicePlacementValidationError, DeviceSandboxLevel, DeviceTrustZone,
    MAX_DEVICE_PLACEMENT_CANDIDATES, MAX_DEVICE_PLACEMENT_POLICY_ITEMS,
    MAX_DEVICE_RESIDENCY_ZONE_BYTES, dry_run_device_placement,
};
pub use runner::{RunnerHeartbeat, RunnerInstance, RunnerLiveness};

#[cfg(test)]
mod tests;
