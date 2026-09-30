use super::state::{ParsedState, case_state, evaluation_owner, parse_state, replacement_runner};
use super::wire::{Authority, Case, CaseOutput, Fixture, PersistenceOutput, StateOutput};
use super::{EVALUATION_MODE, MAX_CASE_NAME_BYTES, MAX_CASES, SCHEMA_VERSION};
use forge_runtime_domain::{
    PersistedInventoryProjection, PersistedInventoryState, commit_persisted_inventory,
    project_persisted_inventory, restore_persisted_inventory,
};
use std::{collections::HashSet, error::Error};

pub(super) fn evaluate(fixture: Fixture) -> Result<PersistenceOutput, Box<dyn Error>> {
    validate_fixture_shape(&fixture)?;
    let base = parse_state(&fixture.state)?;
    let mut names = HashSet::with_capacity(fixture.cases.len());
    let mut cases = Vec::with_capacity(fixture.cases.len());
    for case in fixture.cases {
        validate_case_name(&case.name, &mut names)?;
        cases.push(evaluate_case(&base, &case, fixture.stale_after_ms)?);
    }
    Ok(PersistenceOutput {
        v: 1,
        output_type: "device_inventory_persistence_preview",
        schema_version: SCHEMA_VERSION,
        evaluation_mode: EVALUATION_MODE,
        stale_after_ms: fixture.stale_after_ms,
        state: StateOutput {
            revision: fixture.state.revision,
            device_id: fixture.state.device.device_id,
            instance_id: fixture.state.runner.instance_id,
            tenant_id: fixture.state.device.owner.tenant_id,
            approval_state: fixture.state.device.approval_state,
            cordon_state: fixture.state.device.cordon_state,
            reservation_state: fixture.state.device.reservation_state,
            liveness: fixture.state.runner.liveness,
        },
        cases,
        authority: Authority::default(),
    })
}

fn validate_fixture_shape(fixture: &Fixture) -> Result<(), Box<dyn Error>> {
    if fixture.schema_version != SCHEMA_VERSION
        || fixture.evaluation_mode != EVALUATION_MODE
        || fixture.stale_after_ms != forge_runtime_domain::DEFAULT_STALE_AFTER_MS
        || fixture.cases.len() != 12
        || fixture.cases.len() > MAX_CASES
        || fixture.authority != Authority::default()
    {
        return Err(
            "device inventory persistence input is not a bounded pure CAS/projection contract"
                .into(),
        );
    }
    Ok(())
}

fn validate_case_name(name: &str, names: &mut HashSet<String>) -> Result<(), Box<dyn Error>> {
    if name.trim().is_empty() || name.len() > MAX_CASE_NAME_BYTES {
        return Err("device inventory persistence case name is empty or too long".into());
    }
    if !names.insert(name.to_owned()) {
        return Err(format!("duplicate device inventory persistence case {name:?}").into());
    }
    Ok(())
}

fn evaluate_case(
    base: &ParsedState,
    case: &Case,
    stale_after_ms: u64,
) -> Result<CaseOutput, Box<dyn Error>> {
    let parsed = case_state(base, case)?;
    let result: Result<CaseResult, String> = match case.operation.as_str() {
        "restore" => restore_persisted_inventory(parsed.revision, parsed.device, parsed.runner)
            .map(|_| CaseResult::Accepted(CaseValues::default()))
            .map_err(|error| error.to_string()),
        "commit" => evaluate_commit(parsed, case),
        "project" => {
            let owner = evaluation_owner(&parsed.device, case.evaluation_owner.as_deref())
                .map_err(|error| error.to_string())?;
            restore_persisted_inventory(parsed.revision, parsed.device, parsed.runner)
                .map_err(|error| error.to_string())
                .and_then(|state| {
                    project_persisted_inventory(
                        &state,
                        &owner,
                        case.evaluated_at_ms
                            .ok_or_else(|| "missing evaluated_at_ms".to_owned())?,
                        stale_after_ms,
                    )
                    .map(|projection| {
                        CaseResult::Accepted(CaseValues::from_projection(&projection))
                    })
                    .map_err(|error| error.to_string())
                })
        }
        operation => Err(format!(
            "unknown inventory persistence operation {operation:?}"
        )),
    };
    match result {
        Ok(CaseResult::Accepted(values)) => compare_success(case, values),
        Err(error) => compare_error(case, error),
    }
}

#[derive(Debug, Default)]
struct CaseValues {
    revision: Option<u64>,
    device_id: Option<String>,
    instance_id: Option<String>,
    heartbeat_sequence: Option<u64>,
    server_observed_at_ms: Option<u64>,
    capability_lease_expires_at_ms: Option<u64>,
    status: Option<String>,
    fresh: Option<bool>,
    declared_eligible: Option<bool>,
}

impl CaseValues {
    fn from_commit(state: &PersistedInventoryState) -> Self {
        Self {
            revision: Some(state.revision()),
            heartbeat_sequence: Some(state.runner().heartbeat_sequence()),
            server_observed_at_ms: Some(state.runner().server_observed_at_ms()),
            capability_lease_expires_at_ms: Some(state.runner().capability_lease_expires_at_ms()),
            ..Self::default()
        }
    }

    fn from_projection(projection: &PersistedInventoryProjection) -> Self {
        Self {
            revision: Some(projection.revision),
            device_id: Some(projection.device_id.to_string()),
            instance_id: Some(projection.instance_id.to_string()),
            status: Some(projection.status.as_str().to_owned()),
            fresh: Some(projection.fresh),
            declared_eligible: Some(projection.declared_eligible),
            ..Self::default()
        }
    }
}

enum CaseResult {
    Accepted(CaseValues),
}

fn compare_success(case: &Case, values: CaseValues) -> Result<CaseOutput, Box<dyn Error>> {
    let expected = &case.expected;
    if !expected.accepted
        || expected.error.is_some()
        || expected.revision != values.revision
        || expected.device_id != values.device_id
        || expected.instance_id != values.instance_id
        || expected.heartbeat_sequence != values.heartbeat_sequence
        || expected.server_observed_at_ms != values.server_observed_at_ms
        || expected.capability_lease_expires_at_ms != values.capability_lease_expires_at_ms
        || expected.status != values.status
        || expected.fresh != values.fresh
        || expected.declared_eligible != values.declared_eligible
    {
        return Err(format!(
            "device inventory persistence case {:?} expectation mismatch",
            case.name
        )
        .into());
    }
    Ok(CaseOutput {
        name: case.name.clone(),
        operation: case.operation.clone(),
        accepted: true,
        revision: values.revision,
        device_id: values.device_id,
        instance_id: values.instance_id,
        heartbeat_sequence: values.heartbeat_sequence,
        server_observed_at_ms: values.server_observed_at_ms,
        capability_lease_expires_at_ms: values.capability_lease_expires_at_ms,
        status: values.status,
        fresh: values.fresh,
        declared_eligible: values.declared_eligible,
        error: None,
    })
}

fn compare_error(case: &Case, error: String) -> Result<CaseOutput, Box<dyn Error>> {
    let expected = &case.expected;
    if expected.accepted
        || expected.error.as_deref() != Some(error.as_str())
        || expected.revision.is_some()
        || expected.device_id.is_some()
        || expected.instance_id.is_some()
        || expected.heartbeat_sequence.is_some()
        || expected.server_observed_at_ms.is_some()
        || expected.capability_lease_expires_at_ms.is_some()
        || expected.status.is_some()
        || expected.fresh.is_some()
        || expected.declared_eligible.is_some()
    {
        return Err(format!(
            "device inventory persistence case {:?} expectation mismatch",
            case.name
        )
        .into());
    }
    Ok(CaseOutput {
        name: case.name.clone(),
        operation: case.operation.clone(),
        accepted: false,
        revision: None,
        device_id: None,
        instance_id: None,
        heartbeat_sequence: None,
        server_observed_at_ms: None,
        capability_lease_expires_at_ms: None,
        status: None,
        fresh: None,
        declared_eligible: None,
        error: Some(error),
    })
}

fn evaluate_commit(parsed: ParsedState, case: &Case) -> Result<CaseResult, String> {
    let current = restore_persisted_inventory(
        parsed.revision,
        parsed.device.clone(),
        parsed.runner.clone(),
    );
    match current {
        Ok(current) => replacement_runner(case, &parsed.runner)
            .map_err(|error| error.to_string())
            .and_then(|runner| {
                commit_persisted_inventory(
                    Some(&current),
                    case.expected_revision
                        .ok_or_else(|| "missing expected_revision".to_owned())?,
                    parsed.device,
                    runner,
                )
                .map(|state| CaseResult::Accepted(CaseValues::from_commit(&state)))
                .map_err(|error| error.to_string())
            }),
        Err(error) => Err(error.to_string()),
    }
}
