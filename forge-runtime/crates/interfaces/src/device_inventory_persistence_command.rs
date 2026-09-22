use std::{
    collections::HashSet,
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
    process::ExitCode,
};

use forge_runtime_domain::{
    CapabilitySnapshot, Device, DeviceApprovalState, DeviceId, GpuCapability,
    PersistedInventoryDevice, PersistedInventoryProjection, PersistedInventoryState,
    RunnerInstance, RunnerInstanceId, RunnerLiveness, SnapshotOwner, TenantId,
    commit_persisted_inventory, project_persisted_inventory, restore_persisted_inventory,
};
use serde::{Deserialize, Serialize};

use crate::args::{DeviceCommand, DeviceInventoryCommand};

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_CASES: usize = 128;
const MAX_CASE_NAME_BYTES: usize = 128;
const SCHEMA_VERSION: &str = "forge.device-inventory-persistence/v1";
const EVALUATION_MODE: &str = "pure_persisted_inventory_cas_projection";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    stale_after_ms: u64,
    authority: Authority,
    state: StateFixture,
    cases: Vec<Case>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StateFixture {
    revision: u64,
    device: DeviceFixture,
    runner: RunnerFixture,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceFixture {
    device_id: String,
    owner: Owner,
    approval_state: String,
    cordon_state: String,
    reservation_state: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunnerFixture {
    device_id: String,
    instance_id: String,
    generation: u64,
    heartbeat_sequence: u64,
    server_observed_at_ms: u64,
    capability_lease_expires_at_ms: u64,
    liveness: String,
    capabilities: CapabilitiesFixture,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GpuFixture {
    id: String,
    vendor: String,
    memory_bytes: u64,
    available_memory_bytes: u64,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Replacement {
    #[serde(default)]
    heartbeat_sequence: Option<u64>,
    #[serde(default)]
    server_observed_at_ms: Option<u64>,
    #[serde(default)]
    capability_lease_expires_at_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
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

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
struct Authority {
    identity_verified: bool,
    heartbeat_persisted: bool,
    inventory_authoritative: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct PersistenceOutput {
    v: u16,
    #[serde(rename = "type")]
    output_type: &'static str,
    schema_version: &'static str,
    evaluation_mode: &'static str,
    stale_after_ms: u64,
    state: StateOutput,
    cases: Vec<CaseOutput>,
    authority: Authority,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct StateOutput {
    revision: u64,
    device_id: String,
    instance_id: String,
    tenant_id: String,
    approval_state: String,
    cordon_state: String,
    reservation_state: String,
    liveness: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct CaseOutput {
    name: String,
    operation: String,
    accepted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    revision: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instance_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    heartbeat_sequence: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    server_observed_at_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    capability_lease_expires_at_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fresh: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    declared_eligible: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Clone, Debug)]
struct ParsedState {
    revision: u64,
    device: PersistedInventoryDevice,
    runner: RunnerInstance,
}

pub(crate) fn execute(command: &DeviceCommand) -> Result<PersistenceOutput, Box<dyn Error>> {
    let DeviceCommand::Inventory(DeviceInventoryCommand::PersistencePreview { input }) = command
    else {
        return Err("device inventory persistence preview command is required".into());
    };
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes).map_err(|error| {
        format!("device inventory persistence input has duplicate JSON keys: {error}")
    })?;
    let fixture: Fixture = serde_json::from_slice(&bytes)
        .map_err(|error| format!("device inventory persistence input is invalid JSON: {error}"))?;
    evaluate(fixture)
}

pub(crate) fn run(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device inventory persistence preview failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = write_output(&output, json, &mut io::stdout().lock()) {
        eprintln!("failed to write device inventory persistence preview output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

pub(crate) fn write_output(
    output: &PersistenceOutput,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    writeln!(
        writer,
        "offline device inventory persistence preview [{}] revision={} device={} instance={}",
        output.schema_version,
        output.state.revision,
        output.state.device_id,
        output.state.instance_id
    )?;
    writeln!(
        writer,
        "evaluation: {} (pure CAS/projection model; no storage write, clock read, network, or authority)",
        output.evaluation_mode
    )?;
    for case in &output.cases {
        if let Some(error) = &case.error {
            writeln!(writer, "{}: accepted=false error={error}", case.name)?;
        } else if case.operation == "project" {
            writeln!(
                writer,
                "{}: accepted=true revision={} status={} fresh={} declared_eligible={}",
                case.name,
                case.revision.unwrap_or_default(),
                case.status.as_deref().unwrap_or("unknown"),
                case.fresh.unwrap_or(false),
                case.declared_eligible.unwrap_or(false)
            )?;
        } else {
            writeln!(
                writer,
                "{}: accepted=true revision={} sequence={} observed_at_ms={} lease_expires_at_ms={}",
                case.name,
                case.revision.unwrap_or_default(),
                case.heartbeat_sequence.unwrap_or_default(),
                case.server_observed_at_ms.unwrap_or_default(),
                case.capability_lease_expires_at_ms.unwrap_or_default()
            )?;
        }
    }
    writeln!(
        writer,
        "authority: identity_verified=false heartbeat_persisted=false inventory_authoritative=false reservation_created=false execution_authorized=false dispatch_performed=false"
    )
}

#[allow(clippy::too_many_lines)]
fn evaluate(fixture: Fixture) -> Result<PersistenceOutput, Box<dyn Error>> {
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

fn parse_state(value: &StateFixture) -> Result<ParsedState, Box<dyn Error>> {
    let device_id = DeviceId::parse(value.device.device_id.clone())?;
    let tenant_id = TenantId::parse(value.device.owner.tenant_id.clone())?;
    let device = Device::restore(
        device_id.clone(),
        tenant_id,
        parse_approval(&value.device.approval_state)?,
        parse_cordon(&value.device.cordon_state)?,
    );
    let persisted_device = PersistedInventoryDevice::restore(
        device,
        value.device.owner.snapshot(),
        parse_reservation(&value.device.reservation_state)?,
    );
    let runner = parse_runner(&value.runner)?;
    Ok(ParsedState {
        revision: value.revision,
        device: persisted_device,
        runner,
    })
}

fn parse_runner(value: &RunnerFixture) -> Result<RunnerInstance, Box<dyn Error>> {
    let device_id = DeviceId::parse(value.device_id.clone())?;
    let instance_id = RunnerInstanceId::parse(value.instance_id.clone())?;
    let capabilities = parse_capabilities(&value.capabilities)?;
    RunnerInstance::restore(
        device_id,
        instance_id,
        value.generation,
        value.heartbeat_sequence,
        value.server_observed_at_ms,
        value.capability_lease_expires_at_ms,
        parse_liveness(&value.liveness)?,
        capabilities,
    )
    .map_err(|error| format!("invalid persisted Runner instance: {error}").into())
}

fn parse_capabilities(value: &CapabilitiesFixture) -> Result<CapabilitySnapshot, Box<dyn Error>> {
    let gpus = value
        .gpus
        .iter()
        .map(|gpu| {
            GpuCapability::new(
                gpu.id.clone(),
                gpu.vendor.clone(),
                gpu.memory_bytes,
                gpu.available_memory_bytes,
            )
            .map_err(|error| -> Box<dyn Error> { error.into() })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CapabilitySnapshot::new(
        value.os.clone(),
        value.architecture.clone(),
        value.cpu_cores,
        value.available_cpu_cores,
        value.memory_bytes,
        value.available_memory_bytes,
        value.storage_bytes,
        value.available_storage_bytes,
        gpus,
        value.runtimes.clone(),
    )?)
}

#[allow(clippy::too_many_lines)]
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
        "commit" => {
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

fn case_state(base: &ParsedState, case: &Case) -> Result<ParsedState, Box<dyn Error>> {
    let approval = case
        .device_approval_state
        .as_deref()
        .map(parse_approval)
        .transpose()?
        .unwrap_or(base.device.device().approval());
    let cordoned = case
        .device_cordon_state
        .as_deref()
        .map(parse_cordon)
        .transpose()?
        .unwrap_or_else(|| base.device.device().is_cordoned());
    let reservation = base.device.reserved();
    let device = Device::restore(
        base.device.device().id().clone(),
        base.device.device().tenant_id().clone(),
        approval,
        cordoned,
    );
    let persisted_device =
        PersistedInventoryDevice::restore(device, base.device.owner().clone(), reservation);
    let runner_device_id = case
        .runner_device_id
        .as_deref()
        .unwrap_or(base.runner.device_id().as_str());
    let runner = RunnerInstance::restore(
        DeviceId::parse(runner_device_id.to_owned())?,
        base.runner.instance_id().clone(),
        base.runner.generation(),
        base.runner.heartbeat_sequence(),
        base.runner.server_observed_at_ms(),
        base.runner.capability_lease_expires_at_ms(),
        case.runner_liveness
            .as_deref()
            .map(parse_liveness)
            .transpose()?
            .unwrap_or(base.runner.liveness()),
        base.runner.capabilities().clone(),
    )
    .map_err(|error| format!("invalid persisted Runner instance: {error}"))?;
    Ok(ParsedState {
        revision: case.state_revision.unwrap_or(base.revision),
        device: persisted_device,
        runner,
    })
}

fn replacement_runner(
    case: &Case,
    base: &RunnerInstance,
) -> Result<RunnerInstance, Box<dyn Error>> {
    RunnerInstance::restore(
        base.device_id().clone(),
        base.instance_id().clone(),
        base.generation(),
        case.replacement
            .heartbeat_sequence
            .unwrap_or(base.heartbeat_sequence()),
        case.replacement
            .server_observed_at_ms
            .unwrap_or(base.server_observed_at_ms()),
        case.replacement
            .capability_lease_expires_at_ms
            .unwrap_or(base.capability_lease_expires_at_ms()),
        base.liveness(),
        base.capabilities().clone(),
    )
    .map_err(|error| format!("invalid replacement Runner instance: {error}").into())
}

fn evaluation_owner(
    device: &PersistedInventoryDevice,
    declaration: Option<&str>,
) -> Result<SnapshotOwner, Box<dyn Error>> {
    let mut owner = device.owner().clone();
    match declaration.unwrap_or("same") {
        "same" => {}
        "foreign" => "other-user".clone_into(&mut owner.subject),
        "invalid" => {
            owner = SnapshotOwner {
                issuer: String::new(),
                subject: String::new(),
                tenant_id: String::new(),
            }
        }
        other => return Err(format!("unsupported evaluation owner {other:?}").into()),
    }
    Ok(owner)
}

fn parse_approval(value: &str) -> Result<DeviceApprovalState, Box<dyn Error>> {
    match value {
        "pending" => Ok(DeviceApprovalState::Pending),
        "approved" => Ok(DeviceApprovalState::Approved),
        "revoked" => Ok(DeviceApprovalState::Revoked),
        _ => Err(format!("unsupported device approval state {value:?}").into()),
    }
}

fn parse_cordon(value: &str) -> Result<bool, Box<dyn Error>> {
    match value {
        "clear" => Ok(false),
        "cordoned" => Ok(true),
        _ => Err(format!("unsupported device cordon state {value:?}").into()),
    }
}

fn parse_reservation(value: &str) -> Result<bool, Box<dyn Error>> {
    match value {
        "none" => Ok(false),
        "reserved" => Ok(true),
        _ => Err(format!("unsupported device reservation state {value:?}").into()),
    }
}

fn parse_liveness(value: &str) -> Result<RunnerLiveness, Box<dyn Error>> {
    match value {
        "online" => Ok(RunnerLiveness::Online),
        "offline" => Ok(RunnerLiveness::Offline),
        _ => Err(format!("unsupported Runner liveness {value:?}").into()),
    }
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::with_capacity(MAX_INPUT_BYTES.min(64 * 1024));
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
        return Err(
            format!("device inventory persistence input exceeds {MAX_INPUT_BYTES} bytes").into(),
        );
    }
    Ok(bytes)
}
