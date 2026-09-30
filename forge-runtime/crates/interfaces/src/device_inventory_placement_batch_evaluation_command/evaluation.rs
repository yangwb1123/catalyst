use super::input::{build_input, owner, requirements};
use super::validation::{
    validate_error_cases, validate_evaluation, validate_fixture, validate_source,
};
use super::wire::{Authority, DecisionOutput, Fixture, Output, SourceFixture};
use forge_runtime_domain::evaluate_persisted_inventory_placement;
use std::{collections::HashSet, error::Error};
pub(super) fn evaluate(fixture: Fixture, source: &SourceFixture) -> Result<Output, Box<dyn Error>> {
    validate_fixture(&fixture, source)?;
    validate_source(&fixture, source)?;
    validate_error_cases(&fixture)?;
    let owner = owner(&fixture.evaluation_owner)?;
    let requirements = requirements(&fixture.requirements)?;
    let mut names = HashSet::new();
    let inputs = fixture
        .cases
        .iter()
        .map(|case| {
            if case.name.is_empty() || !names.insert(case.name.clone()) {
                return Err("duplicate placement batch case".into());
            }
            build_input(source, case, &owner)
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    let actual = evaluate_persisted_inventory_placement(
        &inputs,
        &owner,
        requirements,
        fixture.evaluated_at_ms,
    )
    .map_err(|e| e.to_string())?;
    validate_evaluation(&actual, &fixture, &owner)?;
    Ok(Output {
        schema_version: actual.schema_version,
        evaluation_mode: actual.evaluation_mode,
        owner_declaration: fixture.evaluation_owner,
        evaluated_at_ms: actual.evaluated_at_ms,
        owner_declaration_unverified: actual.owner_declaration_unverified,
        device_attributes_unverified: actual.device_attributes_unverified,
        decisions: actual
            .decisions
            .into_iter()
            .map(|d| DecisionOutput {
                revision: d.revision,
                device_id: d.device_id,
                instance_id: d.instance_id,
                matches_requirements: d.matches_requirements,
                exclusion_reasons: d.exclusion_reasons,
            })
            .collect(),
        selected_device_id: actual.selected_device_id,
        selected_instance_id: actual.selected_instance_id,
        authority: Authority::default(),
    })
}
