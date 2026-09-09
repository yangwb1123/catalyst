use crate::runtime_domain::{
    execution::attempt::{
        AttemptBudget, AttemptRequest, AttemptRequestInput, ControlVersionBinding,
    },
    platform_core_contract::{ArtifactRef, EntityRef, ExecutorDescriptor, RecordRef, ScopeRef},
};
use serde::{Deserialize, Serialize};

pub(super) const FORMAT: &str = "forge.runtime.attempt-request.storage.v1";

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StoredRequest {
    pub format: String,
    pub version: i64,
    scope_ref: ScopeRef,
    attempt_ref: EntityRef,
    work_item_ref: EntityRef,
    project_ref: EntityRef,
    project_snapshot_ref: EntityRef,
    control_versions: StoredVersions,
    executor: ExecutorDescriptor,
    context_artifact_ref: Option<ArtifactRef>,
    workspace_capability_ref: Option<RecordRef>,
    grant_ref: Option<RecordRef>,
    approval_refs: Vec<RecordRef>,
    requested_effects: Vec<String>,
    budget: StoredBudget,
    timeout_ms: i64,
    idempotency_key: String,
}

impl StoredRequest {
    pub(super) fn from_request(request: &AttemptRequest) -> Self {
        Self {
            format: FORMAT.into(),
            version: 1,
            scope_ref: request.scope_ref().clone(),
            attempt_ref: request.attempt_ref().clone(),
            work_item_ref: request.work_item_ref().clone(),
            project_ref: request.project_ref().clone(),
            project_snapshot_ref: request.project_snapshot_ref().clone(),
            control_versions: StoredVersions::from_binding(request.control_versions()),
            executor: request.executor().clone(),
            context_artifact_ref: request.context_artifact_ref().cloned(),
            workspace_capability_ref: request.workspace_capability_ref().cloned(),
            grant_ref: request.grant_ref().cloned(),
            approval_refs: request.approval_refs().to_vec(),
            requested_effects: request.requested_effects().to_vec(),
            budget: StoredBudget::from_budget(request.budget()),
            timeout_ms: request.timeout_ms(),
            idempotency_key: request.idempotency_key().into(),
        }
    }

    pub(super) fn into_input(self) -> AttemptRequestInput {
        AttemptRequestInput {
            scope_ref: self.scope_ref,
            attempt_ref: self.attempt_ref,
            work_item_ref: self.work_item_ref,
            project_ref: self.project_ref,
            project_snapshot_ref: self.project_snapshot_ref,
            control_versions: self.control_versions.into_binding(),
            executor: self.executor,
            context_artifact_ref: self.context_artifact_ref,
            workspace_capability_ref: self.workspace_capability_ref,
            grant_ref: self.grant_ref,
            approval_refs: self.approval_refs,
            requested_effects: self.requested_effects,
            budget: self.budget.into_budget(),
            timeout_ms: self.timeout_ms,
            idempotency_key: self.idempotency_key,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_field_names)] // Mirror the frozen declaration's exact storage field names.
struct StoredVersions {
    objective_version: i64,
    change_version: i64,
    work_graph_version: i64,
    work_item_version: i64,
}

impl StoredVersions {
    fn from_binding(value: ControlVersionBinding) -> Self {
        Self {
            objective_version: value.objective_version,
            change_version: value.change_version,
            work_graph_version: value.work_graph_version,
            work_item_version: value.work_item_version,
        }
    }

    fn into_binding(self) -> ControlVersionBinding {
        ControlVersionBinding {
            objective_version: self.objective_version,
            change_version: self.change_version,
            work_graph_version: self.work_graph_version,
            work_item_version: self.work_item_version,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_field_names)] // Mirror the frozen declaration's exact storage field names.
struct StoredBudget {
    max_duration_ms: i64,
    max_cost_usd_micros: i64,
    max_model_calls: i64,
    max_tool_calls: i64,
    max_input_tokens: i64,
    max_output_tokens: i64,
    max_output_bytes: i64,
    max_network_bytes: i64,
}

impl StoredBudget {
    fn from_budget(value: AttemptBudget) -> Self {
        Self {
            max_duration_ms: value.max_duration_ms,
            max_cost_usd_micros: value.max_cost_usd_micros,
            max_model_calls: value.max_model_calls,
            max_tool_calls: value.max_tool_calls,
            max_input_tokens: value.max_input_tokens,
            max_output_tokens: value.max_output_tokens,
            max_output_bytes: value.max_output_bytes,
            max_network_bytes: value.max_network_bytes,
        }
    }

    fn into_budget(self) -> AttemptBudget {
        AttemptBudget {
            max_duration_ms: self.max_duration_ms,
            max_cost_usd_micros: self.max_cost_usd_micros,
            max_model_calls: self.max_model_calls,
            max_tool_calls: self.max_tool_calls,
            max_input_tokens: self.max_input_tokens,
            max_output_tokens: self.max_output_tokens,
            max_output_bytes: self.max_output_bytes,
            max_network_bytes: self.max_network_bytes,
        }
    }
}
