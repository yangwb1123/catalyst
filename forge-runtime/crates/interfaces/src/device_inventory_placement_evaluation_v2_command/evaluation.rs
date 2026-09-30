use super::validation::{validate_observation, validate_owner};
use super::wire::{Authority, DecisionOutput, Fixture, Output, Requirements};
use super::{
    EVALUATION_MODE, MAX_DECISIONS, MAX_SAFE_INTEGER, NOTICE, SCHEMA_VERSION, SOURCE_SCHEMA_VERSION,
};
use forge_runtime_domain::{
    DevicePlacementPolicy, DevicePlacementRequirements, PersistedInventoryPlacementBatchAuthority,
    PersistedInventoryPlacementV2Evaluation, TenantId, evaluate_persisted_inventory_observation_v2,
};
use std::error::Error;
pub(super) fn evaluate(fixture: Fixture) -> Result<Output, Box<dyn Error>> {
    validate_fixture(&fixture)?;
    let owner = fixture.evaluation_owner.snapshot();
    validate_owner(&fixture.evaluation_owner)?;
    TenantId::parse(owner.tenant_id.clone())?;
    validate_observation(&fixture.observation, &owner)?;
    let requirements = requirements(&fixture.requirements)?;
    let actual = evaluate_persisted_inventory_observation_v2(
        &fixture.observation,
        &owner,
        requirements,
        fixture.evaluated_at_ms,
    )
    .map_err(|error| error.to_string())?;
    validate_evaluation(&actual, &fixture)?;
    Ok(Output {
        schema_version: SCHEMA_VERSION,
        evaluation_mode: EVALUATION_MODE,
        source_schema_version: SOURCE_SCHEMA_VERSION,
        evaluation_owner: fixture.evaluation_owner,
        evaluated_at_ms: fixture.evaluated_at_ms,
        notice: NOTICE,
        decisions: actual
            .decisions
            .into_iter()
            .map(|decision| DecisionOutput {
                revision: decision.revision,
                generation: decision.generation,
                heartbeat_sequence: decision.heartbeat_sequence,
                device_id: decision.device_id,
                instance_id: decision.instance_id,
                reservation_state: decision.reservation_state,
                gpu_count: decision.gpu_count,
                available_gpu_memory_bytes: decision.available_gpu_memory_bytes,
                matches_requirements: decision.matches_requirements,
                exclusion_reasons: decision.exclusion_reasons,
                owner_declaration_unverified: true,
                device_attributes_unverified: true,
            })
            .collect(),
        eligible_candidate_count: actual.eligible_candidate_count,
        selected_device_id: None,
        selected_instance_id: None,
        authority: Authority::default(),
    })
}

fn validate_fixture(fixture: &Fixture) -> Result<(), Box<dyn Error>> {
    if fixture.schema_version != SCHEMA_VERSION
        || fixture.evaluation_mode != EVALUATION_MODE
        || fixture.source_schema_version != SOURCE_SCHEMA_VERSION
        || fixture.notice != NOTICE
        || fixture.evaluated_at_ms == 0
        || fixture.evaluated_at_ms > MAX_SAFE_INTEGER
        || fixture.expected.len() > MAX_DECISIONS
        || fixture.selected_device_id.is_some()
        || fixture.selected_instance_id.is_some()
        || fixture.authority != Authority::default()
    {
        return Err(
            "v2 placement-evaluation input is not a bounded pure read-only contract".into(),
        );
    }
    if !fixture.requirements.gpu.runtime.is_empty()
        || fixture.observation.schema_version != SOURCE_SCHEMA_VERSION
        || fixture.observation.evaluated_at_ms != fixture.evaluated_at_ms
    {
        return Err("v2 placement-evaluation input has unsupported binding".into());
    }
    Ok(())
}

fn validate_evaluation(
    actual: &PersistedInventoryPlacementV2Evaluation,
    fixture: &Fixture,
) -> Result<(), Box<dyn Error>> {
    if actual.decisions.len() != fixture.expected.len()
        || actual.eligible_candidate_count != fixture.eligible_candidate_count
        || actual.selected_device_id.is_some()
        || actual.selected_instance_id.is_some()
        || actual.authority != PersistedInventoryPlacementBatchAuthority::default()
        || actual.notice != NOTICE
    {
        return Err("v2 placement-evaluation output drifted from the fixture".into());
    }
    for (actual, expected) in actual.decisions.iter().zip(&fixture.expected) {
        if actual.revision != expected.revision
            || actual.generation != expected.generation
            || actual.heartbeat_sequence != expected.heartbeat_sequence
            || actual.device_id != expected.device_id
            || actual.instance_id != expected.instance_id
            || actual.reservation_state != expected.reservation_state
            || actual.gpu_count != expected.gpu_count
            || actual.available_gpu_memory_bytes != expected.available_gpu_memory_bytes
            || actual.matches_requirements != expected.matches_requirements
            || actual.exclusion_reasons != expected.exclusion_reasons
        {
            return Err(format!(
                "v2 placement-evaluation decision mismatch for {}",
                expected.device_id
            )
            .into());
        }
    }
    Ok(())
}
fn requirements(value: &Requirements) -> Result<DevicePlacementRequirements, Box<dyn Error>> {
    if !value.gpu.runtime.is_empty() {
        return Err("v2 placement-evaluation GPU runtime is unsupported".into());
    }
    let policy = DevicePlacementPolicy::new()
        .with_allowed_data_residency_zones(value.data_residency_zones.clone())?
        .with_minimum_trust_zone(&value.minimum_trust_zone)?
        .with_sandbox_floor(&value.sandbox_floor)?
        .with_concurrency_slots(value.concurrency_slots)?;
    Ok(DevicePlacementRequirements::new(
        Some(&value.os),
        Some(&value.architecture),
        value.min_cpu_cores,
        value.min_memory_bytes,
        value.min_storage_bytes,
        vec![value.runtime.clone()],
        usize::from(value.gpu.required),
        value.gpu.min_memory_bytes,
    )?
    .with_policy(policy))
}
