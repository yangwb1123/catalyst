use std::{
    collections::HashSet,
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use forge_runtime_domain::{
    CapabilitySnapshot, Device, DeviceApprovalState, DeviceId, PersistedRunnerInstance,
    PersistenceError, RunnerHeartbeat, RunnerInstance, RunnerInstanceId, RunnerLiveness, TenantId,
    commit_device_heartbeat,
};
use serde::{Deserialize, Serialize};

use crate::args::DeviceCommand;

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_CASES: usize = 128;
const MAX_CASE_NAME_BYTES: usize = 128;
const SCHEMA_VERSION: &str = "forge.device-heartbeat-persistence-contract/v1";
const EVALUATION_MODE: &str = "pure_compare_and_swap_plan";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    authority: Authority,
    device: DeviceFixture,
    cases: Vec<Case>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceFixture {
    device_id: String,
    tenant_id: String,
    approval_state: String,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HeartbeatFixture {
    device_id: String,
    instance_id: String,
    generation: u64,
    sequence: u64,
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
    generation: Option<u64>,
    #[serde(default)]
    heartbeat_sequence: Option<u64>,
    #[serde(default)]
    server_observed_at_ms: Option<u64>,
    #[serde(default)]
    capability_lease_expires_at_ms: Option<u64>,
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
pub(crate) struct HeartbeatPersistenceOutput {
    v: u16,
    #[serde(rename = "type")]
    output_type: &'static str,
    schema_version: &'static str,
    evaluation_mode: &'static str,
    device: DeviceOutput,
    cases: Vec<CaseOutput>,
    authority: Authority,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct DeviceOutput {
    device_id: String,
    tenant_id: String,
    approval_state: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct CaseOutput {
    name: String,
    accepted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    revision: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    heartbeat_sequence: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    server_observed_at_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    capability_lease_expires_at_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

pub(crate) fn execute(
    command: &DeviceCommand,
) -> Result<HeartbeatPersistenceOutput, Box<dyn Error>> {
    let DeviceCommand::HeartbeatPersistencePreview { input } = command else {
        return Err("device heartbeat persistence preview command is required".into());
    };
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes)
        .map_err(|error| format!("device heartbeat persistence input is invalid JSON: {error}"))?;
    let fixture: Fixture = serde_json::from_slice(&bytes)
        .map_err(|error| format!("device heartbeat persistence input is invalid JSON: {error}"))?;
    evaluate(fixture)
}

pub(crate) fn write_output(
    output: &HeartbeatPersistenceOutput,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    writeln!(
        writer,
        "offline heartbeat persistence preview [{}] device={} tenant={} approval={}",
        output.schema_version,
        output.device.device_id,
        output.device.tenant_id,
        output.device.approval_state
    )?;
    writeln!(
        writer,
        "evaluation: {} (pure compare-and-swap model; no storage write, clock read, network, retry, or authority)",
        output.evaluation_mode
    )?;
    for case in &output.cases {
        if let Some(error) = &case.error {
            writeln!(writer, "{}: accepted=false error={error}", case.name)?;
        } else {
            writeln!(
                writer,
                "{}: accepted=true revision={} generation={} sequence={} observed_at_ms={} lease_expires_at_ms={}",
                case.name,
                case.revision.unwrap_or_default(),
                case.generation.unwrap_or_default(),
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

fn evaluate(fixture: Fixture) -> Result<HeartbeatPersistenceOutput, Box<dyn Error>> {
    validate_fixture_shape(&fixture)?;
    let device_id = DeviceId::parse(fixture.device.device_id.clone())?;
    let tenant_id = TenantId::parse(fixture.device.tenant_id.clone())?;
    let base_approval = parse_approval(&fixture.device.approval_state)?;
    let capabilities = capabilities()?;
    let mut names = HashSet::with_capacity(fixture.cases.len());
    let mut cases = Vec::with_capacity(fixture.cases.len());
    for case in fixture.cases {
        validate_case_name(&case.name, &mut names)?;
        let approval = case
            .device_approval_state
            .as_deref()
            .map(parse_approval)
            .transpose()?;
        let device = Device::restore(
            device_id.clone(),
            tenant_id.clone(),
            approval.unwrap_or(base_approval),
            false,
        );
        cases.push(evaluate_case(case, &device, &capabilities)?);
    }
    Ok(HeartbeatPersistenceOutput {
        v: 1,
        output_type: "device_heartbeat_persistence_preview",
        schema_version: SCHEMA_VERSION,
        evaluation_mode: EVALUATION_MODE,
        device: DeviceOutput {
            device_id: device_id.to_string(),
            tenant_id: tenant_id.to_string(),
            approval_state: approval_label(base_approval).to_owned(),
        },
        cases,
        authority: Authority::default(),
    })
}

fn validate_fixture_shape(fixture: &Fixture) -> Result<(), Box<dyn Error>> {
    if fixture.schema_version != SCHEMA_VERSION
        || fixture.evaluation_mode != EVALUATION_MODE
        || fixture.cases.is_empty()
        || fixture.cases.len() > MAX_CASES
        || fixture.authority != Authority::default()
    {
        return Err(
            "device heartbeat persistence input is not a bounded pure compare-and-swap contract"
                .into(),
        );
    }
    Ok(())
}

fn validate_case_name(name: &str, names: &mut HashSet<String>) -> Result<(), Box<dyn Error>> {
    if name.trim().is_empty() || name.len() > MAX_CASE_NAME_BYTES {
        return Err("device heartbeat persistence case name is empty or too long".into());
    }
    if !names.insert(name.to_owned()) {
        return Err(format!("duplicate device heartbeat persistence case {name:?}").into());
    }
    Ok(())
}

fn evaluate_case(
    case: Case,
    device: &Device,
    capabilities: &CapabilitySnapshot,
) -> Result<CaseOutput, Box<dyn Error>> {
    let name = case.name;
    let expected = case.expected;
    let current = match case.current.as_ref() {
        Some(current) => match restore_current(current, capabilities) {
            Ok(current) => Some(current),
            Err(error) => return compare_error(name, expected, error.to_string()),
        },
        None => None,
    };
    let heartbeat = parse_heartbeat(case.heartbeat, capabilities)?;
    let result = commit_device_heartbeat(
        device,
        current.as_ref(),
        case.expected_revision,
        &heartbeat,
        case.server_observed_at_ms,
        case.lease_ttl_ms,
    );
    match result {
        Ok(state) => compare_success(name, expected, state),
        Err(error) => compare_error(name, expected, error.to_string()),
    }
}

fn compare_success(
    name: String,
    expected: Expected,
    state: PersistedRunnerInstance,
) -> Result<CaseOutput, Box<dyn Error>> {
    if !expected.accepted
        || expected.error.is_some()
        || expected.revision != Some(state.revision())
        || expected.generation != Some(state.instance().generation())
        || expected.heartbeat_sequence != Some(state.instance().heartbeat_sequence())
        || expected.server_observed_at_ms != Some(state.instance().server_observed_at_ms())
        || expected.capability_lease_expires_at_ms
            != Some(state.instance().capability_lease_expires_at_ms())
    {
        return Err(format!("heartbeat persistence case {name:?} expectation mismatch").into());
    }
    Ok(CaseOutput {
        name,
        accepted: true,
        revision: Some(state.revision()),
        generation: Some(state.instance().generation()),
        heartbeat_sequence: Some(state.instance().heartbeat_sequence()),
        server_observed_at_ms: Some(state.instance().server_observed_at_ms()),
        capability_lease_expires_at_ms: Some(state.instance().capability_lease_expires_at_ms()),
        error: None,
    })
}

fn compare_error(
    name: String,
    expected: Expected,
    actual_error: String,
) -> Result<CaseOutput, Box<dyn Error>> {
    if expected.accepted
        || expected.error.as_deref() != Some(actual_error.as_str())
        || expected.revision.is_some()
        || expected.generation.is_some()
        || expected.heartbeat_sequence.is_some()
        || expected.server_observed_at_ms.is_some()
        || expected.capability_lease_expires_at_ms.is_some()
    {
        return Err(format!("heartbeat persistence case {name:?} expectation mismatch").into());
    }
    Ok(CaseOutput {
        name,
        accepted: false,
        revision: None,
        generation: None,
        heartbeat_sequence: None,
        server_observed_at_ms: None,
        capability_lease_expires_at_ms: None,
        error: Some(actual_error),
    })
}

fn restore_current(
    current: &Current,
    capabilities: &CapabilitySnapshot,
) -> Result<PersistedRunnerInstance, PersistenceError> {
    if current.revision == 0 {
        return Err(PersistenceError::InvalidPersistedState);
    }
    let device_id = DeviceId::parse(current.device_id.clone())
        .map_err(|_| PersistenceError::InvalidPersistedState)?;
    let instance_id = RunnerInstanceId::parse(current.instance_id.clone())
        .map_err(|_| PersistenceError::InvalidPersistedState)?;
    let instance = RunnerInstance::restore(
        device_id,
        instance_id,
        current.generation,
        current.heartbeat_sequence,
        current.server_observed_at_ms,
        current.capability_lease_expires_at_ms,
        RunnerLiveness::Online,
        capabilities.clone(),
    )
    .map_err(|_| PersistenceError::InvalidPersistedState)?;
    PersistedRunnerInstance::restore(current.revision, instance)
}

fn parse_heartbeat(
    heartbeat: HeartbeatFixture,
    capabilities: &CapabilitySnapshot,
) -> Result<RunnerHeartbeat, Box<dyn Error>> {
    let device_id = DeviceId::parse(heartbeat.device_id)?;
    let instance_id = RunnerInstanceId::parse(heartbeat.instance_id)?;
    Ok(RunnerHeartbeat::new(
        device_id,
        instance_id,
        heartbeat.generation,
        heartbeat.sequence,
        capabilities.clone(),
    )?)
}

fn parse_approval(value: &str) -> Result<DeviceApprovalState, Box<dyn Error>> {
    match value {
        "pending" => Ok(DeviceApprovalState::Pending),
        "approved" => Ok(DeviceApprovalState::Approved),
        "revoked" => Ok(DeviceApprovalState::Revoked),
        _ => Err(format!("unsupported device approval state {value:?}").into()),
    }
}

fn approval_label(value: DeviceApprovalState) -> &'static str {
    match value {
        DeviceApprovalState::Pending => "pending",
        DeviceApprovalState::Approved => "approved",
        DeviceApprovalState::Revoked => "revoked",
    }
}

fn capabilities() -> Result<CapabilitySnapshot, Box<dyn Error>> {
    Ok(CapabilitySnapshot::new(
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
    )?)
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
            format!("device heartbeat persistence input exceeds {MAX_INPUT_BYTES} bytes").into(),
        );
    }
    Ok(bytes)
}
