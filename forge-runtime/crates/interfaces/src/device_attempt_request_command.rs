use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use forge_runtime_domain::{
    execution::attempt::{
        AttemptBudget, AttemptRequest, AttemptRequestErrorCode, AttemptRequestInput,
        ControlVersionBinding,
    },
    platform_core_contract::{ArtifactRef, EntityRef, ExecutorDescriptor, RecordRef, ScopeRef},
};
use serde::{Deserialize, Serialize};

use crate::{args::DeviceCommand, device_json_unique::reject_duplicate_keys};

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const SCHEMA_VERSION: &str = "forge.attempt-request/v1";
const EVALUATION_MODE: &str = "pure_attempt_request_only";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    authority: Authority,
    cases: Vec<Case>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "Independent contract flags preserve the frozen observation and authority wire shape"
)]
struct Authority {
    device_identity_verified: bool,
    references_resolved: bool,
    request_persisted: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
    audit_published: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    request: WireRequest,
    expected: Expected,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_field_names,
    reason = "Preserve frozen wire field names and existing Serde type-name diagnostics"
)]
struct WireControlVersionBinding {
    objective_version: i64,
    change_version: i64,
    work_graph_version: i64,
    work_item_version: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_field_names,
    reason = "Preserve frozen wire field names and existing Serde type-name diagnostics"
)]
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    accepted: bool,
    #[serde(default)]
    error: Option<String>,
    initial_state: String,
    requested_effects: Vec<String>,
    approval_record_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AttemptRequestPreviewOutput {
    schema_version: &'static str,
    evaluation_mode: &'static str,
    cases: Vec<CaseOutput>,
    authority: Authority,
}

#[derive(Debug, Serialize)]
struct CaseOutput {
    name: String,
    accepted: bool,
    error: Option<&'static str>,
    initial_state: &'static str,
    requested_effects: Vec<String>,
    approval_record_ids: Vec<String>,
}

pub(crate) fn execute(
    command: &DeviceCommand,
) -> Result<AttemptRequestPreviewOutput, Box<dyn Error>> {
    let DeviceCommand::AttemptRequestPreview { input } = command else {
        return Err("device Attempt request preview command is required".into());
    };
    let bytes = read_bounded_input(input)?;
    reject_duplicate_keys(&bytes)
        .map_err(|error| format!("Attempt request input has duplicate JSON keys: {error}"))?;
    let fixture: Fixture = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Attempt request input is invalid JSON: {error}"))?;
    evaluate(fixture)
}

pub(crate) fn write_output(
    output: &AttemptRequestPreviewOutput,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    let accepted = output.cases.iter().filter(|case| case.accepted).count();
    writeln!(
        writer,
        "offline Attempt request preview [{}] cases={} accepted={} rejected={}",
        output.schema_version,
        output.cases.len(),
        accepted,
        output.cases.len() - accepted
    )?;
    for case in &output.cases {
        let error = case.error.unwrap_or("none");
        writeln!(
            writer,
            "{}: accepted={} error={} state={} effects={} approvals={}",
            case.name,
            case.accepted,
            error,
            case.initial_state,
            case.requested_effects.join(","),
            case.approval_record_ids.join(",")
        )?;
    }
    writeln!(
        writer,
        "authority: device_identity_verified=false references_resolved=false request_persisted=false reservation_created=false execution_authorized=false dispatch_performed=false audit_published=false"
    )
}

fn evaluate(fixture: Fixture) -> Result<AttemptRequestPreviewOutput, Box<dyn Error>> {
    validate_fixture(&fixture)?;
    let mut cases = Vec::with_capacity(fixture.cases.len());
    for case in fixture.cases {
        cases.push(evaluate_case(case)?);
    }
    Ok(AttemptRequestPreviewOutput {
        schema_version: SCHEMA_VERSION,
        evaluation_mode: EVALUATION_MODE,
        cases,
        authority: fixture.authority,
    })
}

fn validate_fixture(fixture: &Fixture) -> Result<(), Box<dyn Error>> {
    if fixture.schema_version != SCHEMA_VERSION || fixture.evaluation_mode != EVALUATION_MODE {
        return Err("Attempt request input has an unsupported schema or evaluation mode".into());
    }
    if !authority_is_false(fixture.authority) {
        return Err("Attempt request input claims execution authority".into());
    }
    if fixture.cases.is_empty() || fixture.cases.len() > 64 {
        return Err("Attempt request input must contain 1..=64 cases".into());
    }
    let mut names = std::collections::HashSet::with_capacity(fixture.cases.len());
    for case in &fixture.cases {
        if case.name.is_empty() || case.name.len() > 128 || !names.insert(&case.name) {
            return Err("Attempt request case names must be unique bounded text".into());
        }
    }
    Ok(())
}

fn evaluate_case(case: Case) -> Result<CaseOutput, Box<dyn Error>> {
    let expected = case.expected;
    match AttemptRequest::try_from_input(&case.request.into()) {
        Ok(request) => accepted_case(case.name, &expected, &request),
        Err(error) => rejected_case(case.name, expected, error.code()),
    }
}

fn accepted_case(
    name: String,
    expected: &Expected,
    request: &AttemptRequest,
) -> Result<CaseOutput, Box<dyn Error>> {
    if !expected.accepted
        || expected.initial_state != "requested"
        || request.requested_effects() != expected.requested_effects
        || request
            .approval_refs()
            .iter()
            .map(|value| value.record_id.as_str())
            .collect::<Vec<_>>()
            != expected
                .approval_record_ids
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
    {
        return Err(format!("Attempt request fixture expectation mismatch: {name}").into());
    }
    Ok(CaseOutput {
        name,
        accepted: true,
        error: None,
        initial_state: "requested",
        requested_effects: request.requested_effects().to_vec(),
        approval_record_ids: request
            .approval_refs()
            .iter()
            .map(|value| value.record_id.clone())
            .collect(),
    })
}

fn rejected_case(
    name: String,
    expected: Expected,
    code: AttemptRequestErrorCode,
) -> Result<CaseOutput, Box<dyn Error>> {
    let expected_error = expected
        .error
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{name} has no expected rejection class"))?;
    let actual_error = error_code(code);
    if expected.accepted || expected_error != actual_error {
        return Err(format!("Attempt request fixture expectation mismatch: {name}").into());
    }
    Ok(CaseOutput {
        name,
        accepted: false,
        error: Some(actual_error),
        initial_state: "requested",
        requested_effects: expected.requested_effects,
        approval_record_ids: expected.approval_record_ids,
    })
}

fn error_code(value: AttemptRequestErrorCode) -> &'static str {
    match value {
        AttemptRequestErrorCode::InvalidValue => "invalid_value",
        AttemptRequestErrorCode::ReferenceMismatch => "reference_mismatch",
    }
}

fn authority_is_false(value: Authority) -> bool {
    !value.device_identity_verified
        && !value.references_resolved
        && !value.request_persisted
        && !value.reservation_created
        && !value.execution_authorized
        && !value.dispatch_performed
        && !value.audit_published
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::new();
    if input == "-" {
        io::stdin()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
    } else {
        File::open(Path::new(input))?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(format!("Attempt request input exceeds {MAX_INPUT_BYTES} bytes").into());
    }
    Ok(bytes)
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

#[cfg(test)]
#[path = "device_attempt_request_command_tests.rs"]
mod tests;
