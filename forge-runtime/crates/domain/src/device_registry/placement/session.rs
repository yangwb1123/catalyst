use std::fmt;

use super::super::MAX_DEVICE_IDENTIFIER_BYTES;
use super::super::model::TenantId;
use super::{
    DevicePlacementCandidate, DevicePlacementDisposition, DevicePlacementExclusion,
    DevicePlacementRequest, dry_run_device_placement,
};

pub const SESSION_PLACEMENT_OBSERVATION_SCHEMA_VERSION: &str =
    "forge.session-placement-observation/v1";

/// Caller-declared owner metadata attached to one shared Conversation Run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionPlacementOwner {
    pub issuer: String,
    pub subject: String,
    pub tenant_id: TenantId,
}

/// Input to the pure session placement observer. Candidates and owner fields
/// are declarations; they do not establish identity or inventory authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionPlacementObservationRequest {
    pub owner: SessionPlacementOwner,
    /// The owner tuple declared by the embedded placement request.
    pub placement_owner: SessionPlacementOwner,
    pub conversation_id: String,
    pub run_id: String,
    pub placement: DevicePlacementRequest,
    pub candidates: Vec<DevicePlacementCandidate>,
    pub evaluated_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionPlacementDecision {
    pub device_id: String,
    pub instance_id: String,
    pub matches_requirements: bool,
    pub exclusion_reasons: Vec<String>,
}

/// Authority bits are deliberately all false for this offline observation.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SessionPlacementAuthority {
    pub identity_verified: bool,
    pub heartbeat_persisted: bool,
    pub inventory_authoritative: bool,
    pub reservation_created: bool,
    pub execution_authorized: bool,
    pub dispatch_performed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionPlacementObservation {
    pub schema_version: &'static str,
    pub evaluation_mode: &'static str,
    pub owner: SessionPlacementOwner,
    pub conversation_id: String,
    pub run_id: String,
    pub evaluated_at_ms: u64,
    pub owner_declaration_unverified: bool,
    pub device_attributes_unverified: bool,
    pub decisions: Vec<SessionPlacementDecision>,
    pub selected_device_id: Option<String>,
    pub selected_instance_id: Option<String>,
    pub authority: SessionPlacementAuthority,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionPlacementObservationError {
    InvalidBinding,
    Placement(super::DevicePlacementValidationError),
}

impl fmt::Display for SessionPlacementObservationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "session placement observation is invalid: {self:?}"
        )
    }
}

impl std::error::Error for SessionPlacementObservationError {}

/// Evaluates one owner/conversation/run binding without selecting or assigning
/// a target. The supplied timestamp is the only evaluation time input.
///
/// # Errors
///
/// Returns an error when the owner/session binding or placement candidates are
/// invalid under the pure placement contract.
pub fn observe_session_placement(
    input: SessionPlacementObservationRequest,
) -> Result<SessionPlacementObservation, SessionPlacementObservationError> {
    if !valid_owner(&input.owner)
        || !valid_identifier(&input.conversation_id)
        || !valid_identifier(&input.run_id)
        || input.owner != input.placement_owner
        || input.placement_owner.tenant_id != *input.placement.tenant_id()
    {
        return Err(SessionPlacementObservationError::InvalidBinding);
    }
    let decisions =
        dry_run_device_placement(&input.candidates, &input.placement, input.evaluated_at_ms)
            .map_err(SessionPlacementObservationError::Placement)?;
    let decisions = decisions
        .into_iter()
        .map(|decision| {
            let (matches_requirements, exclusion_reasons) = match decision.disposition() {
                DevicePlacementDisposition::Eligible => (true, Vec::new()),
                DevicePlacementDisposition::Excluded(reasons) => (
                    false,
                    reasons
                        .iter()
                        .map(|reason| exclusion_name(*reason).to_owned())
                        .collect(),
                ),
            };
            SessionPlacementDecision {
                device_id: decision.device_id().as_str().to_owned(),
                instance_id: decision.instance_id().as_str().to_owned(),
                matches_requirements,
                exclusion_reasons,
            }
        })
        .collect();
    Ok(SessionPlacementObservation {
        schema_version: SESSION_PLACEMENT_OBSERVATION_SCHEMA_VERSION,
        evaluation_mode: "offline_static_only",
        owner: input.owner,
        conversation_id: input.conversation_id,
        run_id: input.run_id,
        evaluated_at_ms: input.evaluated_at_ms,
        owner_declaration_unverified: true,
        device_attributes_unverified: true,
        decisions,
        selected_device_id: None,
        selected_instance_id: None,
        authority: SessionPlacementAuthority::default(),
    })
}

fn valid_owner(owner: &SessionPlacementOwner) -> bool {
    valid_owner_part(&owner.issuer) && valid_owner_part(&owner.subject)
}

fn valid_owner_part(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn valid_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= MAX_DEVICE_IDENTIFIER_BYTES
        && bytes[0].is_ascii_alphanumeric()
        && bytes.iter().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'+' | b'/' | b'-')
        })
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
