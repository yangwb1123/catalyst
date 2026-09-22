use super::heartbeat::apply_device_heartbeat;
use super::model::{Device, DeviceApprovalState, DeviceId, RunnerInstanceId};
use super::runner::{RunnerHeartbeat, RunnerInstance, RunnerLiveness};
use super::snapshot::SnapshotOwner;
use super::status::{InventoryStatus, project_inventory_status};

/// A value-level compare-and-swap state for the future heartbeat transaction.
/// It carries no database handle and does not perform a write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedRunnerInstance {
    revision: u64,
    instance: RunnerInstance,
}

impl PersistedRunnerInstance {
    /// Rebuilds a nonzero revision from a bounded persisted observation.
    ///
    /// # Errors
    ///
    /// Returns an error when a storage row uses revision zero.
    pub fn restore(revision: u64, instance: RunnerInstance) -> Result<Self, PersistenceError> {
        if revision == 0 {
            return Err(PersistenceError::InvalidPersistedState);
        }
        Ok(Self { revision, instance })
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn instance(&self) -> &RunnerInstance {
        &self.instance
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistenceError {
    RevisionConflict,
    InvalidPersistedState,
    RevisionOverflow,
    InvalidDeviceRecord,
    InvalidRunnerRecord,
    RunnerDeviceMismatch,
    InventoryOwnerMismatch,
    DeviceBindingChanged,
    InvalidEvaluationOwner,
    InventoryStatus(super::status::InventoryStatusError),
    DeviceHeartbeat(super::heartbeat::DeviceHeartbeatError),
}

impl std::fmt::Display for PersistenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::RevisionConflict => "revision_conflict",
            Self::InvalidPersistedState => "invalid_persisted_state",
            Self::RevisionOverflow => "revision_overflow",
            Self::InvalidDeviceRecord => "invalid_device_record",
            Self::InvalidRunnerRecord => "invalid_runner_record",
            Self::RunnerDeviceMismatch => "runner_device_mismatch",
            Self::InventoryOwnerMismatch => "owner_mismatch",
            Self::DeviceBindingChanged => "device_binding_changed",
            Self::InvalidEvaluationOwner => "invalid_evaluation_owner",
            Self::InventoryStatus(error) => return error.fmt(formatter),
            Self::DeviceHeartbeat(error) => match error {
                super::heartbeat::DeviceHeartbeatError::DeviceMismatch => "device_mismatch",
                super::heartbeat::DeviceHeartbeatError::DeviceRevoked => "device_revoked",
                super::heartbeat::DeviceHeartbeatError::GenerationMustStartAtOne => {
                    "generation_must_start_at_one"
                }
                super::heartbeat::DeviceHeartbeatError::OldGeneration => "old_generation",
                super::heartbeat::DeviceHeartbeatError::GenerationSkipped => "generation_skipped",
                super::heartbeat::DeviceHeartbeatError::InstanceChangedWithinGeneration => {
                    "instance_changed_within_generation"
                }
                super::heartbeat::DeviceHeartbeatError::SequenceMustStartAtOne => {
                    "sequence_must_start_at_one"
                }
                super::heartbeat::DeviceHeartbeatError::SequenceNotIncreasing => {
                    "sequence_not_increasing"
                }
                super::heartbeat::DeviceHeartbeatError::ServerTimeWentBackwards => {
                    "server_time_went_backwards"
                }
                super::heartbeat::DeviceHeartbeatError::InvalidLeaseDuration => {
                    "invalid_lease_duration"
                }
                super::heartbeat::DeviceHeartbeatError::LeaseExpiryOverflow => {
                    "lease_expiry_overflow"
                }
            },
        })
    }
}

impl std::error::Error for PersistenceError {}

/// A complete owner-bound device value for the future inventory transaction.
/// The owner tuple is still caller-supplied and unverified at this boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedInventoryDevice {
    device: Device,
    owner: SnapshotOwner,
    reserved: bool,
}

impl PersistedInventoryDevice {
    #[must_use]
    pub fn restore(device: Device, owner: SnapshotOwner, reserved: bool) -> Self {
        Self {
            device,
            owner,
            reserved,
        }
    }

    #[must_use]
    pub fn device(&self) -> &Device {
        &self.device
    }

    #[must_use]
    pub fn owner(&self) -> &SnapshotOwner {
        &self.owner
    }

    #[must_use]
    pub fn reserved(&self) -> bool {
        self.reserved
    }
}

/// A complete persisted inventory value. It contains no storage handle,
/// clock, listener, credential, reservation, or execution authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedInventoryState {
    revision: u64,
    device: PersistedInventoryDevice,
    runner: RunnerInstance,
}

impl PersistedInventoryState {
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn device(&self) -> &PersistedInventoryDevice {
        &self.device
    }

    #[must_use]
    pub fn runner(&self) -> &RunnerInstance {
        &self.runner
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedInventoryProjection {
    pub revision: u64,
    pub device_id: DeviceId,
    pub instance_id: RunnerInstanceId,
    pub status: InventoryStatus,
    pub fresh: bool,
    pub declared_eligible: bool,
}

impl PersistedInventoryProjection {
    #[must_use]
    pub fn status(&self) -> InventoryStatus {
        self.status
    }
}

/// Rebuilds a complete owner-bound inventory value from already parsed data.
/// This function performs no I/O and does not verify the owner tuple.
///
/// # Errors
///
/// Returns [`PersistenceError::InvalidPersistedState`] for revision zero,
/// [`PersistenceError::InvalidDeviceRecord`] for an invalid owner binding, or
/// [`PersistenceError::RunnerDeviceMismatch`] for a foreign runner.
pub fn restore_persisted_inventory(
    revision: u64,
    device: PersistedInventoryDevice,
    runner: RunnerInstance,
) -> Result<PersistedInventoryState, PersistenceError> {
    if revision == 0 {
        return Err(PersistenceError::InvalidPersistedState);
    }
    let state = PersistedInventoryState {
        revision,
        device,
        runner,
    };
    validate_persisted_inventory(&state)?;
    Ok(state)
}

/// Evaluates one complete replacement using an exact revision. The returned
/// value is a replacement plan; no write or listener is performed.
///
/// # Errors
///
/// Returns a revision conflict or overflow, binding error, or invalid
/// persisted state when the compare-and-swap precondition cannot be accepted.
pub fn commit_persisted_inventory(
    current: Option<&PersistedInventoryState>,
    expected_revision: u64,
    device: PersistedInventoryDevice,
    runner: RunnerInstance,
) -> Result<PersistedInventoryState, PersistenceError> {
    if let Some(current) = current {
        validate_persisted_inventory(current)?;
    }
    let actual_revision = current.map_or(0, PersistedInventoryState::revision);
    if expected_revision != actual_revision {
        return Err(PersistenceError::RevisionConflict);
    }
    let next_revision = actual_revision
        .checked_add(1)
        .ok_or(PersistenceError::RevisionOverflow)?;
    if let Some(current) = current
        && (device.device().id() != current.device.device().id()
            || device.owner() != current.device.owner()
            || runner.device_id() != current.runner.device_id())
    {
        return Err(PersistenceError::DeviceBindingChanged);
    }
    let next = PersistedInventoryState {
        revision: next_revision,
        device,
        runner,
    };
    validate_persisted_inventory(&next)?;
    Ok(next)
}

/// Projects a persisted value at an explicit evaluation time. It is a
/// display-only result and never grants inventory or execution authority.
///
/// # Errors
///
/// Returns an owner or persisted-state error, or propagates the fixed-time
/// inventory status validation error.
pub fn project_persisted_inventory(
    value: &PersistedInventoryState,
    evaluation_owner: &SnapshotOwner,
    evaluated_at_ms: u64,
    stale_after_ms: u64,
) -> Result<PersistedInventoryProjection, PersistenceError> {
    validate_persisted_inventory(value)?;
    if !valid_owner(evaluation_owner) {
        return Err(PersistenceError::InvalidEvaluationOwner);
    }
    if value.device.owner() != evaluation_owner {
        return Err(PersistenceError::InventoryOwnerMismatch);
    }
    let status = project_inventory_status(
        &super::status::InventoryStatusObservation {
            approval_state: approval_state_string(value.device.device().approval()),
            cordon_state: if value.device.device().is_cordoned() {
                "cordoned".to_owned()
            } else {
                "clear".to_owned()
            },
            liveness: liveness_string(value.runner.liveness()),
            reservation_state: if value.device.reserved() {
                "reserved".to_owned()
            } else {
                "none".to_owned()
            },
            snapshot_observed_at_ms: value.runner.server_observed_at_ms(),
            lease_expires_at_ms: value.runner.capability_lease_expires_at_ms(),
            evaluated_at_ms,
        },
        stale_after_ms,
    )
    .map_err(PersistenceError::InventoryStatus)?;
    Ok(PersistedInventoryProjection {
        revision: value.revision,
        device_id: value.device.device().id().clone(),
        instance_id: value.runner.instance_id().clone(),
        status: status.status,
        fresh: status.fresh,
        declared_eligible: status.declared_eligible,
    })
}

pub(crate) fn validate_persisted_inventory(
    value: &PersistedInventoryState,
) -> Result<(), PersistenceError> {
    if value.revision == 0 {
        return Err(PersistenceError::InvalidPersistedState);
    }
    if !valid_owner(value.device.owner())
        || value.device.owner().tenant_id != value.device.device().tenant_id().as_str()
    {
        return Err(PersistenceError::InvalidDeviceRecord);
    }
    if value.runner.device_id() != value.device.device().id() {
        return Err(PersistenceError::RunnerDeviceMismatch);
    }
    Ok(())
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

fn approval_state_string(value: DeviceApprovalState) -> String {
    match value {
        DeviceApprovalState::Approved => "approved",
        DeviceApprovalState::Pending => "pending",
        DeviceApprovalState::Revoked => "revoked",
    }
    .to_owned()
}

fn liveness_string(value: RunnerLiveness) -> String {
    match value {
        RunnerLiveness::Online => "online",
        RunnerLiveness::Offline => "offline",
    }
    .to_owned()
}

/// Applies a heartbeat only when the expected revision names the supplied
/// snapshot. The returned value is the complete replacement for one future
/// atomic transaction; no storage, clock, retry, or listener is involved.
///
/// # Errors
///
/// Returns a revision, persisted-state, overflow, or heartbeat validation
/// error when the supplied compare-and-swap precondition cannot be accepted.
pub fn commit_device_heartbeat(
    device: &Device,
    current: Option<&PersistedRunnerInstance>,
    expected_revision: u64,
    heartbeat: &RunnerHeartbeat,
    server_observed_at_ms: u64,
    lease_ttl_ms: u64,
) -> Result<PersistedRunnerInstance, PersistenceError> {
    if current.is_some_and(|state| state.revision == 0) {
        return Err(PersistenceError::InvalidPersistedState);
    }
    let actual_revision = current.map_or(0, PersistedRunnerInstance::revision);
    if expected_revision != actual_revision {
        return Err(PersistenceError::RevisionConflict);
    }
    let next_revision = actual_revision
        .checked_add(1)
        .ok_or(PersistenceError::RevisionOverflow)?;
    let next_instance = apply_device_heartbeat(
        device,
        current.map(PersistedRunnerInstance::instance),
        heartbeat,
        server_observed_at_ms,
        lease_ttl_ms,
    )
    .map_err(PersistenceError::DeviceHeartbeat)?;
    Ok(PersistedRunnerInstance {
        revision: next_revision,
        instance: next_instance,
    })
}
