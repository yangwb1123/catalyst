use crate::platform_core_contract::{
    ArtifactRef, AttemptState, EntityRef, ExecutorDescriptor, RecordRef, ScopeRef,
};

use super::{AttemptRequestError, validation};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttemptBudget {
    pub max_duration_ms: i64,
    pub max_cost_usd_micros: i64,
    pub max_model_calls: i64,
    pub max_tool_calls: i64,
    pub max_input_tokens: i64,
    pub max_output_tokens: i64,
    pub max_output_bytes: i64,
    pub max_network_bytes: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlVersionBinding {
    pub objective_version: i64,
    pub change_version: i64,
    pub work_graph_version: i64,
    pub work_item_version: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttemptRequestInput {
    pub scope_ref: ScopeRef,
    pub attempt_ref: EntityRef,
    pub work_item_ref: EntityRef,
    pub project_ref: EntityRef,
    pub project_snapshot_ref: EntityRef,
    pub control_versions: ControlVersionBinding,
    pub executor: ExecutorDescriptor,
    pub context_artifact_ref: Option<ArtifactRef>,
    pub workspace_capability_ref: Option<RecordRef>,
    pub grant_ref: Option<RecordRef>,
    pub approval_refs: Vec<RecordRef>,
    pub requested_effects: Vec<String>,
    pub budget: AttemptBudget,
    pub timeout_ms: i64,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttemptRequest {
    scope_ref: ScopeRef,
    attempt_ref: EntityRef,
    work_item_ref: EntityRef,
    project_ref: EntityRef,
    project_snapshot_ref: EntityRef,
    control_versions: ControlVersionBinding,
    executor: ExecutorDescriptor,
    context_artifact_ref: Option<ArtifactRef>,
    workspace_capability_ref: Option<RecordRef>,
    grant_ref: Option<RecordRef>,
    approval_refs: Vec<RecordRef>,
    requested_effects: Vec<String>,
    budget: AttemptBudget,
    timeout_ms: i64,
    idempotency_key: String,
}

impl AttemptRequest {
    /// Validates and defensively owns an exact caller-supplied request.
    ///
    /// # Errors
    /// Returns an error when any declaration or exact relation is invalid.
    pub fn try_from_input(value: &AttemptRequestInput) -> Result<Self, AttemptRequestError> {
        validation::validate(value)?;
        Ok(Self::copy_from(value))
    }

    fn copy_from(value: &AttemptRequestInput) -> Self {
        let mut approval_refs = value.approval_refs.clone();
        approval_refs.sort_by(|left, right| {
            (&left.record_id, &left.record_sha256, &left.record_type).cmp(&(
                &right.record_id,
                &right.record_sha256,
                &right.record_type,
            ))
        });
        let mut requested_effects = value.requested_effects.clone();
        requested_effects.sort();
        Self {
            scope_ref: value.scope_ref.clone(),
            attempt_ref: value.attempt_ref.clone(),
            work_item_ref: value.work_item_ref.clone(),
            project_ref: value.project_ref.clone(),
            project_snapshot_ref: value.project_snapshot_ref.clone(),
            control_versions: value.control_versions,
            executor: value.executor.clone(),
            context_artifact_ref: value.context_artifact_ref.clone(),
            workspace_capability_ref: value.workspace_capability_ref.clone(),
            grant_ref: value.grant_ref.clone(),
            approval_refs,
            requested_effects,
            budget: value.budget,
            timeout_ms: value.timeout_ms,
            idempotency_key: value.idempotency_key.clone(),
        }
    }

    #[must_use]
    pub fn scope_ref(&self) -> &ScopeRef {
        &self.scope_ref
    }

    #[must_use]
    pub fn attempt_ref(&self) -> &EntityRef {
        &self.attempt_ref
    }

    #[must_use]
    pub fn work_item_ref(&self) -> &EntityRef {
        &self.work_item_ref
    }

    #[must_use]
    pub fn project_ref(&self) -> &EntityRef {
        &self.project_ref
    }

    #[must_use]
    pub fn project_snapshot_ref(&self) -> &EntityRef {
        &self.project_snapshot_ref
    }

    #[must_use]
    pub const fn control_versions(&self) -> ControlVersionBinding {
        self.control_versions
    }

    #[must_use]
    pub fn executor(&self) -> &ExecutorDescriptor {
        &self.executor
    }

    #[must_use]
    pub fn context_artifact_ref(&self) -> Option<&ArtifactRef> {
        self.context_artifact_ref.as_ref()
    }

    #[must_use]
    pub fn workspace_capability_ref(&self) -> Option<&RecordRef> {
        self.workspace_capability_ref.as_ref()
    }

    #[must_use]
    pub fn grant_ref(&self) -> Option<&RecordRef> {
        self.grant_ref.as_ref()
    }

    #[must_use]
    pub fn approval_refs(&self) -> &[RecordRef] {
        &self.approval_refs
    }

    #[must_use]
    pub fn requested_effects(&self) -> &[String] {
        &self.requested_effects
    }

    #[must_use]
    pub const fn budget(&self) -> AttemptBudget {
        self.budget
    }

    #[must_use]
    pub const fn timeout_ms(&self) -> i64 {
        self.timeout_ms
    }

    #[must_use]
    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }

    #[must_use]
    pub fn initial_state(&self) -> AttemptState {
        AttemptState::Requested
    }
}

impl TryFrom<&AttemptRequestInput> for AttemptRequest {
    type Error = AttemptRequestError;

    fn try_from(value: &AttemptRequestInput) -> Result<Self, Self::Error> {
        Self::try_from_input(value)
    }
}
