use super::*;
use serde::Deserialize;

const FIXTURE: &str = include_str!(
    "../../../../../../docs/contracts/fixtures/forge-device-heartbeat-persistence-contract-v1.json"
);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    authority: Authority,
    device: DeviceFixture,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[allow(clippy::struct_excessive_bools)]
#[serde(deny_unknown_fields)]
struct Authority {
    identity_verified: bool,
    heartbeat_persisted: bool,
    inventory_authoritative: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
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
struct Case {
    name: String,
    expected_revision: u64,
    #[serde(default)]
    device_approval_state: Option<String>,
    current: Option<Current>,
    heartbeat: HeartbeatFixture,
    server_observed_at_ms: u64,
    lease_ttl_ms: u64,
    expected: Expected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Current {
    revision: u64,
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
struct Expected {
    accepted: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    revision: Option<u64>,
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
fn heartbeat_persistence_contract_fixture() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
    assert_fixture_envelope(&fixture);
    let capabilities = capabilities();
    for case in fixture.cases {
        let device = device(&fixture.device, case.device_approval_state.as_deref());
        let current = case
            .current
            .as_ref()
            .map(|current| {
                PersistedRunnerInstance::restore(current.revision, instance(current, &capabilities))
            })
            .transpose();
        let current = match current {
            Ok(current) => current,
            Err(error) => {
                assert_eq!(
                    error.to_string(),
                    case.expected.error.as_deref().unwrap(),
                    "{}",
                    case.name
                );
                continue;
            }
        };
        let heartbeat = RunnerHeartbeat::new(
            DeviceId::parse(case.heartbeat.device_id.clone()).unwrap(),
            RunnerInstanceId::parse(case.heartbeat.instance_id.clone()).unwrap(),
            case.heartbeat.generation,
            case.heartbeat.sequence,
            capabilities.clone(),
        )
        .unwrap();
        let result = commit_device_heartbeat(
            &device,
            current.as_ref(),
            case.expected_revision,
            &heartbeat,
            case.server_observed_at_ms,
            case.lease_ttl_ms,
        );
        assert_case(&case.name, &case.expected, result);
    }
}

fn assert_fixture_envelope(fixture: &Fixture) {
    assert_eq!(
        fixture.schema_version,
        "forge.device-heartbeat-persistence-contract/v1"
    );
    assert_eq!(fixture.evaluation_mode, "pure_compare_and_swap_plan");
    assert!(!fixture.authority.identity_verified);
    assert!(!fixture.authority.heartbeat_persisted);
    assert!(!fixture.authority.inventory_authoritative);
    assert!(!fixture.authority.reservation_created);
    assert!(!fixture.authority.execution_authorized);
    assert!(!fixture.authority.dispatch_performed);
    assert_eq!(fixture.device.device_id, "device-a");
    assert_eq!(fixture.device.tenant_id, "tenant-1");
    assert_eq!(fixture.device.approval_state, "approved");
    assert_eq!(fixture.cases.len(), 10);
}

fn device(fixture: &DeviceFixture, override_state: Option<&str>) -> Device {
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

fn capabilities() -> CapabilitySnapshot {
    CapabilitySnapshot::new(
        "linux",
        "amd64",
        8,
        8,
        16_384,
        16_384,
        8_192,
        8_192,
        Vec::new(),
        vec!["oci".to_owned()],
    )
    .unwrap()
}

fn instance(current: &Current, capabilities: &CapabilitySnapshot) -> RunnerInstance {
    RunnerInstance::restore(
        DeviceId::parse(current.device_id.clone()).unwrap(),
        RunnerInstanceId::parse(current.instance_id.clone()).unwrap(),
        current.generation,
        current.heartbeat_sequence,
        current.server_observed_at_ms,
        current.capability_lease_expires_at_ms,
        RunnerLiveness::Online,
        capabilities.clone(),
    )
    .unwrap()
}

fn assert_case(
    name: &str,
    expected: &Expected,
    result: Result<PersistedRunnerInstance, PersistenceError>,
) {
    if expected.accepted {
        let state = result.unwrap_or_else(|error| panic!("{name} rejected: {error}"));
        assert_eq!(state.revision(), expected.revision.unwrap());
        assert_eq!(state.instance().generation(), expected.generation.unwrap());
        assert_eq!(
            state.instance().heartbeat_sequence(),
            expected.heartbeat_sequence.unwrap()
        );
        assert_eq!(
            state.instance().server_observed_at_ms(),
            expected.server_observed_at_ms.unwrap()
        );
        assert_eq!(
            state.instance().capability_lease_expires_at_ms(),
            expected.capability_lease_expires_at_ms.unwrap()
        );
        return;
    }
    let error = result.expect_err(name);
    assert_eq!(
        error.to_string(),
        expected.error.as_deref().unwrap(),
        "{name}"
    );
}
