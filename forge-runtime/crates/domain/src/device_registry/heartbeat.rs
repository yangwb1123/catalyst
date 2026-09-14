use std::fmt;

use super::model::{Device, DeviceApprovalState};
use super::runner::{RunnerHeartbeat, RunnerInstance, RunnerLiveness};

pub const MIN_DEVICE_CAPABILITY_LEASE_TTL_MS: u64 = 1_000;
pub const MAX_DEVICE_CAPABILITY_LEASE_TTL_MS: u64 = 600_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceHeartbeatError {
    DeviceMismatch,
    DeviceRevoked,
    GenerationMustStartAtOne,
    OldGeneration,
    GenerationSkipped,
    InstanceChangedWithinGeneration,
    SequenceMustStartAtOne,
    SequenceNotIncreasing,
    ServerTimeWentBackwards,
    InvalidLeaseDuration,
    LeaseExpiryOverflow,
}

impl fmt::Display for DeviceHeartbeatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "device heartbeat rejected: {self:?}")
    }
}

impl std::error::Error for DeviceHeartbeatError {}

/// Applies a validated heartbeat using server-owned observation and expiry times.
///
/// A new process incarnation must advance the generation by exactly one and start at
/// sequence one. Within an incarnation, sequence numbers must strictly increase. The
/// returned value is a new snapshot; this function does not persist or publish it.
///
/// # Errors
///
/// Returns an error for a mismatched or revoked device, invalid lease, stale generation
/// or sequence, or server-time regression.
pub fn apply_device_heartbeat(
    device: &Device,
    current: Option<&RunnerInstance>,
    heartbeat: &RunnerHeartbeat,
    server_observed_at_ms: u64,
    capability_lease_ttl_ms: u64,
) -> Result<RunnerInstance, DeviceHeartbeatError> {
    if heartbeat.device_id != *device.id() {
        return Err(DeviceHeartbeatError::DeviceMismatch);
    }
    if device.approval() == DeviceApprovalState::Revoked {
        return Err(DeviceHeartbeatError::DeviceRevoked);
    }
    if !(MIN_DEVICE_CAPABILITY_LEASE_TTL_MS..=MAX_DEVICE_CAPABILITY_LEASE_TTL_MS)
        .contains(&capability_lease_ttl_ms)
    {
        return Err(DeviceHeartbeatError::InvalidLeaseDuration);
    }
    validate_incarnation(current, heartbeat, device, server_observed_at_ms)?;
    let capability_lease_expires_at_ms = server_observed_at_ms
        .checked_add(capability_lease_ttl_ms)
        .ok_or(DeviceHeartbeatError::LeaseExpiryOverflow)?;
    Ok(RunnerInstance {
        device_id: heartbeat.device_id.clone(),
        instance_id: heartbeat.instance_id.clone(),
        generation: heartbeat.generation,
        heartbeat_sequence: heartbeat.sequence,
        server_observed_at_ms,
        capability_lease_expires_at_ms,
        liveness: RunnerLiveness::Online,
        capabilities: heartbeat.capabilities.clone(),
    })
}

fn validate_incarnation(
    current: Option<&RunnerInstance>,
    heartbeat: &RunnerHeartbeat,
    device: &Device,
    server_now_ms: u64,
) -> Result<(), DeviceHeartbeatError> {
    let Some(current) = current else {
        if heartbeat.generation != 1 {
            return Err(DeviceHeartbeatError::GenerationMustStartAtOne);
        }
        return (heartbeat.sequence == 1)
            .then_some(())
            .ok_or(DeviceHeartbeatError::SequenceMustStartAtOne);
    };
    if current.device_id != *device.id() {
        return Err(DeviceHeartbeatError::DeviceMismatch);
    }
    if server_now_ms < current.server_observed_at_ms {
        return Err(DeviceHeartbeatError::ServerTimeWentBackwards);
    }
    if heartbeat.generation < current.generation {
        return Err(DeviceHeartbeatError::OldGeneration);
    }
    if heartbeat.generation == current.generation {
        if heartbeat.instance_id != current.instance_id {
            return Err(DeviceHeartbeatError::InstanceChangedWithinGeneration);
        }
        return (heartbeat.sequence > current.heartbeat_sequence)
            .then_some(())
            .ok_or(DeviceHeartbeatError::SequenceNotIncreasing);
    }
    match current.generation.checked_add(1) {
        Some(next) if heartbeat.generation == next => {}
        _ => return Err(DeviceHeartbeatError::GenerationSkipped),
    }
    (heartbeat.sequence == 1)
        .then_some(())
        .ok_or(DeviceHeartbeatError::SequenceMustStartAtOne)
}
