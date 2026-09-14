use super::model::{CapabilitySnapshot, DeviceId, DeviceRegistryValidationError, RunnerInstanceId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunnerHeartbeat {
    pub(super) device_id: DeviceId,
    pub(super) instance_id: RunnerInstanceId,
    pub(super) generation: u64,
    pub(super) sequence: u64,
    pub(super) capabilities: CapabilitySnapshot,
}

impl RunnerHeartbeat {
    /// Creates an inbound heartbeat with valid, nonzero incarnation counters.
    ///
    /// # Errors
    ///
    /// Returns an error when generation or sequence is zero.
    pub fn new(
        device_id: DeviceId,
        instance_id: RunnerInstanceId,
        generation: u64,
        sequence: u64,
        capabilities: CapabilitySnapshot,
    ) -> Result<Self, DeviceRegistryValidationError> {
        if generation == 0 || sequence == 0 {
            return Err(DeviceRegistryValidationError::InvalidCapabilityValue);
        }
        Ok(Self {
            device_id,
            instance_id,
            generation,
            sequence,
            capabilities,
        })
    }

    #[must_use]
    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    #[must_use]
    pub fn instance_id(&self) -> &RunnerInstanceId {
        &self.instance_id
    }

    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    #[must_use]
    pub fn capabilities(&self) -> &CapabilitySnapshot {
        &self.capabilities
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunnerLiveness {
    Online,
    Offline,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunnerInstance {
    pub(super) device_id: DeviceId,
    pub(super) instance_id: RunnerInstanceId,
    pub(super) generation: u64,
    pub(super) heartbeat_sequence: u64,
    pub(super) server_observed_at_ms: u64,
    pub(super) capability_lease_expires_at_ms: u64,
    pub(super) liveness: RunnerLiveness,
    pub(super) capabilities: CapabilitySnapshot,
}

impl RunnerInstance {
    /// Rebuilds persisted state after checking generation, sequence, and lease bounds.
    #[allow(clippy::too_many_arguments)]
    ///
    /// # Errors
    ///
    /// Returns an error when persisted counters or the capability lease are invalid.
    pub fn restore(
        device_id: DeviceId,
        instance_id: RunnerInstanceId,
        generation: u64,
        heartbeat_sequence: u64,
        server_observed_at_ms: u64,
        capability_lease_expires_at_ms: u64,
        liveness: RunnerLiveness,
        capabilities: CapabilitySnapshot,
    ) -> Result<Self, DeviceRegistryValidationError> {
        let lease_ttl = capability_lease_expires_at_ms.checked_sub(server_observed_at_ms);
        if generation == 0
            || heartbeat_sequence == 0
            || lease_ttl.is_none_or(|ttl| {
                !(super::heartbeat::MIN_DEVICE_CAPABILITY_LEASE_TTL_MS
                    ..=super::heartbeat::MAX_DEVICE_CAPABILITY_LEASE_TTL_MS)
                    .contains(&ttl)
            })
        {
            return Err(DeviceRegistryValidationError::InvalidPersistedState);
        }
        Ok(Self {
            device_id,
            instance_id,
            generation,
            heartbeat_sequence,
            server_observed_at_ms,
            capability_lease_expires_at_ms,
            liveness,
            capabilities,
        })
    }

    #[must_use]
    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    #[must_use]
    pub fn instance_id(&self) -> &RunnerInstanceId {
        &self.instance_id
    }

    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub fn heartbeat_sequence(&self) -> u64 {
        self.heartbeat_sequence
    }

    #[must_use]
    pub fn server_observed_at_ms(&self) -> u64 {
        self.server_observed_at_ms
    }

    #[must_use]
    pub fn capability_lease_expires_at_ms(&self) -> u64 {
        self.capability_lease_expires_at_ms
    }

    #[must_use]
    pub fn liveness(&self) -> RunnerLiveness {
        self.liveness
    }

    #[must_use]
    pub fn capabilities(&self) -> &CapabilitySnapshot {
        &self.capabilities
    }

    /// Marks this observation offline without changing its last server-observed heartbeat.
    #[must_use]
    pub fn mark_offline(&self) -> Self {
        Self {
            liveness: RunnerLiveness::Offline,
            ..self.clone()
        }
    }
}
