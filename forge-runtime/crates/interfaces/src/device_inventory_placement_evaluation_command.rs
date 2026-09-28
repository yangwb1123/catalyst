use std::{
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
    build_persisted_inventory_placement_input, evaluate_persisted_inventory_placement_input,
    restore_persisted_inventory,
};
use serde::{Deserialize, Serialize};

use crate::args::{DeviceCommand, DeviceInventoryCommand};

#[path = "device_inventory_placement_evaluation_command/source.rs"]
mod source;
use source::{SourceAuthority, SourceCapabilities, SourceFixture};

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const SCHEMA_VERSION: &str = "forge.device-inventory-placement-evaluation/v1";
const EVALUATION_MODE: &str = "pure_persisted_inventory_offline_evaluation";
const SOURCE_FIXTURE: &str = "forge-device-inventory-placement-input-v1.json";
const SOURCE_SCHEMA_VERSION: &str = "forge.device-inventory-placement-input/v1";
const SOURCE_MODE: &str = "pure_persisted_inventory_to_placement_input";
const SOURCE_FIXTURE_TEXT: &str = include_str!(
    "../../../../docs/contracts/fixtures/forge-device-inventory-placement-input-v1.json"
);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct GpuRequirement {
    required: bool,
    min_memory_bytes: u64,
    runtime: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "Independent contract flags preserve the frozen observation and authority wire shape"
)]
struct Authority {
    placement_evaluated: bool,
    placement_selected: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "Independent contract flags preserve the frozen observation and authority wire shape"
)]
struct Expected {
    accepted: bool,
    error: String,
    revision: u64,
    device_id: String,
    instance_id: String,
    matches_requirements: bool,
    exclusion_reasons: Vec<String>,
    owner_declaration_unverified: bool,
    device_attributes_unverified: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_field_names,
    reason = "Preserve frozen wire field names and existing Serde type-name diagnostics"
)]
pub(crate) struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    source_fixture: String,
    source_case: String,
    evaluated_at_ms: u64,
    policy_requirements: Requirements,
    authority: Authority,
    expected: Expected,
}

pub(crate) fn execute(command: &DeviceCommand) -> Result<Fixture, Box<dyn Error>> {
    let DeviceCommand::Inventory(DeviceInventoryCommand::PlacementEvaluation { input }) = command
    else {
        return Err("device inventory placement-evaluation command is required".into());
    };
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes).map_err(|error| {
        format!("device inventory placement-evaluation input has duplicate JSON keys: {error}")
    })?;
    let fixture: Fixture = serde_json::from_slice(&bytes).map_err(|error| {
        format!("device inventory placement-evaluation input is invalid JSON: {error}")
    })?;
    let source_bytes = if input == "-" {
        SOURCE_FIXTURE_TEXT.as_bytes().to_vec()
    } else {
        fs::read(
            Path::new(input)
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(SOURCE_FIXTURE),
        )
        .map_err(|error| format!("placement evaluation source fixture is unavailable: {error}"))?
    };
    crate::device_json_unique::reject_duplicate_keys(&source_bytes)
        .map_err(|error| format!("placement evaluation source has duplicate JSON keys: {error}"))?;
    if source_bytes != SOURCE_FIXTURE_TEXT.as_bytes() {
        return Err(
            "placement evaluation source fixture drifted from the canonical fixture".into(),
        );
    }
    let source: SourceFixture = serde_json::from_slice(&source_bytes)
        .map_err(|error| format!("placement evaluation source fixture is invalid: {error}"))?;
    evaluate(fixture, &source)
}

pub(crate) fn run(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device inventory placement evaluation failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = write_output(&output, json, &mut io::stdout().lock()) {
        eprintln!("failed to write device inventory placement evaluation output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

pub(crate) fn write_output(
    output: &Fixture,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    writeln!(
        writer,
        "offline placement evaluation [{}] source={} device={} instance={} matches={} reasons={}",
        output.evaluation_mode,
        output.source_case,
        output.expected.device_id,
        output.expected.instance_id,
        output.expected.matches_requirements,
        output.expected.exclusion_reasons.join(",")
    )?;
    writeln!(
        writer,
        "authority: placement_evaluated=false placement_selected=false reservation_created=false execution_authorized=false dispatch_performed=false"
    )
}

fn evaluate(fixture: Fixture, source: &SourceFixture) -> Result<Fixture, Box<dyn Error>> {
    if fixture.schema_version != SCHEMA_VERSION
        || fixture.evaluation_mode != EVALUATION_MODE
        || fixture.source_fixture != SOURCE_FIXTURE
        || fixture.source_case != "online"
        || fixture.evaluated_at_ms == 0
        || fixture.evaluated_at_ms > MAX_SAFE_INTEGER
        || fixture.authority != Authority::default()
    {
        return Err("placement evaluation input is not a bounded pure read-only contract".into());
    }
    if source.schema_version != SOURCE_SCHEMA_VERSION
        || source.evaluation_mode != SOURCE_MODE
        || source.authority != SourceAuthority::default()
        || source.cases.len() != 12
        || source.policy_requirements.data_residency_zones
            != fixture.policy_requirements.data_residency_zones
        || source.policy_requirements.minimum_trust_zone
            != fixture.policy_requirements.minimum_trust_zone
        || source.policy_requirements.sandbox_floor != fixture.policy_requirements.sandbox_floor
        || source.policy_requirements.concurrency_slots
            != fixture.policy_requirements.concurrency_slots
    {
        return Err("placement evaluation source is not a bounded pure input contract".into());
    }
    let owner = source.evaluation_owner.snapshot();
    validate_owner(&source.evaluation_owner)?;
    TenantId::parse(owner.tenant_id.clone())?;
    let requirements = requirements(&fixture.policy_requirements)?;
    let input = build_online_input(source, &owner)?;
    let actual =
        evaluate_persisted_inventory_placement_input(&input, requirements, fixture.evaluated_at_ms)
            .map_err(|error| error.to_string())?;
    if !fixture.expected.accepted
        || !fixture.expected.error.is_empty()
        || actual.revision != fixture.expected.revision
        || actual.device_id != fixture.expected.device_id
        || actual.instance_id != fixture.expected.instance_id
        || actual.matches_requirements != fixture.expected.matches_requirements
        || actual.exclusion_reasons != fixture.expected.exclusion_reasons
        || actual.owner_declaration_unverified != fixture.expected.owner_declaration_unverified
        || actual.device_attributes_unverified != fixture.expected.device_attributes_unverified
    {
        return Err("placement evaluation result drifted from the canonical fixture".into());
    }
    Ok(fixture)
}

fn build_online_input(
    source: &SourceFixture,
    owner: &SnapshotOwner,
) -> Result<forge_runtime_domain::PersistedInventoryPlacementInput, Box<dyn Error>> {
    validate_online_source(source, owner)?;
    let device_id = DeviceId::parse(source.state.device.device_id.clone())?;
    let tenant_id = TenantId::parse(source.state.device.owner.tenant_id.clone())?;
    let device = Device::restore(
        device_id.clone(),
        tenant_id,
        DeviceApprovalState::Approved,
        false,
    );
    let persisted = PersistedInventoryDevice::restore(device, owner.clone(), false);
    let runner = RunnerInstance::restore(
        device_id,
        RunnerInstanceId::parse(source.state.runner.instance_id.clone())?,
        source.state.runner.generation,
        source.state.runner.heartbeat_sequence,
        source.state.runner.server_observed_at_ms,
        source.state.runner.capability_lease_expires_at_ms,
        RunnerLiveness::Online,
        capabilities(&source.state.runner.capabilities)?,
    )?;
    let state = restore_persisted_inventory(source.state.revision, persisted, runner)?;
    Ok(build_persisted_inventory_placement_input(&state, owner)?)
}

fn validate_online_source(
    source: &SourceFixture,
    owner: &SnapshotOwner,
) -> Result<(), Box<dyn Error>> {
    let source_case = source
        .cases
        .iter()
        .find(|candidate| candidate.name == "online")
        .ok_or("placement evaluation source is missing the online case")?;
    if source_case.evaluation_owner.is_some()
        || source_case.runner_device_id.is_some()
        || source_case.approval_state.is_some()
        || source_case.cordon_state.is_some()
        || source_case.liveness.is_some()
        || source_case.server_observed_at_ms.is_some()
        || source_case.capability_lease_expires_at_ms.is_some()
        || source.state.device.owner != source.evaluation_owner
        || source.state.device.device_id != source.state.runner.device_id
        || source.state.device.device_id.is_empty()
        || source.state.device.approval_state != "approved"
        || source.state.device.cordon_state != "clear"
        || source.state.device.reservation_state != "none"
        || source.state.runner.liveness != "online"
    {
        return Err("placement evaluation source online case drifted".into());
    }
    if source.state.device.owner.snapshot() != *owner
        || source.state.runner.server_observed_at_ms > MAX_SAFE_INTEGER
        || source.state.runner.capability_lease_expires_at_ms > MAX_SAFE_INTEGER
    {
        return Err("placement evaluation source owner or timestamp drifted".into());
    }
    Ok(())
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
            .map_err(|error| format!("invalid GPU: {error}"))
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

fn requirements(value: &Requirements) -> Result<DevicePlacementRequirements, Box<dyn Error>> {
    if value.gpu.required || value.gpu.min_memory_bytes != 0 || !value.gpu.runtime.is_empty() {
        return Err("placement evaluation GPU requirements are unsupported".into());
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

fn validate_owner(value: &Owner) -> Result<(), Box<dyn Error>> {
    for part in [&value.issuer, &value.subject, &value.tenant_id] {
        if part.is_empty() || part.trim() != part || part.chars().any(char::is_control) {
            return Err("placement evaluation owner declaration is invalid".into());
        }
    }
    Ok(())
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::with_capacity(MAX_INPUT_BYTES.min(64 * 1024));
    if input == "-" {
        io::stdin()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
    } else {
        fs::File::open(Path::new(input))?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "device inventory placement-evaluation input exceeds {MAX_INPUT_BYTES} bytes"
        )
        .into());
    }
    Ok(bytes)
}
