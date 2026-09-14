use super::super::model::{Device, DeviceId, RunnerInstanceId, TenantId};
use super::super::runner::RunnerInstance;
use super::requirements::DevicePlacementRequirements;
use super::requirements::{DevicePlacementAttributes, DevicePlacementValidationError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevicePlacementRequest {
    pub(super) tenant_id: TenantId,
    pub(super) requirements: DevicePlacementRequirements,
}

impl DevicePlacementRequest {
    #[must_use]
    pub fn new(tenant_id: TenantId, requirements: DevicePlacementRequirements) -> Self {
        Self {
            tenant_id,
            requirements,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevicePlacementCandidate {
    pub(super) device: Device,
    pub(super) instance: RunnerInstance,
    pub(super) attributes: DevicePlacementAttributes,
}

impl DevicePlacementCandidate {
    /// Pairs one stable device with its active runner observation.
    ///
    /// # Errors
    ///
    /// Returns an error when the runner observation belongs to a different device.
    pub fn new(
        device: Device,
        instance: RunnerInstance,
    ) -> Result<Self, DevicePlacementValidationError> {
        if device.id() != instance.device_id() {
            return Err(DevicePlacementValidationError::CandidateDeviceMismatch);
        }
        Ok(Self {
            device,
            instance,
            attributes: DevicePlacementAttributes::default(),
        })
    }

    /// Attaches optional caller-declared placement attributes without changing the
    /// existing device/runner constructor contract.
    #[must_use]
    pub fn with_attributes(mut self, attributes: DevicePlacementAttributes) -> Self {
        self.attributes = attributes;
        self
    }

    #[must_use]
    pub fn device(&self) -> &Device {
        &self.device
    }

    #[must_use]
    pub fn instance(&self) -> &RunnerInstance {
        &self.instance
    }

    #[must_use]
    pub fn attributes(&self) -> &DevicePlacementAttributes {
        &self.attributes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DevicePlacementExclusion {
    TenantMismatch,
    ApprovalPending,
    DeviceRevoked,
    DeviceCordoned,
    RunnerOffline,
    HeartbeatObservedInFuture,
    HeartbeatStale,
    CapabilityLeaseExpired,
    CapabilityLeaseInvalid,
    OperatingSystemMismatch,
    ArchitectureMismatch,
    CpuCapacityInsufficient,
    MemoryCapacityInsufficient,
    StorageCapacityInsufficient,
    RuntimeUnavailable,
    GpuCountInsufficient,
    GpuMemoryInsufficient,
    DataResidencyZoneMismatch,
    TrustZoneUnconfirmed,
    TrustZoneBelowMinimum,
    SandboxFloorUnmet,
    ConcurrencyCapacityInsufficient,
}

impl DevicePlacementExclusion {
    /// Returns the stable lowercase reason token for deterministic reporting.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TenantMismatch => "tenant_mismatch",
            Self::ApprovalPending => "approval_pending",
            Self::DeviceRevoked => "device_revoked",
            Self::DeviceCordoned => "device_cordoned",
            Self::RunnerOffline => "runner_offline",
            Self::HeartbeatObservedInFuture => "heartbeat_observed_in_future",
            Self::HeartbeatStale => "heartbeat_stale",
            Self::CapabilityLeaseExpired => "capability_lease_expired",
            Self::CapabilityLeaseInvalid => "capability_lease_invalid",
            Self::OperatingSystemMismatch => "os_mismatch",
            Self::ArchitectureMismatch => "architecture_mismatch",
            Self::CpuCapacityInsufficient => "cpu_cores_insufficient",
            Self::MemoryCapacityInsufficient => "memory_insufficient",
            Self::StorageCapacityInsufficient => "storage_insufficient",
            Self::RuntimeUnavailable => "runtime_missing",
            Self::GpuCountInsufficient => "gpu_count_insufficient",
            Self::GpuMemoryInsufficient => "gpu_memory_insufficient",
            Self::DataResidencyZoneMismatch => "data_residency_zone_mismatch",
            Self::TrustZoneUnconfirmed => "trust_zone_unconfirmed",
            Self::TrustZoneBelowMinimum => "trust_zone_below_minimum",
            Self::SandboxFloorUnmet => "sandbox_floor_unmet",
            Self::ConcurrencyCapacityInsufficient => "concurrency_capacity_insufficient",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DevicePlacementDisposition {
    Eligible,
    Excluded(Vec<DevicePlacementExclusion>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevicePlacementDecision {
    pub(super) device_id: DeviceId,
    pub(super) instance_id: RunnerInstanceId,
    pub(super) disposition: DevicePlacementDisposition,
}

impl DevicePlacementDecision {
    #[must_use]
    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    #[must_use]
    pub fn instance_id(&self) -> &RunnerInstanceId {
        &self.instance_id
    }

    #[must_use]
    pub fn disposition(&self) -> &DevicePlacementDisposition {
        &self.disposition
    }
}
