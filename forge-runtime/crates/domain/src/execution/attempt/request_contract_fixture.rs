use super::{
    AttemptBudget, AttemptRequest, AttemptRequestErrorCode, AttemptRequestInput,
    ControlVersionBinding,
};
use crate::platform_core_contract::{
    ArtifactRef, AttemptState, EntityRef, ExecutorDescriptor, RecordRef, ScopeRef,
};
use serde::Deserialize;

const FIXTURE: &str =
    include_str!("../../../../../../docs/contracts/fixtures/forge-attempt-request-v1.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    authority: Authority,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Authority {
    device_identity_verified: bool,
    references_resolved: bool,
    request_persisted: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
    audit_published: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    request: WireRequest,
    expected: Expected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRequest {
    scope_ref: ScopeRef,
    attempt_ref: EntityRef,
    work_item_ref: EntityRef,
    project_ref: EntityRef,
    project_snapshot_ref: EntityRef,
    control_versions: WireControlVersionBinding,
    executor: ExecutorDescriptor,
    context_artifact_ref: Option<ArtifactRef>,
    workspace_capability_ref: Option<RecordRef>,
    grant_ref: Option<RecordRef>,
    approval_refs: Vec<RecordRef>,
    requested_effects: Vec<String>,
    budget: WireAttemptBudget,
    timeout_ms: i64,
    idempotency_key: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireControlVersionBinding {
    objective_version: i64,
    change_version: i64,
    work_graph_version: i64,
    work_item_version: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireAttemptBudget {
    max_duration_ms: i64,
    max_cost_usd_micros: i64,
    max_model_calls: i64,
    max_tool_calls: i64,
    max_input_tokens: i64,
    max_output_tokens: i64,
    max_output_bytes: i64,
    max_network_bytes: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    accepted: bool,
    #[serde(default)]
    error: Option<String>,
    initial_state: String,
    requested_effects: Vec<String>,
    approval_record_ids: Vec<String>,
}

impl From<WireRequest> for AttemptRequestInput {
    fn from(value: WireRequest) -> Self {
        Self {
            scope_ref: value.scope_ref,
            attempt_ref: value.attempt_ref,
            work_item_ref: value.work_item_ref,
            project_ref: value.project_ref,
            project_snapshot_ref: value.project_snapshot_ref,
            control_versions: value.control_versions.into(),
            executor: value.executor,
            context_artifact_ref: value.context_artifact_ref,
            workspace_capability_ref: value.workspace_capability_ref,
            grant_ref: value.grant_ref,
            approval_refs: value.approval_refs,
            requested_effects: value.requested_effects,
            budget: value.budget.into(),
            timeout_ms: value.timeout_ms,
            idempotency_key: value.idempotency_key,
        }
    }
}

impl From<WireControlVersionBinding> for ControlVersionBinding {
    fn from(value: WireControlVersionBinding) -> Self {
        Self {
            objective_version: value.objective_version,
            change_version: value.change_version,
            work_graph_version: value.work_graph_version,
            work_item_version: value.work_item_version,
        }
    }
}

impl From<WireAttemptBudget> for AttemptBudget {
    fn from(value: WireAttemptBudget) -> Self {
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
}

#[test]
fn attempt_request_contract_fixture() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).expect("strict Attempt request fixture");
    assert_eq!(fixture.schema_version, "forge.attempt-request/v1");
    assert_eq!(fixture.evaluation_mode, "pure_attempt_request_only");
    assert!(!fixture.authority.device_identity_verified);
    assert!(!fixture.authority.references_resolved);
    assert!(!fixture.authority.request_persisted);
    assert!(!fixture.authority.reservation_created);
    assert!(!fixture.authority.execution_authorized);
    assert!(!fixture.authority.dispatch_performed);
    assert!(!fixture.authority.audit_published);
    assert_eq!(fixture.cases.len(), 18);

    for case in fixture.cases {
        let result = AttemptRequest::try_from_input(&case.request.into());
        if case.expected.accepted {
            let request =
                result.unwrap_or_else(|error| panic!("{}: request rejected: {error}", case.name));
            assert_eq!(
                request.initial_state(),
                AttemptState::Requested,
                "{}",
                case.name
            );
            assert_eq!(case.expected.initial_state, "requested", "{}", case.name);
            assert_eq!(
                request.requested_effects(),
                case.expected.requested_effects,
                "{}",
                case.name
            );
            let ids: Vec<&str> = request
                .approval_refs()
                .iter()
                .map(|value| value.record_id.as_str())
                .collect();
            assert_eq!(ids, case.expected.approval_record_ids, "{}", case.name);
        } else {
            let error = result.expect_err("request accepted unexpectedly");
            assert_eq!(
                error_code(error.code()),
                case.expected.error.as_deref().unwrap(),
                "{}",
                case.name
            );
        }
    }
}

fn error_code(value: AttemptRequestErrorCode) -> &'static str {
    match value {
        AttemptRequestErrorCode::InvalidValue => "invalid_value",
        AttemptRequestErrorCode::ReferenceMismatch => "reference_mismatch",
    }
}
