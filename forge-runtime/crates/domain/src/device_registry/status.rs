use std::fmt;

pub const DEFAULT_STALE_AFTER_MS: u64 = 90_000;
pub const MAX_STALE_AFTER_MS: u64 = 24 * 60 * 60 * 1000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InventoryStatus {
    Online,
    Pending,
    Stale,
    Offline,
    Reserved,
    Cordoned,
    Revoked,
}

impl InventoryStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::Pending => "pending",
            Self::Stale => "stale",
            Self::Offline => "offline",
            Self::Reserved => "reserved",
            Self::Cordoned => "cordoned",
            Self::Revoked => "revoked",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InventoryStatusObservation {
    pub approval_state: String,
    pub cordon_state: String,
    pub liveness: String,
    pub reservation_state: String,
    pub snapshot_observed_at_ms: u64,
    pub lease_expires_at_ms: u64,
    pub evaluated_at_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InventoryStatusProjection {
    pub status: InventoryStatus,
    pub fresh: bool,
    pub declared_eligible: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InventoryStatusError {
    InvalidEvaluationTime,
    InvalidStaleAfter,
    SnapshotFromFuture,
    LeaseBeforeSnapshot,
    UnknownApproval,
    UnknownCordon,
    UnknownLiveness,
    UnknownReservation,
}

impl fmt::Display for InventoryStatusError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidEvaluationTime => "invalid_evaluation_time",
            Self::InvalidStaleAfter => "invalid_stale_after",
            Self::SnapshotFromFuture => "snapshot_from_future",
            Self::LeaseBeforeSnapshot => "lease_before_snapshot",
            Self::UnknownApproval => "unknown_approval",
            Self::UnknownCordon => "unknown_cordon",
            Self::UnknownLiveness => "unknown_liveness",
            Self::UnknownReservation => "unknown_reservation",
        })
    }
}

impl std::error::Error for InventoryStatusError {}

/// Projects one declared row at a fixed evaluation time. The output is for
/// display and offline comparison; it never authorizes a reservation or task.
///
/// # Errors
///
/// Returns an error when the evaluation time, freshness bound, lease window,
/// or declared state vocabulary is invalid.
pub fn project_inventory_status(
    observation: &InventoryStatusObservation,
    stale_after_ms: u64,
) -> Result<InventoryStatusProjection, InventoryStatusError> {
    if observation.evaluated_at_ms == 0 {
        return Err(InventoryStatusError::InvalidEvaluationTime);
    }
    if stale_after_ms == 0 || stale_after_ms > MAX_STALE_AFTER_MS {
        return Err(InventoryStatusError::InvalidStaleAfter);
    }
    if observation.snapshot_observed_at_ms > observation.evaluated_at_ms {
        return Err(InventoryStatusError::SnapshotFromFuture);
    }
    if observation.lease_expires_at_ms < observation.snapshot_observed_at_ms {
        return Err(InventoryStatusError::LeaseBeforeSnapshot);
    }
    if !matches!(
        observation.approval_state.as_str(),
        "approved" | "pending" | "revoked"
    ) {
        return Err(InventoryStatusError::UnknownApproval);
    }
    if !matches!(observation.cordon_state.as_str(), "clear" | "cordoned") {
        return Err(InventoryStatusError::UnknownCordon);
    }
    if !matches!(observation.liveness.as_str(), "online" | "offline") {
        return Err(InventoryStatusError::UnknownLiveness);
    }
    if !matches!(observation.reservation_state.as_str(), "none" | "reserved") {
        return Err(InventoryStatusError::UnknownReservation);
    }
    let age = observation.evaluated_at_ms - observation.snapshot_observed_at_ms;
    let fresh =
        age <= stale_after_ms && observation.lease_expires_at_ms > observation.evaluated_at_ms;
    let status = if observation.approval_state == "revoked" {
        InventoryStatus::Revoked
    } else if observation.cordon_state == "cordoned" {
        InventoryStatus::Cordoned
    } else if observation.liveness == "offline" {
        InventoryStatus::Offline
    } else if !fresh {
        InventoryStatus::Stale
    } else if observation.approval_state == "pending" {
        InventoryStatus::Pending
    } else if observation.reservation_state == "reserved" {
        InventoryStatus::Reserved
    } else {
        InventoryStatus::Online
    };
    Ok(InventoryStatusProjection {
        declared_eligible: status == InventoryStatus::Online,
        status,
        fresh,
    })
}
