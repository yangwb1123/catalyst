use super::*;
use serde::Deserialize;

const FIXTURE: &str = include_str!(
    "../../../../../../docs/contracts/fixtures/forge-device-enrollment-heartbeat-lifecycle-persistence-v1.json"
);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    notice: String,
    authority: Authority,
    owner: OwnerFixture,
    device: DeviceFixture,
    capabilities: CapabilitiesFixture,
    images: Vec<ImageFixture>,
    cases: Vec<CaseFixture>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
struct Authority {
    identity_verified: bool,
    challenge_consumed: bool,
    heartbeat_persisted: bool,
    inventory_authoritative: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerFixture {
    issuer: String,
    subject: String,
    tenant_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceFixture {
    device_id: String,
    approval_state: String,
    credential_state: String,
    cordon_state: String,
    reservation_state: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CapabilitiesFixture {
    os: String,
    architecture: String,
    cpu_cores: u32,
    available_cpu_cores: u32,
    memory_bytes: u64,
    available_memory_bytes: u64,
    storage_bytes: u64,
    available_storage_bytes: u64,
    gpus: Vec<GpuFixture>,
    runtimes: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GpuFixture {
    id: String,
    vendor: String,
    memory_bytes: u64,
    available_memory_bytes: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImageFixture {
    name: String,
    revision: u64,
    heartbeat: RunnerFixture,
    inventory: InventoryFixture,
    expected: ImageExpected,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunnerFixture {
    revision: u64,
    device_id: String,
    instance_id: String,
    generation: u64,
    heartbeat_sequence: u64,
    server_observed_at_ms: u64,
    capability_lease_expires_at_ms: u64,
    liveness: String,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct InventoryFixture {
    revision: u64,
    device_id: String,
    runner_device_id: String,
    runner_instance_id: String,
    runner_generation: u64,
    runner_heartbeat_sequence: u64,
    runner_server_observed_at_ms: u64,
    runner_capability_lease_expires_at_ms: u64,
    runner_liveness: String,
    reserved: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImageExpected {
    accepted: bool,
    revision: u64,
    heartbeat_revision: u64,
    inventory_revision: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CaseFixture {
    name: String,
    outer_revision: u64,
    heartbeat_revision: u64,
    inventory_revision: u64,
    #[serde(default)]
    runner_device_id: Option<String>,
    #[serde(default)]
    runner_generation: Option<u64>,
    #[serde(default)]
    owner_subject: Option<String>,
    expected: CaseExpected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CaseExpected {
    accepted: bool,
    error: Option<String>,
}

struct RestoredImage {
    revision: u64,
    heartbeat: PersistedRunnerInstance,
    inventory: PersistedInventoryState,
}

#[test]
fn enrollment_heartbeat_lifecycle_persistence_contract_fixture() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(
        fixture.schema_version,
        "forge.device-enrollment-heartbeat-lifecycle-persistence/v1"
    );
    assert_eq!(fixture.evaluation_mode, "pure_joined_value_replacement");
    assert!(fixture.notice.contains("no storage I/O"));
    assert!(!fixture.authority.identity_verified);
    assert!(!fixture.authority.challenge_consumed);
    assert!(!fixture.authority.heartbeat_persisted);
    assert!(!fixture.authority.inventory_authoritative);
    assert!(!fixture.authority.reservation_created);
    assert!(!fixture.authority.execution_authorized);
    assert!(!fixture.authority.dispatch_performed);
    assert_eq!(fixture.device.device_id, "device-a");
    assert_eq!(fixture.device.approval_state, "approved");
    assert_eq!(fixture.device.credential_state, "active");
    assert_eq!(fixture.device.cordon_state, "clear");
    assert_eq!(fixture.device.reservation_state, "none");
    assert_eq!(fixture.images.len(), 2);
    assert_eq!(fixture.cases.len(), 7);

    for image in &fixture.images {
        assert!(image.expected.accepted, "{}", image.name);
        assert_eq!(image.inventory.device_id, fixture.device.device_id);
        assert_eq!(image.inventory.runner_device_id, image.heartbeat.device_id);
        assert_eq!(
            image.inventory.runner_instance_id,
            image.heartbeat.instance_id
        );
        assert_eq!(
            image.inventory.runner_generation,
            image.heartbeat.generation
        );
        assert_eq!(
            image.inventory.runner_heartbeat_sequence,
            image.heartbeat.heartbeat_sequence
        );
        assert_eq!(
            image.inventory.runner_server_observed_at_ms,
            image.heartbeat.server_observed_at_ms
        );
        assert_eq!(
            image.inventory.runner_capability_lease_expires_at_ms,
            image.heartbeat.capability_lease_expires_at_ms
        );
        assert_eq!(image.inventory.runner_liveness, image.heartbeat.liveness);
        let restored = restore_image(&fixture, image, None, None, None).unwrap();
        assert_eq!(restored.revision, image.expected.revision, "{}", image.name);
        assert_eq!(
            restored.heartbeat.revision(),
            image.expected.heartbeat_revision,
            "{}",
            image.name
        );
        assert_eq!(
            restored.inventory.revision(),
            image.expected.inventory_revision,
            "{}",
            image.name
        );
        assert_eq!(
            restored.heartbeat.instance().device_id(),
            restored.inventory.runner().device_id(),
            "{}",
            image.name
        );
        assert_eq!(
            restored.heartbeat.instance().generation(),
            restored.inventory.runner().generation(),
            "{}",
            image.name
        );
        assert_eq!(
            restored.heartbeat.instance().heartbeat_sequence(),
            restored.inventory.runner().heartbeat_sequence(),
            "{}",
            image.name
        );
    }

    let initial = &fixture.images[0];
    for case in &fixture.cases {
        let result = restore_image(
            &fixture,
            initial,
            Some(case.outer_revision),
            Some(case.heartbeat_revision),
            Some(case.inventory_revision),
        );
        let result = match (
            case.runner_device_id.as_deref(),
            case.runner_generation,
            case.owner_subject.as_deref(),
        ) {
            (Some(device_id), generation, owner_subject) => restore_image_with_overrides(
                &fixture,
                initial,
                case.outer_revision,
                case.heartbeat_revision,
                case.inventory_revision,
                Some(device_id),
                generation,
                owner_subject,
            ),
            (None, Some(generation), owner_subject) => restore_image_with_overrides(
                &fixture,
                initial,
                case.outer_revision,
                case.heartbeat_revision,
                case.inventory_revision,
                None,
                Some(generation),
                owner_subject,
            ),
            (None, None, Some(owner_subject)) => restore_image_with_overrides(
                &fixture,
                initial,
                case.outer_revision,
                case.heartbeat_revision,
                case.inventory_revision,
                None,
                None,
                Some(owner_subject),
            ),
            (None, None, None) => result,
        };
        match (result, case.expected.accepted) {
            (Ok(_), true) => {}
            (Ok(_), false) => panic!("{}: malformed image was accepted", case.name),
            (Err(error), false) => assert_eq!(
                error,
                case.expected.error.as_deref().unwrap(),
                "{}",
                case.name
            ),
            (Err(error), true) => panic!("{}: image rejected with {error}", case.name),
        }
    }
}

fn restore_image(
    fixture: &Fixture,
    image: &ImageFixture,
    outer_revision: Option<u64>,
    heartbeat_revision: Option<u64>,
    inventory_revision: Option<u64>,
) -> Result<RestoredImage, &'static str> {
    restore_image_with_overrides(
        fixture,
        image,
        outer_revision.unwrap_or(image.revision),
        heartbeat_revision.unwrap_or(image.heartbeat.revision),
        inventory_revision.unwrap_or(image.inventory.revision),
        None,
        None,
        None,
    )
}

fn restore_image_with_overrides(
    fixture: &Fixture,
    image: &ImageFixture,
    outer_revision: u64,
    heartbeat_revision: u64,
    inventory_revision: u64,
    runner_device_override: Option<&str>,
    runner_generation_override: Option<u64>,
    owner_subject_override: Option<&str>,
) -> Result<RestoredImage, &'static str> {
    if outer_revision == 0
        || heartbeat_revision == 0
        || inventory_revision == 0
        || outer_revision != heartbeat_revision
        || outer_revision != inventory_revision
    {
        return Err("invalid_persisted_state");
    }

    let capabilities = capabilities(&fixture.capabilities);
    let heartbeat_runner = runner(&image.heartbeat, &capabilities, None, None)?;
    let persisted_heartbeat =
        PersistedRunnerInstance::restore(heartbeat_revision, heartbeat_runner)
            .map_err(persistence_error_code)?;

    let inventory_runner = runner(
        &image.heartbeat,
        &capabilities,
        runner_device_override,
        runner_generation_override,
    )?;
    let inventory_device_id = image.inventory.device_id.as_str();
    if inventory_device_id != fixture.device.device_id {
        return Err("invalid_device_record");
    }
    let owner = SnapshotOwner {
        issuer: fixture.owner.issuer.clone(),
        subject: fixture.owner.subject.clone(),
        tenant_id: fixture.owner.tenant_id.clone(),
    };
    let inventory_owner = SnapshotOwner {
        subject: owner_subject_override
            .unwrap_or(owner.subject.as_str())
            .to_owned(),
        ..owner.clone()
    };
    let device = Device::restore(
        DeviceId::parse(fixture.device.device_id.clone()).map_err(|_| "invalid_device_record")?,
        TenantId::parse(fixture.owner.tenant_id.clone()).map_err(|_| "invalid_device_record")?,
        approval(&fixture.device.approval_state)?,
        fixture.device.cordon_state == "cordoned",
    );
    let inventory_device = PersistedInventoryDevice::restore(
        device,
        inventory_owner.clone(),
        image.inventory.reserved,
    );
    let inventory =
        restore_persisted_inventory(inventory_revision, inventory_device, inventory_runner)
            .map_err(persistence_error_code)?;
    if inventory_owner != owner {
        return Err("owner_mismatch");
    }
    if persisted_heartbeat.instance().device_id().as_str() != fixture.device.device_id
        || inventory.runner().device_id().as_str() != fixture.device.device_id
        || persisted_heartbeat.instance().instance_id() != inventory.runner().instance_id()
        || persisted_heartbeat.instance().generation() != inventory.runner().generation()
        || persisted_heartbeat.instance().heartbeat_sequence()
            != inventory.runner().heartbeat_sequence()
        || persisted_heartbeat.instance().server_observed_at_ms()
            != inventory.runner().server_observed_at_ms()
        || persisted_heartbeat
            .instance()
            .capability_lease_expires_at_ms()
            != inventory.runner().capability_lease_expires_at_ms()
        || persisted_heartbeat.instance().liveness() != inventory.runner().liveness()
        || persisted_heartbeat.instance().capabilities() != inventory.runner().capabilities()
    {
        return Err("invalid_persisted_state");
    }
    Ok(RestoredImage {
        revision: outer_revision,
        heartbeat: persisted_heartbeat,
        inventory,
    })
}

fn runner(
    source: &RunnerFixture,
    capabilities: &CapabilitySnapshot,
    device_override: Option<&str>,
    generation_override: Option<u64>,
) -> Result<RunnerInstance, &'static str> {
    RunnerInstance::restore(
        DeviceId::parse(device_override.unwrap_or(&source.device_id).to_owned())
            .map_err(|_| "invalid_runner_record")?,
        RunnerInstanceId::parse(source.instance_id.clone()).map_err(|_| "invalid_runner_record")?,
        generation_override.unwrap_or(source.generation),
        source.heartbeat_sequence,
        source.server_observed_at_ms,
        source.capability_lease_expires_at_ms,
        liveness(&source.liveness)?,
        capabilities.clone(),
    )
    .map_err(|_| "invalid_runner_record")
}

fn approval(value: &str) -> Result<DeviceApprovalState, &'static str> {
    match value {
        "approved" => Ok(DeviceApprovalState::Approved),
        "pending" => Ok(DeviceApprovalState::Pending),
        "revoked" => Ok(DeviceApprovalState::Revoked),
        _ => Err("invalid_device_record"),
    }
}

fn liveness(value: &str) -> Result<RunnerLiveness, &'static str> {
    match value {
        "online" => Ok(RunnerLiveness::Online),
        "offline" => Ok(RunnerLiveness::Offline),
        _ => Err("invalid_runner_record"),
    }
}

fn capabilities(value: &CapabilitiesFixture) -> CapabilitySnapshot {
    let gpus = value
        .gpus
        .iter()
        .map(|gpu| {
            GpuCapability::new(
                &gpu.id,
                &gpu.vendor,
                gpu.memory_bytes,
                gpu.available_memory_bytes,
            )
            .unwrap()
        })
        .collect();
    CapabilitySnapshot::new(
        &value.os,
        &value.architecture,
        value.cpu_cores,
        value.available_cpu_cores,
        value.memory_bytes,
        value.available_memory_bytes,
        value.storage_bytes,
        value.available_storage_bytes,
        gpus,
        value.runtimes.clone(),
    )
    .unwrap()
}

fn persistence_error_code(error: PersistenceError) -> &'static str {
    match error {
        PersistenceError::RevisionConflict => "revision_conflict",
        PersistenceError::InvalidPersistedState => "invalid_persisted_state",
        PersistenceError::RevisionOverflow => "revision_overflow",
        PersistenceError::InvalidDeviceRecord => "invalid_device_record",
        PersistenceError::InvalidRunnerRecord => "invalid_runner_record",
        PersistenceError::RunnerDeviceMismatch => "runner_device_mismatch",
        PersistenceError::InventoryOwnerMismatch => "owner_mismatch",
        PersistenceError::DeviceBindingChanged => "device_binding_changed",
        PersistenceError::InvalidEvaluationOwner => "invalid_evaluation_owner",
        PersistenceError::InventoryStatus(_) | PersistenceError::DeviceHeartbeat(_) => {
            "invalid_persisted_state"
        }
    }
}
