use super::wire::{Authority, Fixture, SourceFixture};
use super::{BATCH_MODE, BATCH_SCHEMA, MAX_SAFE_INTEGER, SOURCE_FIXTURE};
use forge_runtime_domain::{PersistedInventoryPlacementBatchEvaluation, SnapshotOwner};
use std::error::Error;
pub(super) fn validate_fixture(
    fixture: &Fixture,
    source: &SourceFixture,
) -> Result<(), Box<dyn Error>> {
    if fixture.schema_version != BATCH_SCHEMA
        || fixture.evaluation_mode != BATCH_MODE
        || fixture.source_fixture != SOURCE_FIXTURE
        || fixture.cases.len() != 6
        || !fixture.empty_inputs_allowed
        || fixture.selected_device_id.is_some()
        || fixture.selected_instance_id.is_some()
        || fixture.authority != Authority::default()
        || fixture.evaluation_owner != source.evaluation_owner
        || fixture.evaluated_at_ms == 0
        || fixture.evaluated_at_ms > MAX_SAFE_INTEGER
    {
        return Err("placement batch input is not a bounded pure read-only contract".into());
    }
    Ok(())
}

pub(super) fn validate_source(
    fixture: &Fixture,
    source: &SourceFixture,
) -> Result<(), Box<dyn Error>> {
    if source.schema_version != "forge.device-inventory-placement-input/v1"
        || source.evaluation_mode != "pure_persisted_inventory_to_placement_input"
        || source.authority != Authority::default()
        || source.policy_requirements.data_residency_zones
            != fixture.requirements.data_residency_zones
        || source.policy_requirements.minimum_trust_zone != fixture.requirements.minimum_trust_zone
        || source.policy_requirements.sandbox_floor != fixture.requirements.sandbox_floor
        || source.policy_requirements.concurrency_slots != fixture.requirements.concurrency_slots
        || source.state.device.device_id != source.state.runner.device_id
        || source.state.runner.instance_id.is_empty()
    {
        return Err("placement batch source is not a bounded pure input contract".into());
    }
    Ok(())
}

pub(super) fn validate_error_cases(fixture: &Fixture) -> Result<(), Box<dyn Error>> {
    if fixture.error_cases.len() != 4
        || fixture
            .error_cases
            .iter()
            .any(|case| match case.name.as_str() {
                "owner_mismatch" => case.error != "owner_mismatch",
                "duplicate_device" | "duplicate_instance" | "invalid_evaluated_at" => {
                    case.error != "invalid_persisted_inventory_placement_input"
                }
                _ => true,
            })
    {
        return Err("placement batch error_cases are not canonical".into());
    }
    Ok(())
}

pub(super) fn validate_evaluation(
    actual: &PersistedInventoryPlacementBatchEvaluation,
    fixture: &Fixture,
    owner: &SnapshotOwner,
) -> Result<(), Box<dyn Error>> {
    if actual.schema_version
        != forge_runtime_domain::PERSISTED_INVENTORY_PLACEMENT_EVALUATION_SCHEMA_VERSION
        || actual.evaluation_mode
            != forge_runtime_domain::PERSISTED_INVENTORY_PLACEMENT_EVALUATION_MODE
        || actual.owner != *owner
        || actual.evaluated_at_ms != fixture.evaluated_at_ms
        || !actual.owner_declaration_unverified
        || !actual.device_attributes_unverified
        || actual.selected_device_id.is_some()
        || actual.selected_instance_id.is_some()
        || actual.authority
            != forge_runtime_domain::PersistedInventoryPlacementBatchAuthority::default()
    {
        return Err("placement batch evaluation output failed contract validation".into());
    }
    if actual.decisions.len() != fixture.cases.len() {
        return Err("placement batch decision count mismatch".into());
    }
    for (actual, case) in actual.decisions.iter().zip(&fixture.cases) {
        if actual.revision != case.expected.revision
            || actual.device_id != case.expected.device_id
            || actual.instance_id != case.expected.instance_id
            || actual.matches_requirements != case.expected.matches_requirements
            || actual.exclusion_reasons != case.expected.exclusion_reasons
        {
            return Err(format!("placement batch decision mismatch for {}", case.name).into());
        }
    }
    Ok(())
}
