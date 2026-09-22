//! Bounded reference contracts for Runner heartbeats and placement eligibility.
//!
//! This module is a pure model only. It does not persist the coordinator's
//! authoritative device registry or grant dispatch authority; Forge Core Go
//! remains the owner of that state and its leases.

mod heartbeat;
mod identity;
mod model;
mod persisted_observation;
mod persisted_observation_v2;
mod persisted_placement_v2;
mod persistence;
mod placement;
mod runner;
mod snapshot;
mod status;

pub use heartbeat::{
    DeviceHeartbeatError, MAX_DEVICE_CAPABILITY_LEASE_TTL_MS, MIN_DEVICE_CAPABILITY_LEASE_TTL_MS,
    apply_device_heartbeat,
};
pub use identity::{
    DeviceIdentityBinding, DeviceIdentityProof, IDENTITY_PROOF_EVALUATION_MODE,
    IDENTITY_PROOF_SCHEMA_VERSION, IdentityChallenge, IdentityOwner, IdentityProofDecision,
    IdentityProofError, evaluate_identity_proof,
};
pub use model::{
    CapabilitySnapshot, Device, DeviceApprovalState, DeviceId, DeviceRegistryValidationError,
    DeviceStateError, GpuCapability, MAX_DEVICE_CAPABILITY_BYTES, MAX_DEVICE_CPU_CORES,
    MAX_DEVICE_GPU_COUNT, MAX_DEVICE_IDENTIFIER_BYTES, MAX_DEVICE_RUNTIME_COUNT,
    MAX_DEVICE_RUNTIME_NAME_BYTES, RunnerInstanceId, TenantId,
};
pub use persisted_observation::{
    MAX_PERSISTED_INVENTORY_OBSERVATION_SAFE_INTEGER, MAX_PERSISTED_INVENTORY_OBSERVATIONS,
    PERSISTED_INVENTORY_OBSERVATION_EVALUATION_MODE, PERSISTED_INVENTORY_OBSERVATION_NOTICE,
    PERSISTED_INVENTORY_OBSERVATION_SCHEMA_VERSION, PersistedInventoryObservation,
    PersistedInventoryObservationCandidate, PersistedInventoryObservationDevice,
    PersistedInventoryObservationError, PersistedInventoryObservationGpu,
    PersistedInventoryObservationOwner, build_persisted_inventory_observation,
};
pub use persisted_observation_v2::{
    PERSISTED_INVENTORY_OBSERVATION_V2_EVALUATION_MODE, PERSISTED_INVENTORY_OBSERVATION_V2_NOTICE,
    PERSISTED_INVENTORY_OBSERVATION_V2_SCHEMA_VERSION, PersistedInventoryObservationV2,
    PersistedInventoryObservationV2Candidate, PersistedInventoryObservationV2Device,
    PersistedInventoryObservationV2Error, PersistedInventoryObservationV2Gpu,
    build_persisted_inventory_observation_v2,
};
pub use persisted_placement_v2::{
    PERSISTED_INVENTORY_PLACEMENT_V2_EVALUATION_MODE, PERSISTED_INVENTORY_PLACEMENT_V2_NOTICE,
    PERSISTED_INVENTORY_PLACEMENT_V2_SCHEMA_VERSION, PersistedInventoryPlacementV2Decision,
    PersistedInventoryPlacementV2Error, PersistedInventoryPlacementV2Evaluation,
    evaluate_persisted_inventory_observation_v2,
};
pub use persistence::{
    PersistedInventoryDevice, PersistedInventoryProjection, PersistedInventoryState,
    PersistedRunnerInstance, PersistenceError, commit_device_heartbeat, commit_persisted_inventory,
    project_persisted_inventory, restore_persisted_inventory,
};
pub use placement::{
    DEVICE_HEARTBEAT_STALE_AFTER_MS, DEVICE_RESOURCE_SUMMARY_NOTICE,
    DEVICE_RESOURCE_SUMMARY_SCHEMA_VERSION, DevicePlacementAttributes, DevicePlacementCandidate,
    DevicePlacementDecision, DevicePlacementDisposition, DevicePlacementExclusion,
    DevicePlacementPolicy, DevicePlacementRequest, DevicePlacementRequirements,
    DevicePlacementValidationError, DeviceResourceSummary, DeviceResourceSummaryError,
    DeviceResourceSummaryOwner, DeviceResourceSummaryRequest, DeviceSandboxLevel, DeviceTrustZone,
    MAX_DEVICE_PLACEMENT_CANDIDATES, MAX_DEVICE_PLACEMENT_POLICY_ITEMS,
    MAX_DEVICE_RESIDENCY_ZONE_BYTES, MAX_RUN_INTENT_SAFE_INTEGER,
    PERSISTED_INVENTORY_PLACEMENT_EVALUATION_MODE,
    PERSISTED_INVENTORY_PLACEMENT_EVALUATION_SCHEMA_VERSION,
    PersistedInventoryPlacementBatchAuthority, PersistedInventoryPlacementBatchDecision,
    PersistedInventoryPlacementBatchEvaluation, PersistedInventoryPlacementEvaluation,
    PersistedInventoryPlacementEvaluationError, PersistedInventoryPlacementInput,
    PersistedInventoryPlacementInputError, RUN_INTENT_OBSERVATION_SCHEMA_VERSION,
    RunIntentObservation, RunIntentObservationError, RunIntentObservationRequest,
    RunIntentPromptReceipt, RunIntentRunReference, SESSION_PLACEMENT_OBSERVATION_SCHEMA_VERSION,
    SessionPlacementAuthority, SessionPlacementDecision, SessionPlacementObservation,
    SessionPlacementObservationError, SessionPlacementObservationRequest, SessionPlacementOwner,
    build_persisted_inventory_placement_input, dry_run_device_placement,
    evaluate_persisted_inventory_placement, evaluate_persisted_inventory_placement_input,
    observe_device_resource_summary, observe_run_intent, observe_session_placement,
};
pub use runner::{RunnerHeartbeat, RunnerInstance, RunnerLiveness};
pub use snapshot::{
    InventorySnapshot, MAX_SNAPSHOT_IDENTIFIER_BYTES, MAX_SNAPSHOT_OWNER_BYTES, MAX_SNAPSHOT_ROWS,
    SNAPSHOT_CANONICAL_DOMAIN, SnapshotError, SnapshotOwner, SnapshotRow,
    canonicalize_inventory_snapshot, inventory_snapshot_digest,
};
pub use status::{
    DEFAULT_STALE_AFTER_MS, InventoryStatus, InventoryStatusError, InventoryStatusObservation,
    InventoryStatusProjection, MAX_STALE_AFTER_MS, project_inventory_status,
};

#[cfg(test)]
mod tests;
