use std::{
    collections::HashSet,
    error::Error,
    fs,
    io::{self, Read, Write},
    path::Path,
    process::ExitCode,
};

use forge_runtime_domain::{
    CapabilitySnapshot, Device, DeviceApprovalState, DeviceId, DevicePlacementPolicy,
    DevicePlacementRequirements, GpuCapability, PersistedInventoryDevice, RunnerInstance,
    RunnerInstanceId, RunnerLiveness, SnapshotOwner, TenantId,
    build_persisted_inventory_placement_input, evaluate_persisted_inventory_placement,
    restore_persisted_inventory,
};
use serde::{Deserialize, Serialize};

use crate::args::{DeviceCommand, DeviceInventoryCommand};

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const BATCH_SCHEMA: &str = "forge.device-inventory-placement-batch-evaluation/v1";
const SOURCE_FIXTURE: &str = "forge-device-inventory-placement-input-v1.json";
const BATCH_MODE: &str = "pure_persisted_inventory_placement_dry_run";
const SOURCE_FIXTURE_TEXT: &str = include_str!(
    "../../../../docs/contracts/fixtures/forge-device-inventory-placement-input-v1.json"
);

#[derive(Debug, Deserialize)]
#[allow(clippy::struct_field_names)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    source_fixture: String,
    evaluation_owner: Owner,
    evaluated_at_ms: u64,
    requirements: Requirements,
    cases: Vec<Case>,
    empty_inputs_allowed: bool,
    selected_device_id: Option<String>,
    selected_instance_id: Option<String>,
    authority: Authority,
    error_cases: Vec<ErrorCase>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Owner {
    issuer: String,
    subject: String,
    tenant_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Requirements {
    os: String,
    architecture: String,
    min_cpu_cores: u32,
    min_memory_bytes: u64,
    min_storage_bytes: u64,
    runtime: String,
    gpu: GpuRequirement,
    data_residency_zones: Vec<String>,
    minimum_trust_zone: String,
    sandbox_floor: String,
    concurrency_slots: u16,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GpuRequirement {
    required: bool,
    min_memory_bytes: u64,
    runtime: String,
}

#[derive(Debug, Deserialize)]
#[allow(clippy::struct_field_names)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    source_case: String,
    device_id: String,
    instance_id: String,
    #[serde(default)]
    snapshot_observed_at_ms: Option<u64>,
    #[serde(default)]
    capability_lease_expires_at_ms: Option<u64>,
    expected: ExpectedDecision,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedDecision {
    revision: u64,
    device_id: String,
    instance_id: String,
    matches_requirements: bool,
    exclusion_reasons: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
struct Authority {
    identity_verified: bool,
    heartbeat_persisted: bool,
    inventory_authoritative: bool,
    placement_selected: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ErrorCase {
    name: String,
    error: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceFixture {
    schema_version: String,
    evaluation_mode: String,
    evaluation_owner: Owner,
    policy_requirements: SourcePolicy,
    authority: Authority,
    state: SourceState,
    cases: Vec<SourceCase>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourcePolicy {
    data_residency_zones: Vec<String>,
    minimum_trust_zone: String,
    sandbox_floor: String,
    concurrency_slots: u16,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceState {
    revision: u64,
    device: SourceDevice,
    runner: SourceRunner,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceDevice {
    device_id: String,
    owner: Owner,
    approval_state: String,
    cordon_state: String,
    reservation_state: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceRunner {
    device_id: String,
    instance_id: String,
    generation: u64,
    heartbeat_sequence: u64,
    server_observed_at_ms: u64,
    capability_lease_expires_at_ms: u64,
    liveness: String,
    capabilities: SourceCapabilities,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceCapabilities {
    os: String,
    architecture: String,
    cpu_cores: u32,
    available_cpu_cores: u32,
    memory_bytes: u64,
    available_memory_bytes: u64,
    storage_bytes: u64,
    available_storage_bytes: u64,
    gpus: Vec<SourceGpu>,
    runtimes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceGpu {
    id: String,
    vendor: String,
    memory_bytes: u64,
    available_memory_bytes: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceCase {
    name: String,
    #[serde(default)]
    evaluation_owner: Option<Owner>,
    #[serde(default)]
    runner_device_id: Option<String>,
    #[serde(default)]
    approval_state: Option<String>,
    #[serde(default)]
    cordon_state: Option<String>,
    #[serde(default)]
    liveness: Option<String>,
    #[serde(default)]
    server_observed_at_ms: Option<u64>,
    #[serde(default)]
    capability_lease_expires_at_ms: Option<u64>,
    #[allow(dead_code)]
    expected: SourceExpected,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceExpected {
    accepted: bool,
    error: String,
    #[serde(default)]
    revision: Option<u64>,
    #[serde(default)]
    device_id: Option<String>,
    #[serde(default)]
    instance_id: Option<String>,
    #[serde(default)]
    generation: Option<u64>,
    #[serde(default)]
    heartbeat_sequence: Option<u64>,
    #[serde(default)]
    approval_state: Option<String>,
    #[serde(default)]
    cordon_state: Option<String>,
    #[serde(default)]
    reservation_state: Option<String>,
    #[serde(default)]
    liveness: Option<String>,
    #[serde(default)]
    snapshot_observed_at_ms: Option<u64>,
    #[serde(default)]
    lease_expires_at_ms: Option<u64>,
    #[serde(default)]
    owner_declaration_unverified: Option<bool>,
    #[serde(default)]
    policy_attributes_unverified: Option<bool>,
    #[serde(default)]
    data_residency_zones: Option<Vec<String>>,
    #[serde(default)]
    trust_zone: Option<String>,
    #[serde(default)]
    sandbox_levels: Option<Vec<String>>,
    #[serde(default)]
    concurrency_limit: Option<u16>,
    #[serde(default)]
    active_concurrency: Option<u16>,
    #[serde(default)]
    policy_requirements_met: Option<bool>,
}

#[derive(Debug, Serialize)]
pub(crate) struct Output {
    schema_version: &'static str,
    evaluation_mode: &'static str,
    owner_declaration: Owner,
    evaluated_at_ms: u64,
    owner_declaration_unverified: bool,
    device_attributes_unverified: bool,
    decisions: Vec<DecisionOutput>,
    selected_device_id: Option<String>,
    selected_instance_id: Option<String>,
    authority: Authority,
}

#[derive(Debug, Serialize)]
struct DecisionOutput {
    revision: u64,
    device_id: String,
    instance_id: String,
    matches_requirements: bool,
    exclusion_reasons: Vec<String>,
}

pub(crate) fn execute(command: &DeviceCommand) -> Result<Output, Box<dyn Error>> {
    let DeviceCommand::Inventory(DeviceInventoryCommand::PlacementBatchEvaluation { input }) =
        command
    else {
        return Err("device inventory placement-batch-evaluation command is required".into());
    };
    let (fixture_bytes, source_bytes) = if input == "-" {
        (
            read_bounded_input(input)?,
            SOURCE_FIXTURE_TEXT.as_bytes().to_vec(),
        )
    } else {
        let bytes = read_bounded_input(input)?;
        let fixture: Fixture = serde_json::from_slice(&bytes)
            .map_err(|e| format!("placement batch input is invalid JSON: {e}"))?;
        if fixture.source_fixture != SOURCE_FIXTURE {
            return Err("placement batch source_fixture is not the canonical fixture".into());
        }
        let source_path = Path::new(input)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(SOURCE_FIXTURE);
        (
            bytes,
            fs::read(source_path)
                .map_err(|e| format!("placement batch source fixture is unavailable: {e}"))?,
        )
    };
    let fixture: Fixture = serde_json::from_slice(&fixture_bytes)
        .map_err(|e| format!("placement batch input is invalid JSON: {e}"))?;
    crate::device_json_unique::reject_duplicate_keys(&fixture_bytes)
        .map_err(|e| format!("placement batch input has duplicate JSON keys: {e}"))?;
    crate::device_json_unique::reject_duplicate_keys(&source_bytes)
        .map_err(|e| format!("placement batch source has duplicate JSON keys: {e}"))?;
    if source_bytes != SOURCE_FIXTURE_TEXT.as_bytes() {
        return Err("placement batch source fixture drifted from the canonical fixture".into());
    }
    let source: SourceFixture = serde_json::from_slice(&source_bytes)
        .map_err(|e| format!("placement batch source fixture is invalid: {e}"))?;
    evaluate(fixture, &source)
}

pub(crate) fn run(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device inventory placement batch evaluation failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = write_output(&output, json, &mut io::stdout().lock()) {
        eprintln!("failed to write device inventory placement batch evaluation output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

pub(crate) fn write_output(output: &Output, json: bool, writer: &mut impl Write) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    writeln!(
        writer,
        "offline placement batch evaluation [{}] decisions={} selected=none",
        output.evaluation_mode,
        output.decisions.len()
    )?;
    for decision in &output.decisions {
        writeln!(
            writer,
            "{} / {}: matches={} reasons={}",
            decision.device_id,
            decision.instance_id,
            decision.matches_requirements,
            decision.exclusion_reasons.join(",")
        )?;
    }
    writeln!(
        writer,
        "authority: identity_verified=false heartbeat_persisted=false inventory_authoritative=false placement_selected=false reservation_created=false execution_authorized=false dispatch_performed=false"
    )
}

#[allow(clippy::too_many_lines)]
fn evaluate(fixture: Fixture, source: &SourceFixture) -> Result<Output, Box<dyn Error>> {
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
    if actual.schema_version
        != forge_runtime_domain::PERSISTED_INVENTORY_PLACEMENT_EVALUATION_SCHEMA_VERSION
        || actual.evaluation_mode
            != forge_runtime_domain::PERSISTED_INVENTORY_PLACEMENT_EVALUATION_MODE
        || actual.owner != owner
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

fn owner(value: &Owner) -> Result<SnapshotOwner, Box<dyn Error>> {
    if value.issuer.is_empty() || value.subject.is_empty() || value.tenant_id.is_empty() {
        return Err("placement batch owner is empty".into());
    }
    TenantId::parse(value.tenant_id.clone())
        .map_err(|e| -> Box<dyn Error> { format!("invalid placement batch tenant: {e}").into() })?;
    Ok(SnapshotOwner {
        issuer: value.issuer.clone(),
        subject: value.subject.clone(),
        tenant_id: value.tenant_id.clone(),
    })
}

fn requirements(value: &Requirements) -> Result<DevicePlacementRequirements, Box<dyn Error>> {
    if !value.gpu.runtime.is_empty() || value.gpu.required || value.gpu.min_memory_bytes != 0 {
        return Err("placement batch GPU requirements are unsupported".into());
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
        0,
        0,
    )?
    .with_policy(policy))
}

#[allow(clippy::too_many_lines)]
fn build_input(
    source: &SourceFixture,
    case: &Case,
    owner_value: &SnapshotOwner,
) -> Result<forge_runtime_domain::PersistedInventoryPlacementInput, Box<dyn Error>> {
    let source_case = source
        .cases
        .iter()
        .find(|candidate| candidate.name == case.source_case)
        .ok_or_else(|| format!("unknown source case {:?}", case.source_case))?;
    let source_owner = source_case
        .evaluation_owner
        .as_ref()
        .unwrap_or(&source.evaluation_owner);
    if owner(source_owner)? != *owner_value {
        return Err("placement batch source owner mismatch".into());
    }
    if source_case
        .runner_device_id
        .as_deref()
        .is_some_and(|runner_device_id| runner_device_id != case.device_id)
    {
        return Err("runner_device_mismatch".into());
    }
    let approval = match source_case
        .approval_state
        .as_deref()
        .unwrap_or(&source.state.device.approval_state)
    {
        "approved" => DeviceApprovalState::Approved,
        "pending" => DeviceApprovalState::Pending,
        "revoked" => DeviceApprovalState::Revoked,
        other => return Err(format!("invalid approval state {other}").into()),
    };
    let device_id =
        DeviceId::parse(case.device_id.clone()).map_err(|e| format!("invalid device id: {e}"))?;
    let tenant_id = TenantId::parse(source.state.device.owner.tenant_id.clone())
        .map_err(|e| format!("invalid tenant id: {e}"))?;
    let device = Device::restore(
        device_id.clone(),
        tenant_id,
        approval,
        source_case
            .cordon_state
            .as_deref()
            .unwrap_or(&source.state.device.cordon_state)
            == "cordoned",
    );
    let persisted = PersistedInventoryDevice::restore(
        device,
        owner_value.clone(),
        source.state.device.reservation_state == "reserved",
    );
    let observed = case
        .snapshot_observed_at_ms
        .or(source_case.server_observed_at_ms)
        .unwrap_or(source.state.runner.server_observed_at_ms);
    let lease = case
        .capability_lease_expires_at_ms
        .or(source_case.capability_lease_expires_at_ms)
        .unwrap_or(source.state.runner.capability_lease_expires_at_ms);
    if observed > MAX_SAFE_INTEGER || lease > MAX_SAFE_INTEGER {
        return Err("invalid_persisted_inventory_placement_input".into());
    }
    let runner = RunnerInstance::restore(
        device_id,
        RunnerInstanceId::parse(case.instance_id.clone())
            .map_err(|e| format!("invalid instance id: {e}"))?,
        source.state.runner.generation,
        source.state.runner.heartbeat_sequence,
        observed,
        lease,
        match source_case
            .liveness
            .as_deref()
            .unwrap_or(&source.state.runner.liveness)
        {
            "online" => RunnerLiveness::Online,
            "offline" => RunnerLiveness::Offline,
            other => return Err(format!("invalid liveness {other}").into()),
        },
        capabilities(&source.state.runner.capabilities)?,
    )
    .map_err(|e| format!("invalid runner: {e}"))?;
    let state = restore_persisted_inventory(source.state.revision, persisted, runner)
        .map_err(|e| e.to_string())?;
    build_persisted_inventory_placement_input(&state, owner_value).map_err(|e| e.to_string().into())
}

fn capabilities(value: &SourceCapabilities) -> Result<CapabilitySnapshot, Box<dyn Error>> {
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
            .map_err(|e| format!("invalid GPU: {e}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    CapabilitySnapshot::new(
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
    )
    .map_err(|e| format!("invalid capabilities: {e}").into())
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::new();
    if input == "-" {
        io::stdin()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
    } else {
        fs::File::open(input)?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err("placement batch input exceeds 2 MiB".into());
    }
    Ok(bytes)
}
