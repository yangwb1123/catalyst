#![allow(clippy::too_many_lines)]

use super::*;
use serde::Deserialize;

const FIXTURE: &str = include_str!(
    "../../../../../../docs/contracts/fixtures/forge-device-inventory-persistence-v1.json"
);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    stale_after_ms: u64,
    authority: Authority,
    state: State,
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
struct State {
    revision: u64,
    device: DeviceState,
    runner: RunnerState,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceState {
    device_id: String,
    owner: Owner,
    approval_state: String,
    cordon_state: String,
    reservation_state: String,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Owner {
    issuer: String,
    subject: String,
    tenant_id: String,
}

impl Owner {
    fn snapshot(&self) -> SnapshotOwner {
        SnapshotOwner {
            issuer: self.issuer.clone(),
            subject: self.subject.clone(),
            tenant_id: self.tenant_id.clone(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RunnerState {
    device_id: String,
    instance_id: String,
    generation: u64,
    heartbeat_sequence: u64,
    server_observed_at_ms: u64,
    capability_lease_expires_at_ms: u64,
    liveness: String,
    capabilities: Capabilities,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Capabilities {
    #[serde(rename = "os")]
    operating_system: String,
    architecture: String,
    cpu_cores: u32,
    available_cpu_cores: u32,
    memory_bytes: u64,
    available_memory_bytes: u64,
    storage_bytes: u64,
    available_storage_bytes: u64,
    gpus: Vec<Gpu>,
    runtimes: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Gpu {
    id: String,
    vendor: String,
    memory_bytes: u64,
    available_memory_bytes: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    operation: String,
    #[serde(default)]
    evaluation_owner: Option<String>,
    #[serde(default)]
    evaluated_at_ms: Option<u64>,
    #[serde(default)]
    expected_revision: Option<u64>,
    #[serde(default)]
    state_revision: Option<u64>,
    #[serde(default)]
    runner_device_id: Option<String>,
    #[serde(default)]
    device_approval_state: Option<String>,
    #[serde(default)]
    device_cordon_state: Option<String>,
    #[serde(default)]
    runner_liveness: Option<String>,
    #[serde(default)]
    replacement: Replacement,
    expected: Expected,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Replacement {
    #[serde(default)]
    heartbeat_sequence: Option<u64>,
    #[serde(default)]
    server_observed_at_ms: Option<u64>,
    #[serde(default)]
    capability_lease_expires_at_ms: Option<u64>,
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
    device_id: Option<String>,
    #[serde(default)]
    instance_id: Option<String>,
    #[serde(default)]
    heartbeat_sequence: Option<u64>,
    #[serde(default)]
    server_observed_at_ms: Option<u64>,
    #[serde(default)]
    capability_lease_expires_at_ms: Option<u64>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    fresh: Option<bool>,
    #[serde(default)]
    declared_eligible: Option<bool>,
}

#[test]
fn persisted_inventory_contract_fixture() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(
        fixture.schema_version,
        "forge.device-inventory-persistence/v1"
    );
    assert_eq!(
        fixture.evaluation_mode,
        "pure_persisted_inventory_cas_projection"
    );
    assert_eq!(fixture.stale_after_ms, DEFAULT_STALE_AFTER_MS);
    assert!(!fixture.authority.identity_verified);
    assert!(!fixture.authority.heartbeat_persisted);
    assert!(!fixture.authority.inventory_authoritative);
    assert!(!fixture.authority.reservation_created);
    assert!(!fixture.authority.execution_authorized);
    assert!(!fixture.authority.dispatch_performed);
    assert_eq!(fixture.cases.len(), 12);

    for case in fixture.cases {
        if case.operation == "restore" && case.runner_device_id.is_some() {
            let device = Device::restore(
                DeviceId::parse(fixture.state.device.device_id.clone()).unwrap(),
                TenantId::parse(fixture.state.device.owner.tenant_id.clone()).unwrap(),
                DeviceApprovalState::Approved,
                false,
            );
            let persisted_device = PersistedInventoryDevice::restore(
                device,
                fixture.state.device.owner.snapshot(),
                false,
            );
            let runner = RunnerInstance::restore(
                DeviceId::parse(case.runner_device_id.as_deref().unwrap()).unwrap(),
                RunnerInstanceId::parse(fixture.state.runner.instance_id.clone()).unwrap(),
                fixture.state.runner.generation,
                fixture.state.runner.heartbeat_sequence,
                fixture.state.runner.server_observed_at_ms,
                fixture.state.runner.capability_lease_expires_at_ms,
                RunnerLiveness::Online,
                capabilities(&fixture.state.runner.capabilities),
            )
            .unwrap();
            let result =
                restore_persisted_inventory(fixture.state.revision, persisted_device, runner);
            assert_error(&case.name, &case.expected, result.map(|_| ()));
            continue;
        }
        let state = build_state(&fixture.state, &case);
        match case.operation.as_str() {
            "restore" => {
                let result = restore_persisted_inventory(
                    state.revision(),
                    state.device().clone(),
                    state.runner().clone(),
                );
                assert_error(&case.name, &case.expected, result.map(|_| ()));
            }
            "commit" => {
                let mut runner = state.runner().clone();
                if case.replacement.heartbeat_sequence.is_some()
                    || case.replacement.server_observed_at_ms.is_some()
                    || case.replacement.capability_lease_expires_at_ms.is_some()
                {
                    runner = RunnerInstance::restore(
                        runner.device_id().clone(),
                        runner.instance_id().clone(),
                        runner.generation(),
                        case.replacement
                            .heartbeat_sequence
                            .unwrap_or(runner.heartbeat_sequence()),
                        case.replacement
                            .server_observed_at_ms
                            .unwrap_or(runner.server_observed_at_ms()),
                        case.replacement
                            .capability_lease_expires_at_ms
                            .unwrap_or(runner.capability_lease_expires_at_ms()),
                        runner.liveness(),
                        runner.capabilities().clone(),
                    )
                    .unwrap();
                }
                let result = commit_persisted_inventory(
                    Some(&state),
                    case.expected_revision.unwrap(),
                    state.device().clone(),
                    runner,
                );
                assert_commit(&case.name, &case.expected, result);
            }
            "project" => {
                let owner = match case.evaluation_owner.as_deref() {
                    Some("foreign") => SnapshotOwner {
                        issuer: "issuer".to_owned(),
                        subject: "other-user".to_owned(),
                        tenant_id: "tenant".to_owned(),
                    },
                    Some("invalid") => SnapshotOwner {
                        issuer: String::new(),
                        subject: String::new(),
                        tenant_id: String::new(),
                    },
                    _ => state.device().owner().clone(),
                };
                let result = project_persisted_inventory(
                    &state,
                    &owner,
                    case.evaluated_at_ms.unwrap(),
                    fixture.stale_after_ms,
                );
                assert_projection(&case.name, &case.expected, result);
            }
            operation => panic!("{}: unknown operation {operation:?}", case.name),
        }
    }
}

fn build_state(base: &State, case: &Case) -> PersistedInventoryState {
    let approval = match case
        .device_approval_state
        .as_deref()
        .unwrap_or(&base.device.approval_state)
    {
        "approved" => DeviceApprovalState::Approved,
        "pending" => DeviceApprovalState::Pending,
        "revoked" => DeviceApprovalState::Revoked,
        value => panic!("unsupported approval state {value:?}"),
    };
    let cordoned = match case
        .device_cordon_state
        .as_deref()
        .unwrap_or(&base.device.cordon_state)
    {
        "clear" => false,
        "cordoned" => true,
        value => panic!("unsupported cordon state {value:?}"),
    };
    let liveness = match case
        .runner_liveness
        .as_deref()
        .unwrap_or(&base.runner.liveness)
    {
        "online" => RunnerLiveness::Online,
        "offline" => RunnerLiveness::Offline,
        value => panic!("unsupported liveness {value:?}"),
    };
    let device_id = DeviceId::parse(base.device.device_id.clone()).unwrap();
    let device = Device::restore(
        device_id,
        TenantId::parse(base.device.owner.tenant_id.clone()).unwrap(),
        approval,
        cordoned,
    );
    let persisted_device = PersistedInventoryDevice::restore(
        device,
        base.device.owner.snapshot(),
        base.device.reservation_state == "reserved",
    );
    let runner_device_id = case
        .runner_device_id
        .as_deref()
        .unwrap_or(&base.runner.device_id);
    let runner = RunnerInstance::restore(
        DeviceId::parse(runner_device_id).unwrap(),
        RunnerInstanceId::parse(base.runner.instance_id.clone()).unwrap(),
        base.runner.generation,
        base.runner.heartbeat_sequence,
        base.runner.server_observed_at_ms,
        base.runner.capability_lease_expires_at_ms,
        liveness,
        capabilities(&base.runner.capabilities),
    )
    .unwrap();
    restore_persisted_inventory(
        case.state_revision.unwrap_or(base.revision),
        persisted_device,
        runner,
    )
    .unwrap_or_else(|error| panic!("{}: build state rejected: {error}", case.name))
}

fn capabilities(value: &Capabilities) -> CapabilitySnapshot {
    CapabilitySnapshot::new(
        value.operating_system.clone(),
        value.architecture.clone(),
        value.cpu_cores,
        value.available_cpu_cores,
        value.memory_bytes,
        value.available_memory_bytes,
        value.storage_bytes,
        value.available_storage_bytes,
        value
            .gpus
            .iter()
            .map(|gpu| {
                GpuCapability::new(
                    gpu.id.clone(),
                    gpu.vendor.clone(),
                    gpu.memory_bytes,
                    gpu.available_memory_bytes,
                )
                .unwrap()
            })
            .collect(),
        value.runtimes.clone(),
    )
    .unwrap()
}

fn assert_error<T: std::fmt::Debug>(
    name: &str,
    expected: &Expected,
    result: Result<T, PersistenceError>,
) {
    if expected.accepted {
        assert!(result.is_ok(), "{name}: expected acceptance");
    } else {
        let error = result.expect_err(name);
        assert_eq!(
            error.to_string(),
            expected.error.as_deref().unwrap(),
            "{name}"
        );
    }
}

fn assert_commit(
    name: &str,
    expected: &Expected,
    result: Result<PersistedInventoryState, PersistenceError>,
) {
    if !expected.accepted {
        assert_error(name, expected, result.map(|_| ()));
        return;
    }
    let state = result.unwrap_or_else(|error| panic!("{name} rejected: {error}"));
    assert_eq!(state.revision(), expected.revision.unwrap(), "{name}");
    assert_eq!(
        state.runner().heartbeat_sequence(),
        expected.heartbeat_sequence.unwrap(),
        "{name}"
    );
    assert_eq!(
        state.runner().server_observed_at_ms(),
        expected.server_observed_at_ms.unwrap(),
        "{name}"
    );
    assert_eq!(
        state.runner().capability_lease_expires_at_ms(),
        expected.capability_lease_expires_at_ms.unwrap(),
        "{name}"
    );
}

fn assert_projection(
    name: &str,
    expected: &Expected,
    result: Result<PersistedInventoryProjection, PersistenceError>,
) {
    if !expected.accepted {
        assert_error(name, expected, result.map(|_| ()));
        return;
    }
    let projection = result.unwrap_or_else(|error| panic!("{name} rejected: {error}"));
    assert_eq!(projection.revision, expected.revision.unwrap(), "{name}");
    assert_eq!(
        projection.device_id.as_str(),
        expected.device_id.as_deref().unwrap(),
        "{name}"
    );
    assert_eq!(
        projection.instance_id.as_str(),
        expected.instance_id.as_deref().unwrap(),
        "{name}"
    );
    assert_eq!(
        projection.status.as_str(),
        expected.status.as_deref().unwrap(),
        "{name}"
    );
    assert_eq!(projection.fresh, expected.fresh.unwrap(), "{name}");
    assert_eq!(
        projection.declared_eligible,
        expected.declared_eligible.unwrap(),
        "{name}"
    );
}
