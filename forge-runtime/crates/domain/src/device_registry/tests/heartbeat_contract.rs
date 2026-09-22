use super::*;
use serde::Deserialize;

const FIXTURE: &str = include_str!(
    "../../../../../../docs/contracts/fixtures/forge-device-heartbeat-contract-v1.json"
);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    owner_declaration: OwnerFixture,
    device: DeviceFixture,
    capabilities: CapabilitiesFixture,
    authority: AuthorityFixture,
    cases: Vec<HeartbeatCaseFixture>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerFixture {
    tenant_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceFixture {
    device_id: String,
    tenant_id: String,
    approval_state: String,
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
    runtimes: Vec<String>,
}

#[derive(Deserialize)]
#[allow(clippy::struct_excessive_bools)]
#[serde(deny_unknown_fields)]
struct AuthorityFixture {
    identity_verified: bool,
    heartbeat_persisted: bool,
    inventory_authoritative: bool,
    execution_authorized: bool,
    reservation_created: bool,
    dispatch_performed: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HeartbeatCaseFixture {
    name: String,
    server_observed_at_ms: u64,
    lease_ttl_ms: u64,
    #[serde(default)]
    device_approval_state: Option<String>,
    current: Option<CurrentFixture>,
    heartbeat: HeartbeatFixture,
    expected: ExpectedFixture,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CurrentFixture {
    device_id: String,
    instance_id: String,
    generation: u64,
    heartbeat_sequence: u64,
    server_observed_at_ms: u64,
    capability_lease_expires_at_ms: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HeartbeatFixture {
    device_id: String,
    instance_id: String,
    generation: u64,
    sequence: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedFixture {
    accepted: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    generation: Option<u64>,
    #[serde(default)]
    heartbeat_sequence: Option<u64>,
    #[serde(default)]
    server_observed_at_ms: Option<u64>,
    #[serde(default)]
    capability_lease_expires_at_ms: Option<u64>,
}

#[test]
fn heartbeat_contract_fixture() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
    assert_fixture_envelope(&fixture);
    let capabilities = build_capabilities(&fixture.capabilities);
    for case in fixture.cases {
        let device = build_device(&fixture.device, case.device_approval_state.as_deref());
        let current = case
            .current
            .as_ref()
            .map(|current| build_current(current, &capabilities));
        let heartbeat = RunnerHeartbeat::new(
            DeviceId::parse(case.heartbeat.device_id).unwrap(),
            RunnerInstanceId::parse(case.heartbeat.instance_id).unwrap(),
            case.heartbeat.generation,
            case.heartbeat.sequence,
            capabilities.clone(),
        )
        .unwrap();
        let result = apply_device_heartbeat(
            &device,
            current.as_ref(),
            &heartbeat,
            case.server_observed_at_ms,
            case.lease_ttl_ms,
        );
        assert_case(&case.name, &case.expected, result);
    }
}

fn assert_fixture_envelope(fixture: &Fixture) {
    assert_eq!(fixture.schema_version, "forge.device-heartbeat-contract/v1");
    assert_eq!(fixture.evaluation_mode, "pure_reference_only");
    assert_eq!(fixture.owner_declaration.tenant_id, "tenant-1");
    assert_eq!(fixture.device.device_id, "device-a");
    assert_eq!(fixture.device.tenant_id, "tenant-1");
    assert_eq!(fixture.device.approval_state, "approved");
    assert_eq!(fixture.capabilities.os, "linux");
    assert_eq!(fixture.capabilities.architecture, "amd64");
    assert_eq!(fixture.capabilities.cpu_cores, 8);
    assert_eq!(fixture.capabilities.available_cpu_cores, 8);
    assert_eq!(fixture.capabilities.memory_bytes, 16_384);
    assert_eq!(fixture.capabilities.available_memory_bytes, 16_384);
    assert_eq!(fixture.capabilities.storage_bytes, 8_192);
    assert_eq!(fixture.capabilities.available_storage_bytes, 8_192);
    assert_eq!(fixture.capabilities.runtimes, ["oci"]);
    assert!(!fixture.authority.identity_verified);
    assert!(!fixture.authority.heartbeat_persisted);
    assert!(!fixture.authority.inventory_authoritative);
    assert!(!fixture.authority.execution_authorized);
    assert!(!fixture.authority.reservation_created);
    assert!(!fixture.authority.dispatch_performed);
    assert_eq!(fixture.cases.len(), 12);
}

fn build_capabilities(fixture: &CapabilitiesFixture) -> CapabilitySnapshot {
    CapabilitySnapshot::new(
        fixture.os.clone(),
        fixture.architecture.clone(),
        fixture.cpu_cores,
        fixture.available_cpu_cores,
        fixture.memory_bytes,
        fixture.available_memory_bytes,
        fixture.storage_bytes,
        fixture.available_storage_bytes,
        Vec::new(),
        fixture.runtimes.clone(),
    )
    .unwrap()
}

fn build_device(fixture: &DeviceFixture, override_state: Option<&str>) -> Device {
    let approval = match override_state.unwrap_or(&fixture.approval_state) {
        "approved" => DeviceApprovalState::Approved,
        "pending" => DeviceApprovalState::Pending,
        "revoked" => DeviceApprovalState::Revoked,
        value => panic!("unsupported approval state {value:?}"),
    };
    Device::restore(
        DeviceId::parse(fixture.device_id.clone()).unwrap(),
        TenantId::parse(fixture.tenant_id.clone()).unwrap(),
        approval,
        false,
    )
}

fn build_current(fixture: &CurrentFixture, capabilities: &CapabilitySnapshot) -> RunnerInstance {
    RunnerInstance::restore(
        DeviceId::parse(fixture.device_id.clone()).unwrap(),
        RunnerInstanceId::parse(fixture.instance_id.clone()).unwrap(),
        fixture.generation,
        fixture.heartbeat_sequence,
        fixture.server_observed_at_ms,
        fixture.capability_lease_expires_at_ms,
        RunnerLiveness::Online,
        capabilities.clone(),
    )
    .unwrap()
}

fn assert_case(
    name: &str,
    expected: &ExpectedFixture,
    result: Result<RunnerInstance, DeviceHeartbeatError>,
) {
    match (expected.accepted, result) {
        (true, Ok(instance)) => {
            assert_eq!(
                instance.generation(),
                expected.generation.unwrap(),
                "{name}"
            );
            assert_eq!(
                instance.heartbeat_sequence(),
                expected.heartbeat_sequence.unwrap(),
                "{name}"
            );
            assert_eq!(
                instance.server_observed_at_ms(),
                expected.server_observed_at_ms.unwrap(),
                "{name}"
            );
            assert_eq!(
                instance.capability_lease_expires_at_ms(),
                expected.capability_lease_expires_at_ms.unwrap(),
                "{name}"
            );
        }
        (false, Err(error)) => assert_eq!(
            heartbeat_error_code(error),
            expected.error.as_deref().unwrap(),
            "{name}"
        ),
        (accepted, result) => panic!("{name}: accepted={accepted}, result={result:?}"),
    }
}

fn heartbeat_error_code(error: DeviceHeartbeatError) -> &'static str {
    match error {
        DeviceHeartbeatError::DeviceMismatch => "device_mismatch",
        DeviceHeartbeatError::DeviceRevoked => "device_revoked",
        DeviceHeartbeatError::GenerationMustStartAtOne => "generation_must_start_at_one",
        DeviceHeartbeatError::OldGeneration => "old_generation",
        DeviceHeartbeatError::GenerationSkipped => "generation_skipped",
        DeviceHeartbeatError::InstanceChangedWithinGeneration => {
            "instance_changed_within_generation"
        }
        DeviceHeartbeatError::SequenceMustStartAtOne => "sequence_must_start_at_one",
        DeviceHeartbeatError::SequenceNotIncreasing => "sequence_not_increasing",
        DeviceHeartbeatError::ServerTimeWentBackwards => "server_time_went_backwards",
        DeviceHeartbeatError::InvalidLeaseDuration => "invalid_lease_duration",
        DeviceHeartbeatError::LeaseExpiryOverflow => "lease_expiry_overflow",
    }
}
