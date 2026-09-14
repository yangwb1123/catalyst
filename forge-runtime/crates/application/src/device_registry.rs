//! Pure validation and dry-run facade for the Runtime-side device contract.
//!
//! Forge Core Go owns the authoritative inventory, identity binding, persistence,
//! and scheduling leases. This facade does not create any of those effects.

pub use crate::runtime_domain::DEVICE_HEARTBEAT_STALE_AFTER_MS;
use crate::runtime_domain::{
    Device, DeviceHeartbeatError, DevicePlacementCandidate, DevicePlacementDecision,
    DevicePlacementRequest, DevicePlacementValidationError, DeviceStateError,
    MAX_DEVICE_CAPABILITY_LEASE_TTL_MS, RunnerHeartbeat, RunnerInstance, apply_device_heartbeat,
    dry_run_device_placement,
};

#[cfg(test)]
mod tests;

pub const DEVICE_CAPABILITY_LEASE_TTL_MS: u64 = 60_000;

/// Pure application facade for the initial device registry contract.
///
/// A host adapter remains responsible for authenticating the device and atomically
/// persisting the returned heartbeat snapshot. Placement only returns a dry-run view;
/// it cannot reserve resources, dispatch work, or grant execution authority.
#[derive(Clone, Copy, Debug, Default)]
pub struct DeviceRegistryService;

impl DeviceRegistryService {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Validates and applies a heartbeat using the default bounded capability lease.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the heartbeat is stale, mismatched, or otherwise invalid.
    pub fn heartbeat(
        &self,
        device: &Device,
        current: Option<&RunnerInstance>,
        heartbeat: &RunnerHeartbeat,
        server_observed_at_ms: u64,
    ) -> Result<RunnerInstance, DeviceHeartbeatError> {
        apply_device_heartbeat(
            device,
            current,
            heartbeat,
            server_observed_at_ms,
            DEVICE_CAPABILITY_LEASE_TTL_MS,
        )
    }

    /// Applies a heartbeat with an explicitly configured, still bounded lease duration.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the heartbeat or lease duration is invalid.
    pub fn heartbeat_with_lease(
        &self,
        device: &Device,
        current: Option<&RunnerInstance>,
        heartbeat: &RunnerHeartbeat,
        server_observed_at_ms: u64,
        capability_lease_ttl_ms: u64,
    ) -> Result<RunnerInstance, DeviceHeartbeatError> {
        if capability_lease_ttl_ms > MAX_DEVICE_CAPABILITY_LEASE_TTL_MS {
            return Err(DeviceHeartbeatError::InvalidLeaseDuration);
        }
        apply_device_heartbeat(
            device,
            current,
            heartbeat,
            server_observed_at_ms,
            capability_lease_ttl_ms,
        )
    }

    /// Evaluates inventory eligibility without creating reservations or other effects.
    ///
    /// # Errors
    ///
    /// Returns an error for an oversized, duplicated, or mismatched candidate inventory.
    pub fn dry_run_placement(
        &self,
        candidates: &[DevicePlacementCandidate],
        request: &DevicePlacementRequest,
        server_now_ms: u64,
    ) -> Result<Vec<DevicePlacementDecision>, DevicePlacementValidationError> {
        dry_run_device_placement(candidates, request, server_now_ms)
    }

    /// Approves a device that is still pending.
    ///
    /// # Errors
    ///
    /// Returns an error when the device is already approved or revoked.
    pub fn approve(&self, device: &Device) -> Result<Device, DeviceStateError> {
        device.approve()
    }

    #[must_use]
    pub fn revoke(&self, device: &Device) -> Device {
        device.revoke()
    }

    #[must_use]
    pub fn cordon(&self, device: &Device) -> Device {
        device.cordon()
    }

    #[must_use]
    pub fn uncordon(&self, device: &Device) -> Device {
        device.uncordon()
    }
}
