use std::{
    collections::HashSet,
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
    process::ExitCode,
};

use forge_runtime_domain::{
    DeviceId, DevicePlacementPolicy, DevicePlacementRequirements, MAX_DEVICE_CPU_CORES,
    MAX_DEVICE_GPU_COUNT, MAX_DEVICE_RUNTIME_NAME_BYTES, MAX_PERSISTED_INVENTORY_OBSERVATIONS,
    MAX_SNAPSHOT_OWNER_BYTES, PersistedInventoryObservationOwner, PersistedInventoryObservationV2,
    PersistedInventoryPlacementBatchAuthority, RunnerInstanceId, SnapshotOwner, TenantId,
    evaluate_persisted_inventory_observation_v2,
};
use serde::{Deserialize, Serialize};

use crate::args::{DeviceCommand, DeviceInventoryCommand};

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_DECISIONS: usize = 128;
// The v2 observation wire contract is narrower than the legacy capability
// model. Keep the CLI's preflight bound aligned with Go, Flutter, and the
// domain evaluator before attempting the pure comparison.
const MAX_V2_RUNTIME_COUNT: usize = 32;
const SCHEMA_VERSION: &str = "forge.device-inventory-placement-evaluation/v2";
const EVALUATION_MODE: &str = "offline_static_only";
const SOURCE_SCHEMA_VERSION: &str = "forge.device-inventory-observation/v2";
const NOTICE: &str = "Every owner, state, timestamp, resource, GPU, reservation, residency, trust, sandbox, and concurrency value is an unverified caller declaration. This read-only comparison selects no target and grants no execution authority.";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    source_schema_version: String,
    evaluation_owner: Owner,
    evaluated_at_ms: u64,
    notice: String,
    requirements: Requirements,
    observation: PersistedInventoryObservationV2,
    expected: Vec<ExpectedDecision>,
    eligible_candidate_count: usize,
    selected_device_id: Option<String>,
    selected_instance_id: Option<String>,
    authority: Authority,
}

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
#[serde(deny_unknown_fields)]
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

#[derive(Debug, Serialize)]
pub(crate) struct Output {
    schema_version: &'static str,
    evaluation_mode: &'static str,
    source_schema_version: &'static str,
    evaluation_owner: Owner,
    evaluated_at_ms: u64,
    notice: &'static str,
    decisions: Vec<DecisionOutput>,
    eligible_candidate_count: usize,
    selected_device_id: Option<String>,
    selected_instance_id: Option<String>,
    authority: Authority,
}

#[derive(Debug, Serialize)]
struct DecisionOutput {
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
    owner_declaration_unverified: bool,
    device_attributes_unverified: bool,
}

pub(crate) fn execute(command: &DeviceCommand) -> Result<Output, Box<dyn Error>> {
    let DeviceCommand::Inventory(DeviceInventoryCommand::PlacementEvaluationV2 { input }) = command
    else {
        return Err("device inventory placement-evaluation-v2 command is required".into());
    };
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes).map_err(|error| {
        format!("device inventory placement-evaluation-v2 input has duplicate JSON keys: {error}")
    })?;
    let fixture: Fixture = serde_json::from_slice(&bytes).map_err(|error| {
        format!("device inventory placement-evaluation-v2 input is invalid JSON: {error}")
    })?;
    evaluate(fixture)
}

pub(crate) fn run(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device inventory placement-evaluation-v2 failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = write_output(&output, json, &mut io::stdout().lock()) {
        eprintln!("failed to write device inventory placement-evaluation-v2 output: {error}");
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
        "offline placement evaluation [{}] decisions={} eligible={} selected=none",
        output.evaluation_mode,
        output.decisions.len(),
        output.eligible_candidate_count
    )?;
    for decision in &output.decisions {
        writeln!(
            writer,
            "{} / {}: revision={} generation={} heartbeat={} reservation={} gpus={} available_gpu_memory={} matches={} reasons={}",
            decision.device_id,
            decision.instance_id,
            decision.revision,
            decision.generation,
            decision.heartbeat_sequence,
            decision.reservation_state,
            decision.gpu_count,
            decision.available_gpu_memory_bytes,
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
fn evaluate(fixture: Fixture) -> Result<Output, Box<dyn Error>> {
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

#[allow(clippy::too_many_lines)]
fn validate_observation(
    observation: &PersistedInventoryObservationV2,
    owner: &SnapshotOwner,
) -> Result<(), Box<dyn Error>> {
    if observation.schema_version != SOURCE_SCHEMA_VERSION
        || observation.evaluation_mode != EVALUATION_MODE
        || observation.evaluated_at_ms == 0
        || observation.evaluated_at_ms > MAX_SAFE_INTEGER
        || observation.owner_declaration != PersistedInventoryObservationOwner::from(owner)
        || !observation.owner_declaration_unverified
        || !observation.inventory_declarations_unverified
        || observation.notice
            != "Every owner, instance, state, timestamp, resource, GPU, reservation, residency, trust, sandbox, and concurrency value is an unverified caller declaration. This read-only observation selects no target and grants no execution authority."
        || observation.execution_authorized
        || observation.reservation_created
        || observation.dispatch_performed
        || observation.devices.len() > MAX_PERSISTED_INVENTORY_OBSERVATIONS
    {
        return Err(
            "v2 placement-evaluation observation is not a bounded read-only contract".into(),
        );
    }
    let expected_owner = PersistedInventoryObservationOwner::from(owner);
    let mut devices = HashSet::with_capacity(observation.devices.len());
    let mut instances = HashSet::with_capacity(observation.devices.len());
    let mut previous = None;
    for candidate in &observation.devices {
        let key = (
            candidate.device.device_id.as_str(),
            candidate.instance_id.as_str(),
        );
        if previous.is_some_and(|value| value > key) {
            return Err("v2 placement-evaluation observation is not sorted".into());
        }
        previous = Some(key);
        if !devices.insert(candidate.device.device_id.clone())
            || !instances.insert(candidate.instance_id.clone())
        {
            return Err("v2 placement-evaluation observation has duplicate identity".into());
        }
        if candidate.revision == 0
            || candidate.generation == 0
            || candidate.heartbeat_sequence == 0
            || candidate.revision > MAX_SAFE_INTEGER
            || candidate.generation > MAX_SAFE_INTEGER
            || candidate.heartbeat_sequence > MAX_SAFE_INTEGER
        {
            return Err("v2 placement-evaluation observation has unsafe counters".into());
        }
        let device = &candidate.device;
        if device.owner != expected_owner
            || DeviceId::parse(device.device_id.clone()).is_err()
            || RunnerInstanceId::parse(candidate.instance_id.clone()).is_err()
            || TenantId::parse(device.owner.tenant_id.clone()).is_err()
            || device.snapshot_observed_at_ms > MAX_SAFE_INTEGER
            || device.lease_expires_at_ms < device.snapshot_observed_at_ms
            || device.lease_expires_at_ms > MAX_SAFE_INTEGER
            || device.available_cpu_cores > MAX_DEVICE_CPU_CORES
            || device.available_memory_bytes > MAX_SAFE_INTEGER
            || device.available_storage_bytes > MAX_SAFE_INTEGER
            || device.runtimes.len() > MAX_V2_RUNTIME_COUNT
            || device.gpus.len() > MAX_DEVICE_GPU_COUNT
            || device.concurrency_limit != 0
            || device.active_concurrency != 0
            || !device.data_residency_zones.is_empty()
            || device.trust_zone != "unknown"
            || !device.sandbox_levels.is_empty()
            || !matches!(
                device.approval_state.as_str(),
                "pending" | "approved" | "revoked"
            )
            || !matches!(device.cordon_state.as_str(), "clear" | "cordoned")
            || !matches!(device.reservation_state.as_str(), "none" | "reserved")
            || !matches!(device.liveness.as_str(), "online" | "offline")
        {
            return Err(format!(
                "v2 placement-evaluation device {} declaration is invalid",
                device.device_id
            )
            .into());
        }
        validate_canonical_tag(&device.os)?;
        validate_canonical_tag(&device.architecture)?;
        if device.os != device.os.to_ascii_lowercase()
            || device.architecture != device.architecture.to_ascii_lowercase()
        {
            return Err("v2 placement-evaluation device tags are not normalized".into());
        }
        validate_runtimes(&device.runtimes)?;
        validate_gpus(&device.gpus)?;
    }
    Ok(())
}

fn validate_owner(value: &Owner) -> Result<(), Box<dyn Error>> {
    for part in [&value.issuer, &value.subject, &value.tenant_id] {
        if part.is_empty()
            || part.len() > MAX_SNAPSHOT_OWNER_BYTES
            || part.trim() != part
            || part.chars().any(char::is_control)
        {
            return Err("v2 placement-evaluation owner declaration is invalid".into());
        }
    }
    Ok(())
}

fn validate_runtimes(runtimes: &[String]) -> Result<(), Box<dyn Error>> {
    let mut previous = None;
    for runtime in runtimes {
        if previous.is_some_and(|value: &str| value >= runtime.as_str()) {
            return Err("v2 placement-evaluation runtimes are not sorted and unique".into());
        }
        previous = Some(runtime.as_str());
        validate_canonical_tag(runtime)?;
        if runtime != &runtime.to_ascii_lowercase() {
            return Err("v2 placement-evaluation runtime is not normalized".into());
        }
    }
    Ok(())
}

fn validate_gpus(
    gpus: &[forge_runtime_domain::PersistedInventoryObservationV2Gpu],
) -> Result<(), Box<dyn Error>> {
    let mut previous = None;
    let mut ids = HashSet::with_capacity(gpus.len());
    for gpu in gpus {
        if previous.is_some_and(|value: &str| value >= gpu.id.as_str())
            || !ids.insert(gpu.id.clone())
            || DeviceId::parse(gpu.id.clone()).is_err()
            || gpu.memory_bytes == 0
            || gpu.memory_bytes > MAX_SAFE_INTEGER
            || gpu.available_memory_bytes > gpu.memory_bytes
            || gpu.available_memory_bytes > MAX_SAFE_INTEGER
        {
            return Err("v2 placement-evaluation GPU declaration is invalid".into());
        }
        previous = Some(gpu.id.as_str());
        validate_label(&gpu.vendor)?;
    }
    Ok(())
}

fn validate_label(value: &str) -> Result<(), Box<dyn Error>> {
    if value.is_empty()
        || value.len() > MAX_DEVICE_RUNTIME_NAME_BYTES
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err("v2 placement-evaluation label is invalid".into());
    }
    Ok(())
}

fn validate_canonical_tag(value: &str) -> Result<(), Box<dyn Error>> {
    if value.is_empty()
        || value.len() > MAX_DEVICE_RUNTIME_NAME_BYTES
        || value != value.to_ascii_lowercase()
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'_' | b'-' | b'+')
        })
    {
        return Err("v2 placement-evaluation canonical tag is invalid".into());
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
        return Err(format!(
            "device inventory placement-evaluation-v2 input exceeds {MAX_INPUT_BYTES} bytes"
        )
        .into());
    }
    Ok(bytes)
}
