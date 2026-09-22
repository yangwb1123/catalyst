use super::*;
use serde::Deserialize;

const FIXTURE: &str = include_str!(
    "../../../../../../docs/contracts/fixtures/forge-device-inventory-placement-evaluation-v2.json"
);

#[derive(Debug, Deserialize)]
struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    source_schema_version: String,
    notice: String,
    evaluation_owner: OwnerFixture,
    evaluated_at_ms: u64,
    requirements: RequirementFixture,
    observation: PersistedInventoryObservationV2,
    expected: Vec<ExpectedDecision>,
    eligible_candidate_count: usize,
}

#[derive(Debug, Deserialize, Clone)]
struct OwnerFixture {
    issuer: String,
    subject: String,
    tenant_id: String,
}

#[derive(Debug, Deserialize)]
struct RequirementFixture {
    os: String,
    architecture: String,
    min_cpu_cores: u32,
    min_memory_bytes: u64,
    min_storage_bytes: u64,
    runtime: String,
    gpu: GpuRequirementFixture,
    data_residency_zones: Vec<String>,
    minimum_trust_zone: String,
    sandbox_floor: String,
    concurrency_slots: u16,
}

#[derive(Debug, Deserialize)]
struct GpuRequirementFixture {
    required: bool,
    min_memory_bytes: u64,
    runtime: String,
}

#[derive(Debug, Deserialize)]
struct ExpectedDecision {
    revision: u64,
    generation: u64,
    heartbeat_sequence: u64,
    device_id: String,
    instance_id: String,
    reservation_state: String,
    gpu_count: usize,
    available_gpu_memory_bytes: u64,
    matches_requirements: bool,
    exclusion_reasons: Vec<String>,
}

fn requirements(value: &RequirementFixture) -> DevicePlacementRequirements {
    assert!(value.gpu.runtime.is_empty());
    let policy = DevicePlacementPolicy::new()
        .with_allowed_data_residency_zones(value.data_residency_zones.clone())
        .unwrap()
        .with_minimum_trust_zone(&value.minimum_trust_zone)
        .unwrap()
        .with_sandbox_floor(&value.sandbox_floor)
        .unwrap()
        .with_concurrency_slots(value.concurrency_slots)
        .unwrap();
    DevicePlacementRequirements::new(
        Some(&value.os),
        Some(&value.architecture),
        value.min_cpu_cores,
        value.min_memory_bytes,
        value.min_storage_bytes,
        vec![value.runtime.clone()],
        usize::from(value.gpu.required),
        value.gpu.min_memory_bytes,
    )
    .unwrap()
    .with_policy(policy)
}

#[test]
fn v2_evaluation_preserves_reservation_and_all_gpu_totals() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let owner = owner(&fixture.evaluation_owner);
    let actual = evaluate_persisted_inventory_observation_v2(
        &fixture.observation,
        &owner,
        requirements(&fixture.requirements),
        fixture.evaluated_at_ms,
    )
    .unwrap();
    assert_eq!(actual.schema_version, fixture.schema_version);
    assert_eq!(actual.evaluation_mode, fixture.evaluation_mode);
    assert_eq!(actual.source_schema_version, fixture.source_schema_version);
    assert_eq!(actual.notice, fixture.notice);
    assert_eq!(actual.owner, owner);
    assert_eq!(
        actual.eligible_candidate_count,
        fixture.eligible_candidate_count
    );
    assert!(actual.selected_device_id.is_none());
    assert!(actual.selected_instance_id.is_none());
    assert_eq!(
        actual.authority,
        PersistedInventoryPlacementBatchAuthority::default()
    );
    assert_eq!(actual.decisions.len(), fixture.expected.len());
    for (actual, expected) in actual.decisions.iter().zip(fixture.expected) {
        assert_eq!(actual.revision, expected.revision);
        assert_eq!(actual.generation, expected.generation);
        assert_eq!(actual.heartbeat_sequence, expected.heartbeat_sequence);
        assert_eq!(actual.device_id, expected.device_id);
        assert_eq!(actual.instance_id, expected.instance_id);
        assert_eq!(actual.reservation_state, expected.reservation_state);
        assert_eq!(actual.gpu_count, expected.gpu_count);
        assert_eq!(
            actual.available_gpu_memory_bytes,
            expected.available_gpu_memory_bytes
        );
        assert_eq!(actual.matches_requirements, expected.matches_requirements);
        assert_eq!(actual.exclusion_reasons, expected.exclusion_reasons);
        assert!(actual.owner_declaration_unverified);
        assert!(actual.device_attributes_unverified);
    }
}

#[test]
fn v2_evaluation_rejects_foreign_owner_and_does_not_select() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let mut owner = owner(&fixture.evaluation_owner);
    owner.subject = "foreign".to_owned();
    let result = evaluate_persisted_inventory_observation_v2(
        &fixture.observation,
        &owner,
        requirements(&fixture.requirements),
        fixture.evaluated_at_ms,
    );
    assert_eq!(
        result,
        Err(PersistedInventoryPlacementV2Error::InvalidObservation)
    );
}

#[test]
fn v2_evaluation_rejects_duplicate_devices() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let owner = owner(&fixture.evaluation_owner);
    let mut observation = fixture.observation.clone();
    let duplicate = observation.devices[0].clone();
    observation.devices.insert(1, duplicate);
    let result = evaluate_persisted_inventory_observation_v2(
        &observation,
        &owner,
        requirements(&fixture.requirements),
        fixture.evaluated_at_ms,
    );
    assert_eq!(
        result,
        Err(PersistedInventoryPlacementV2Error::DuplicateDevice)
    );
}

#[test]
fn v2_evaluation_rejects_noncanonical_observation_attributes() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let owner = owner(&fixture.evaluation_owner);

    let mut unknown_cordon = fixture.observation.clone();
    unknown_cordon.devices[0].device.cordon_state = "unknown".to_owned();
    assert_eq!(
        evaluate_persisted_inventory_observation_v2(
            &unknown_cordon,
            &owner,
            requirements(&fixture.requirements),
            fixture.evaluated_at_ms,
        ),
        Err(PersistedInventoryPlacementV2Error::InvalidCandidate)
    );

    let mut uppercase_os = fixture.observation.clone();
    uppercase_os.devices[0].device.os = "LINUX".to_owned();
    assert_eq!(
        evaluate_persisted_inventory_observation_v2(
            &uppercase_os,
            &owner,
            requirements(&fixture.requirements),
            fixture.evaluated_at_ms,
        ),
        Err(PersistedInventoryPlacementV2Error::InvalidCandidate)
    );

    let mut unsorted_gpus = fixture.observation.clone();
    unsorted_gpus.devices[0].device.gpus.swap(0, 1);
    assert_eq!(
        evaluate_persisted_inventory_observation_v2(
            &unsorted_gpus,
            &owner,
            requirements(&fixture.requirements),
            fixture.evaluated_at_ms,
        ),
        Err(PersistedInventoryPlacementV2Error::InvalidCandidate)
    );

    let mut unsorted_devices = fixture.observation.clone();
    unsorted_devices.devices.swap(0, 1);
    assert_eq!(
        evaluate_persisted_inventory_observation_v2(
            &unsorted_devices,
            &owner,
            requirements(&fixture.requirements),
            fixture.evaluated_at_ms,
        ),
        Err(PersistedInventoryPlacementV2Error::InvalidObservation)
    );
}

#[test]
fn v2_evaluation_enforces_capability_lease_ttl_bounds() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let owner = owner(&fixture.evaluation_owner);
    let mut below_minimum = fixture.observation.clone();
    below_minimum.devices[0].device.lease_expires_at_ms = 100_999;
    assert_eq!(
        evaluate_persisted_inventory_observation_v2(
            &below_minimum,
            &owner,
            requirements(&fixture.requirements),
            fixture.evaluated_at_ms,
        ),
        Err(PersistedInventoryPlacementV2Error::InvalidCandidate)
    );

    let mut above_maximum = fixture.observation.clone();
    above_maximum.devices[0].device.lease_expires_at_ms = 700_001;
    assert_eq!(
        evaluate_persisted_inventory_observation_v2(
            &above_maximum,
            &owner,
            requirements(&fixture.requirements),
            fixture.evaluated_at_ms,
        ),
        Err(PersistedInventoryPlacementV2Error::InvalidCandidate)
    );

    let mut minimum = fixture.observation.clone();
    minimum.devices[0].device.lease_expires_at_ms = 101_000;
    assert!(
        evaluate_persisted_inventory_observation_v2(
            &minimum,
            &owner,
            requirements(&fixture.requirements),
            fixture.evaluated_at_ms,
        )
        .is_ok()
    );

    let mut maximum = fixture.observation.clone();
    maximum.devices[0].device.lease_expires_at_ms = 700_000;
    assert!(
        evaluate_persisted_inventory_observation_v2(
            &maximum,
            &owner,
            requirements(&fixture.requirements),
            fixture.evaluated_at_ms,
        )
        .is_ok()
    );
}

#[test]
fn v2_evaluation_keeps_exhausted_cpu_and_memory_as_valid_candidates() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let owner = owner(&fixture.evaluation_owner);
    let mut observation = fixture.observation.clone();
    observation.devices[0].device.available_cpu_cores = 0;
    observation.devices[0].device.available_memory_bytes = 0;
    let evaluation = evaluate_persisted_inventory_observation_v2(
        &observation,
        &owner,
        requirements(&fixture.requirements),
        fixture.evaluated_at_ms,
    )
    .expect("zero available resources remain a valid observation");
    assert_eq!(evaluation.decisions[0].device_id, "device-a");
    assert!(!evaluation.decisions[0].matches_requirements);
    assert!(
        evaluation.decisions[0]
            .exclusion_reasons
            .contains(&"cpu_cores_insufficient".to_owned())
    );
    assert!(
        evaluation.decisions[0]
            .exclusion_reasons
            .contains(&"memory_insufficient".to_owned())
    );
}

#[test]
fn v2_evaluation_rejects_runtime_count_above_observation_bound() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let owner = owner(&fixture.evaluation_owner);
    let mut observation = fixture.observation.clone();
    observation.devices[0].device.runtimes =
        (0..33).map(|index| format!("runtime-{index:02}")).collect();
    assert_eq!(
        evaluate_persisted_inventory_observation_v2(
            &observation,
            &owner,
            requirements(&fixture.requirements),
            fixture.evaluated_at_ms,
        ),
        Err(PersistedInventoryPlacementV2Error::InvalidCandidate)
    );
}

fn owner(value: &OwnerFixture) -> SnapshotOwner {
    SnapshotOwner {
        issuer: value.issuer.clone(),
        subject: value.subject.clone(),
        tenant_id: value.tenant_id.clone(),
    }
}
