use std::collections::{BTreeMap, HashSet};

use super::heartbeat::MAX_DEVICE_CAPABILITY_LEASE_TTL_MS;
use super::model::{CapabilitySnapshot, DeviceApprovalState};
use super::persistence::{PersistedInventoryState, PersistenceError, validate_persisted_inventory};
use super::runner::{RunnerInstance, RunnerLiveness};
use super::snapshot::SnapshotOwner;

mod candidate;
mod requirements;
mod resource_summary;
mod run_intent;
mod session;

pub use candidate::{
    DevicePlacementCandidate, DevicePlacementDecision, DevicePlacementDisposition,
    DevicePlacementExclusion, DevicePlacementRequest,
};
pub use requirements::{
    DevicePlacementAttributes, DevicePlacementPolicy, DevicePlacementRequirements,
    DevicePlacementValidationError, DeviceSandboxLevel, DeviceTrustZone,
};
pub use resource_summary::{
    DEVICE_RESOURCE_SUMMARY_NOTICE, DEVICE_RESOURCE_SUMMARY_SCHEMA_VERSION, DeviceResourceSummary,
    DeviceResourceSummaryError, DeviceResourceSummaryOwner, DeviceResourceSummaryRequest,
    observe_device_resource_summary,
};
pub use run_intent::{
    MAX_RUN_INTENT_SAFE_INTEGER, RUN_INTENT_OBSERVATION_SCHEMA_VERSION, RunIntentObservation,
    RunIntentObservationError, RunIntentObservationRequest, RunIntentPromptReceipt,
    RunIntentRunReference, observe_run_intent,
};
pub use session::{
    SESSION_PLACEMENT_OBSERVATION_SCHEMA_VERSION, SessionPlacementAuthority,
    SessionPlacementDecision, SessionPlacementObservation, SessionPlacementObservationError,
    SessionPlacementObservationRequest, SessionPlacementOwner, observe_session_placement,
};

pub const MAX_DEVICE_PLACEMENT_CANDIDATES: usize = 10_000;
pub const MAX_PERSISTED_INVENTORY_PLACEMENT_CANDIDATES: usize = 128;
pub const MAX_DEVICE_PLACEMENT_POLICY_ITEMS: usize = 32;
pub const MAX_DEVICE_RESIDENCY_ZONE_BYTES: usize = 64;
pub const DEVICE_HEARTBEAT_STALE_AFTER_MS: u64 = 90_000;
const MAX_SAFE_INTEGER_MS: u64 = 9_007_199_254_740_991;
pub const PERSISTED_INVENTORY_PLACEMENT_EVALUATION_SCHEMA_VERSION: &str =
    "forge.persisted-inventory-placement-evaluation/v1";
pub const PERSISTED_INVENTORY_PLACEMENT_EVALUATION_MODE: &str =
    "pure_persisted_inventory_placement_dry_run";

/// A value-only placement input copied from one persisted inventory state.
/// The owner tuple and capabilities are unverified declarations. Persisted
/// inventory has no residency, trust, sandbox, or concurrency fields, so the
/// candidate's attributes are the closed defaults: empty residency/sandbox,
/// unknown trust, and zero concurrency.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedInventoryPlacementInput {
    revision: u64,
    owner: SnapshotOwner,
    candidate: DevicePlacementCandidate,
    reserved: bool,
    owner_declaration_unverified: bool,
    policy_attributes_unverified: bool,
}

/// One offline policy evaluation over a persisted inventory placement input.
/// The result keeps the source revision and Runner instance visible while
/// exposing no target selection or execution authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedInventoryPlacementEvaluation {
    pub revision: u64,
    pub device_id: String,
    pub instance_id: String,
    pub matches_requirements: bool,
    pub exclusion_reasons: Vec<String>,
    pub owner_declaration_unverified: bool,
    pub device_attributes_unverified: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PersistedInventoryPlacementEvaluationError {
    OwnerMismatch,
    InvalidBinding,
    InvalidEvaluationTime,
    DuplicateDevice,
    DuplicateInstance,
    TooManyCandidates,
    UnsupportedCapabilities,
    NoDecision,
    Placement(DevicePlacementValidationError),
}

impl std::fmt::Display for PersistedInventoryPlacementEvaluationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OwnerMismatch => formatter.write_str("owner_mismatch"),
            Self::InvalidBinding
            | Self::InvalidEvaluationTime
            | Self::DuplicateDevice
            | Self::DuplicateInstance
            | Self::TooManyCandidates => {
                formatter.write_str("invalid_persisted_inventory_placement_input")
            }
            Self::UnsupportedCapabilities => {
                formatter.write_str("unsupported_persisted_placement_capability")
            }
            Self::NoDecision => formatter.write_str("missing_persisted_placement_decision"),
            Self::Placement(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for PersistedInventoryPlacementEvaluationError {}

impl PersistedInventoryPlacementInput {
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn owner(&self) -> &SnapshotOwner {
        &self.owner
    }

    #[must_use]
    pub fn candidate(&self) -> &DevicePlacementCandidate {
        &self.candidate
    }

    #[must_use]
    pub fn reserved(&self) -> bool {
        self.reserved
    }

    #[must_use]
    pub fn owner_declaration_unverified(&self) -> bool {
        self.owner_declaration_unverified
    }

    #[must_use]
    pub fn policy_attributes_unverified(&self) -> bool {
        self.policy_attributes_unverified
    }
}

/// Stable failures for the persisted-inventory conversion boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PersistedInventoryPlacementInputError {
    PersistedInventory(PersistenceError),
    OwnerMismatch,
    TimestampOutOfRange,
    Candidate(DevicePlacementValidationError),
}

impl std::fmt::Display for PersistedInventoryPlacementInputError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PersistedInventory(error) => error.fmt(formatter),
            Self::OwnerMismatch => formatter.write_str("owner_mismatch"),
            Self::TimestampOutOfRange => {
                formatter.write_str("invalid_persisted_inventory_placement_input")
            }
            Self::Candidate(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for PersistedInventoryPlacementInputError {}

/// Builds one candidate/input from persisted inventory for the exact declared
/// evaluation owner. It does not evaluate policy, select a target, reserve
/// capacity, dispatch, or execute a Runner.
///
/// # Errors
///
/// Returns a persisted-inventory, owner, or candidate-binding error when the
/// supplied value cannot be converted without guessing missing policy data.
pub fn build_persisted_inventory_placement_input(
    value: &PersistedInventoryState,
    evaluation_owner: &SnapshotOwner,
) -> Result<PersistedInventoryPlacementInput, PersistedInventoryPlacementInputError> {
    validate_persisted_inventory(value)
        .map_err(PersistedInventoryPlacementInputError::PersistedInventory)?;
    if value.device().owner() != evaluation_owner {
        return Err(PersistedInventoryPlacementInputError::OwnerMismatch);
    }
    if value.runner().server_observed_at_ms() > MAX_SAFE_INTEGER_MS
        || value.runner().capability_lease_expires_at_ms() > MAX_SAFE_INTEGER_MS
    {
        return Err(PersistedInventoryPlacementInputError::TimestampOutOfRange);
    }
    let candidate =
        DevicePlacementCandidate::new(value.device().device().clone(), value.runner().clone())
            .map_err(PersistedInventoryPlacementInputError::Candidate)?;
    Ok(PersistedInventoryPlacementInput {
        revision: value.revision(),
        owner: value.device().owner().clone(),
        candidate,
        reserved: value.device().reserved(),
        owner_declaration_unverified: true,
        policy_attributes_unverified: true,
    })
}

/// Evaluates one already validated persisted input with the existing pure
/// placement comparator. The persisted owner supplies the tenant boundary;
/// evaluation time is caller supplied. No target is selected or reserved.
///
/// # Errors
///
/// Returns a stable unsupported-capability error for GPU declarations or the
/// underlying placement validation error for malformed requirements.
pub fn evaluate_persisted_inventory_placement_input(
    input: &PersistedInventoryPlacementInput,
    requirements: DevicePlacementRequirements,
    evaluated_at_ms: u64,
) -> Result<PersistedInventoryPlacementEvaluation, PersistedInventoryPlacementEvaluationError> {
    if !input.candidate.instance().capabilities().gpus().is_empty() {
        return Err(PersistedInventoryPlacementEvaluationError::UnsupportedCapabilities);
    }
    let request =
        DevicePlacementRequest::new(input.candidate.device.tenant_id().clone(), requirements);
    let Some(decision) = dry_run_device_placement(
        std::slice::from_ref(input.candidate()),
        &request,
        evaluated_at_ms,
    )
    .map_err(PersistedInventoryPlacementEvaluationError::Placement)?
    .into_iter()
    .next() else {
        return Err(PersistedInventoryPlacementEvaluationError::NoDecision);
    };
    let (matches_requirements, exclusion_reasons) = match decision.disposition() {
        DevicePlacementDisposition::Eligible => (true, Vec::new()),
        DevicePlacementDisposition::Excluded(reasons) => (
            false,
            reasons
                .iter()
                .map(|reason| reason.as_str().to_owned())
                .collect(),
        ),
    };
    Ok(PersistedInventoryPlacementEvaluation {
        revision: input.revision(),
        device_id: decision.device_id().as_str().to_owned(),
        instance_id: decision.instance_id().as_str().to_owned(),
        matches_requirements,
        exclusion_reasons,
        owner_declaration_unverified: input.owner_declaration_unverified(),
        device_attributes_unverified: input.policy_attributes_unverified(),
    })
}

/// One complete, deterministic evaluation over persisted inventory inputs.
/// The source revision and Runner instance remain visible for every decision;
/// no target is selected and no execution authority is created.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedInventoryPlacementBatchEvaluation {
    pub schema_version: &'static str,
    pub evaluation_mode: &'static str,
    pub owner: SnapshotOwner,
    pub evaluated_at_ms: u64,
    pub owner_declaration_unverified: bool,
    pub device_attributes_unverified: bool,
    pub decisions: Vec<PersistedInventoryPlacementBatchDecision>,
    pub selected_device_id: Option<String>,
    pub selected_instance_id: Option<String>,
    pub authority: PersistedInventoryPlacementBatchAuthority,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedInventoryPlacementBatchDecision {
    pub revision: u64,
    pub device_id: String,
    pub instance_id: String,
    pub matches_requirements: bool,
    pub exclusion_reasons: Vec<String>,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PersistedInventoryPlacementBatchAuthority {
    pub identity_verified: bool,
    pub heartbeat_persisted: bool,
    pub inventory_authoritative: bool,
    pub placement_selected: bool,
    pub reservation_created: bool,
    pub execution_authorized: bool,
    pub dispatch_performed: bool,
}

/// Evaluates a bounded batch through the existing offline placement
/// comparator. Every input must carry the exact declared owner tuple.
///
/// # Errors
///
/// Returns an owner/binding, duplicate, unsupported-capability, or placement
/// validation error. The function never reads a clock or mutates a source.
#[allow(clippy::too_many_lines)]
pub fn evaluate_persisted_inventory_placement(
    inputs: &[PersistedInventoryPlacementInput],
    owner: &SnapshotOwner,
    requirements: DevicePlacementRequirements,
    evaluated_at_ms: u64,
) -> Result<PersistedInventoryPlacementBatchEvaluation, PersistedInventoryPlacementEvaluationError>
{
    if !valid_persisted_placement_owner(owner) {
        return Err(PersistedInventoryPlacementEvaluationError::InvalidBinding);
    }
    if evaluated_at_ms == 0 || evaluated_at_ms > MAX_SAFE_INTEGER_MS {
        return Err(PersistedInventoryPlacementEvaluationError::InvalidEvaluationTime);
    }
    if inputs.len() > MAX_PERSISTED_INVENTORY_PLACEMENT_CANDIDATES {
        return Err(PersistedInventoryPlacementEvaluationError::TooManyCandidates);
    }
    let tenant_id = super::model::TenantId::parse(owner.tenant_id.clone())
        .map_err(|_| PersistedInventoryPlacementEvaluationError::InvalidBinding)?;
    let mut candidates = Vec::with_capacity(inputs.len());
    let mut metadata = BTreeMap::new();
    let mut instances = HashSet::with_capacity(inputs.len());
    for input in inputs {
        if input.owner() != owner
            || !input.owner_declaration_unverified()
            || !input.policy_attributes_unverified()
        {
            return Err(PersistedInventoryPlacementEvaluationError::OwnerMismatch);
        }
        if !input
            .candidate()
            .instance()
            .capabilities()
            .gpus()
            .is_empty()
        {
            return Err(PersistedInventoryPlacementEvaluationError::UnsupportedCapabilities);
        }
        let device_id = input.candidate().device().id().as_str().to_owned();
        let instance_id = input
            .candidate()
            .instance()
            .instance_id()
            .as_str()
            .to_owned();
        if metadata
            .insert(device_id.clone(), (input.revision(), instance_id.clone()))
            .is_some()
        {
            return Err(PersistedInventoryPlacementEvaluationError::DuplicateDevice);
        }
        if !instances.insert(instance_id) {
            return Err(PersistedInventoryPlacementEvaluationError::DuplicateInstance);
        }
        candidates.push(input.candidate().clone());
    }
    let request = DevicePlacementRequest::new(tenant_id, requirements);
    let decisions = dry_run_device_placement(&candidates, &request, evaluated_at_ms)
        .map_err(PersistedInventoryPlacementEvaluationError::Placement)?
        .into_iter()
        .map(|decision| map_persisted_placement_decision(&decision, &metadata))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PersistedInventoryPlacementBatchEvaluation {
        schema_version: PERSISTED_INVENTORY_PLACEMENT_EVALUATION_SCHEMA_VERSION,
        evaluation_mode: PERSISTED_INVENTORY_PLACEMENT_EVALUATION_MODE,
        owner: owner.clone(),
        evaluated_at_ms,
        owner_declaration_unverified: true,
        device_attributes_unverified: true,
        decisions,
        selected_device_id: None,
        selected_instance_id: None,
        authority: PersistedInventoryPlacementBatchAuthority::default(),
    })
}

fn map_persisted_placement_decision(
    decision: &DevicePlacementDecision,
    metadata: &BTreeMap<String, (u64, String)>,
) -> Result<PersistedInventoryPlacementBatchDecision, PersistedInventoryPlacementEvaluationError> {
    let device_id = decision.device_id().as_str().to_owned();
    let (revision, instance_id) = metadata
        .get(&device_id)
        .cloned()
        .ok_or(PersistedInventoryPlacementEvaluationError::InvalidBinding)?;
    let (matches_requirements, mut exclusion_reasons) = match decision.disposition() {
        DevicePlacementDisposition::Eligible => (true, Vec::new()),
        DevicePlacementDisposition::Excluded(reasons) => (
            false,
            reasons
                .iter()
                .map(|reason| persisted_placement_exclusion_name(*reason).to_owned())
                .collect(),
        ),
    };
    exclusion_reasons.sort();
    Ok(PersistedInventoryPlacementBatchDecision {
        revision,
        device_id,
        instance_id,
        matches_requirements,
        exclusion_reasons,
    })
}

fn valid_persisted_placement_owner(owner: &SnapshotOwner) -> bool {
    [
        owner.issuer.as_str(),
        owner.subject.as_str(),
        owner.tenant_id.as_str(),
    ]
    .into_iter()
    .all(|value| {
        !value.is_empty()
            && value.len() <= 512
            && value.trim() == value
            && !value.chars().any(char::is_control)
    })
}

fn persisted_placement_exclusion_name(reason: DevicePlacementExclusion) -> &'static str {
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
