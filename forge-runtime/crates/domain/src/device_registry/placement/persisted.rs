use std::collections::{BTreeMap, HashSet};

use super::super::model::TenantId;
use super::super::persistence::{
    PersistedInventoryState, PersistenceError, validate_persisted_inventory,
};
use super::super::snapshot::SnapshotOwner;
use super::{
    DevicePlacementCandidate, DevicePlacementDecision, DevicePlacementDisposition,
    DevicePlacementExclusion, DevicePlacementRequest, DevicePlacementRequirements,
    DevicePlacementValidationError, MAX_PERSISTED_INVENTORY_PLACEMENT_CANDIDATES,
    MAX_SAFE_INTEGER_MS, PERSISTED_INVENTORY_PLACEMENT_EVALUATION_MODE,
    PERSISTED_INVENTORY_PLACEMENT_EVALUATION_SCHEMA_VERSION, dry_run_device_placement,
};

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
    let tenant_id = TenantId::parse(owner.tenant_id.clone())
        .map_err(|_| PersistedInventoryPlacementEvaluationError::InvalidBinding)?;
    let ValidatedBatch {
        candidates,
        metadata,
    } = validate_batch_inputs(inputs, owner)?;
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

struct ValidatedBatch {
    candidates: Vec<DevicePlacementCandidate>,
    metadata: BTreeMap<String, (u64, String)>,
}

fn validate_batch_inputs(
    inputs: &[PersistedInventoryPlacementInput],
    owner: &SnapshotOwner,
) -> Result<ValidatedBatch, PersistedInventoryPlacementEvaluationError> {
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
    Ok(ValidatedBatch {
        candidates,
        metadata,
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
