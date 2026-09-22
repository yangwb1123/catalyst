use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use forge_runtime_domain::{
    CapabilitySnapshot, Device, DeviceApprovalState, DeviceId, DevicePlacementAttributes,
    DevicePlacementCandidate, DevicePlacementDisposition, DevicePlacementExclusion,
    DevicePlacementPolicy, DevicePlacementRequest, DevicePlacementRequirements, GpuCapability,
    RunnerInstance, RunnerInstanceId, RunnerLiveness, SessionPlacementObservation,
    SessionPlacementObservationRequest, SessionPlacementOwner, TenantId, dry_run_device_placement,
    observe_session_placement as observe_session_placement_domain,
};
use serde::{Deserialize, Serialize};

use crate::args::{DeviceCommand, DevicePlacementCommand};

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const PARITY_SCHEMA: &str = "forge.device-placement-policy-parity-test/v1";
const EVALUATION_MODE: &str = "offline_placement_dry_run";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlacementFixture {
    schema_version: String,
    evaluated_at_ms: u64,
    max_snapshot_age_ms: u64,
    owner: Owner,
    requirements: Requirements,
    candidates: Vec<Candidate>,
    expected: Vec<Expected>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
#[serde(deny_unknown_fields)]
struct Candidate {
    instance_id: String,
    device: DeviceDeclaration,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviceDeclaration {
    device_id: String,
    owner: Owner,
    approval_state: String,
    cordon_state: String,
    liveness: String,
    snapshot_observed_at_ms: u64,
    lease_expires_at_ms: u64,
    os: String,
    architecture: String,
    available_cpu_cores: u32,
    available_memory_bytes: u64,
    available_storage_bytes: u64,
    runtimes: Vec<String>,
    gpu: GpuDeclaration,
    data_residency_zones: Vec<String>,
    trust_zone: String,
    sandbox_levels: Vec<String>,
    concurrency_limit: u16,
    active_concurrency: u16,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GpuDeclaration {
    present: bool,
    memory_bytes: u64,
    runtime: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    device_id: String,
    matches_requirements: bool,
    exclusion_reasons: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PlacementDryRunOutput {
    v: u16,
    #[serde(rename = "type")]
    output_type: &'static str,
    schema_version: String,
    evaluation_mode: &'static str,
    evaluated_at_ms: u64,
    owner: Owner,
    decisions: Vec<DecisionOutput>,
    authority: AuthorityOutput,
}

#[derive(Debug, Serialize)]
pub(crate) struct DecisionOutput {
    device_id: String,
    instance_id: String,
    matches_requirements: bool,
    exclusion_reasons: Vec<String>,
}

#[derive(Debug, Serialize, Default)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct AuthorityOutput {
    identity_verified: bool,
    heartbeat_persisted: bool,
    inventory_authoritative: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
}

pub(crate) fn execute(command: &DeviceCommand) -> Result<PlacementDryRunOutput, Box<dyn Error>> {
    let DeviceCommand::Placement(DevicePlacementCommand::DryRun { input }) = command else {
        return Err("device placement command is required".into());
    };
    let bytes = read_bounded_input(input)?;
    let fixture: PlacementFixture = serde_json::from_slice(&bytes)
        .map_err(|error| format!("device placement input is invalid JSON: {error}"))?;
    evaluate(fixture)
}

pub(crate) fn write_output(
    output: &PlacementDryRunOutput,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        writeln!(writer)
    } else {
        writeln!(
            writer,
            "offline placement dry-run [{}] at {}",
            output.schema_version, output.evaluated_at_ms
        )?;
        writeln!(
            writer,
            "authority: identity_verified=false heartbeat_persisted=false inventory_authoritative=false reservation_created=false execution_authorized=false dispatch_performed=false"
        )?;
        for decision in &output.decisions {
            if decision.matches_requirements {
                writeln!(
                    writer,
                    "{} / {}: eligible",
                    decision.device_id, decision.instance_id
                )?;
            } else {
                writeln!(
                    writer,
                    "{} / {}: excluded ({})",
                    decision.device_id,
                    decision.instance_id,
                    decision.exclusion_reasons.join(",")
                )?;
            }
        }
        Ok(())
    }
}

fn evaluate(fixture: PlacementFixture) -> Result<PlacementDryRunOutput, Box<dyn Error>> {
    validate_fixture_shape(&fixture)?;
    let tenant_id = TenantId::parse(fixture.owner.tenant_id.clone())?;
    let request =
        DevicePlacementRequest::new(tenant_id, build_requirements(&fixture.requirements)?);
    let candidates = fixture
        .candidates
        .iter()
        .map(|candidate| build_candidate(candidate, &fixture.owner, &fixture.requirements))
        .collect::<Result<Vec<_>, _>>()?;
    let decisions = dry_run_device_placement(&candidates, &request, fixture.evaluated_at_ms)?;
    validate_expected(&decisions, &fixture.expected)?;
    Ok(PlacementDryRunOutput {
        v: 1,
        output_type: "device_placement_dry_run",
        schema_version: fixture.schema_version,
        evaluation_mode: EVALUATION_MODE,
        evaluated_at_ms: fixture.evaluated_at_ms,
        owner: fixture.owner,
        decisions: decisions.iter().map(DecisionOutput::from_domain).collect(),
        authority: AuthorityOutput::default(),
    })
}

pub(crate) fn observe_session_placement_fixture(
    owner: SessionPlacementOwner,
    conversation_id: String,
    run_id: String,
    placement: serde_json::Value,
) -> Result<SessionPlacementObservation, Box<dyn Error>> {
    let fixture: PlacementFixture = serde_json::from_value(placement)
        .map_err(|error| format!("session placement input is invalid JSON: {error}"))?;
    validate_fixture_shape(&fixture)?;
    let placement_owner = SessionPlacementOwner {
        issuer: fixture.owner.issuer.clone(),
        subject: fixture.owner.subject.clone(),
        tenant_id: TenantId::parse(fixture.owner.tenant_id.clone())?,
    };
    let request = DevicePlacementRequest::new(
        placement_owner.tenant_id.clone(),
        build_requirements(&fixture.requirements)?,
    );
    let candidates = fixture
        .candidates
        .iter()
        .map(|candidate| build_candidate(candidate, &fixture.owner, &fixture.requirements))
        .collect::<Result<Vec<_>, _>>()?;
    validate_expected(
        &dry_run_device_placement(&candidates, &request, fixture.evaluated_at_ms)?,
        &fixture.expected,
    )?;
    Ok(observe_session_placement_domain(
        SessionPlacementObservationRequest {
            owner,
            placement_owner,
            conversation_id,
            run_id,
            placement: request,
            candidates,
            evaluated_at_ms: fixture.evaluated_at_ms,
        },
    )?)
}

fn validate_fixture_shape(fixture: &PlacementFixture) -> Result<(), Box<dyn Error>> {
    if fixture.schema_version != PARITY_SCHEMA {
        return Err(format!(
            "unsupported device placement schema {:?}; expected {PARITY_SCHEMA:?}",
            fixture.schema_version
        )
        .into());
    }
    if fixture.evaluated_at_ms == 0
        || fixture.max_snapshot_age_ms == 0
        || fixture.max_snapshot_age_ms > 90_000
        || fixture.candidates.len() > 10_000
        || fixture.expected.len() != fixture.candidates.len()
    {
        return Err("device placement input exceeds bounded contract shape".into());
    }
    if !fixture.requirements.gpu.runtime.is_empty()
        || fixture
            .candidates
            .iter()
            .any(|candidate| !candidate.device.gpu.runtime.is_empty())
    {
        return Err("GPU runtime declarations are not supported by placement parity v1".into());
    }
    Ok(())
}

fn validate_expected(
    decisions: &[forge_runtime_domain::DevicePlacementDecision],
    expected: &[Expected],
) -> Result<(), Box<dyn Error>> {
    for (decision, expected) in decisions.iter().zip(expected) {
        let output = DecisionOutput::from_domain(decision);
        if output.device_id != expected.device_id
            || output.matches_requirements != expected.matches_requirements
            || output.exclusion_reasons != expected.exclusion_reasons
        {
            return Err(format!(
                "placement parity expectation mismatch for {}",
                expected.device_id
            )
            .into());
        }
    }
    Ok(())
}

fn build_requirements(value: &Requirements) -> Result<DevicePlacementRequirements, Box<dyn Error>> {
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
        if value.gpu.required {
            value.gpu.min_memory_bytes
        } else {
            0
        },
    )?
    .with_policy(policy))
}

fn build_candidate(
    candidate: &Candidate,
    owner: &Owner,
    requirements: &Requirements,
) -> Result<DevicePlacementCandidate, Box<dyn Error>> {
    if candidate.device.owner.issuer != owner.issuer
        || candidate.device.owner.subject != owner.subject
        || candidate.device.owner.tenant_id != owner.tenant_id
    {
        return Err(format!(
            "candidate {} owner does not match the input owner declaration",
            candidate.device.device_id
        )
        .into());
    }
    let (device, device_id) = build_device(candidate)?;
    let instance = build_instance(candidate, device_id.clone(), requirements)?;
    let attributes = build_attributes(&candidate.device)?;
    Ok(DevicePlacementCandidate::new(device, instance)?.with_attributes(attributes))
}

fn build_device(candidate: &Candidate) -> Result<(Device, DeviceId), Box<dyn Error>> {
    let device_id = DeviceId::parse(candidate.device.device_id.clone())?;
    let tenant_id = TenantId::parse(candidate.device.owner.tenant_id.clone())?;
    let approval = match candidate.device.approval_state.as_str() {
        "approved" => DeviceApprovalState::Approved,
        "pending" => DeviceApprovalState::Pending,
        "revoked" => DeviceApprovalState::Revoked,
        value => return Err(format!("unknown approval_state {value:?}").into()),
    };
    let cordoned = match candidate.device.cordon_state.as_str() {
        "clear" => false,
        "cordoned" => true,
        value => return Err(format!("unknown cordon_state {value:?}").into()),
    };
    Ok((
        Device::restore(device_id.clone(), tenant_id, approval, cordoned),
        device_id,
    ))
}

fn build_instance(
    candidate: &Candidate,
    device_id: DeviceId,
    requirements: &Requirements,
) -> Result<RunnerInstance, Box<dyn Error>> {
    let liveness = match candidate.device.liveness.as_str() {
        "online" => RunnerLiveness::Online,
        "offline" => RunnerLiveness::Offline,
        value => return Err(format!("unknown liveness {value:?}").into()),
    };
    let instance_id = RunnerInstanceId::parse(candidate.instance_id.clone())?;
    let capabilities = build_capabilities(&candidate.device, requirements)?;
    Ok(RunnerInstance::restore(
        device_id,
        instance_id,
        1,
        1,
        candidate.device.snapshot_observed_at_ms,
        candidate.device.lease_expires_at_ms,
        liveness,
        capabilities,
    )?)
}

fn build_attributes(
    device: &DeviceDeclaration,
) -> Result<DevicePlacementAttributes, Box<dyn Error>> {
    Ok(DevicePlacementAttributes::new()
        .with_data_residency_zones(device.data_residency_zones.clone())?
        .with_trust_zone(&device.trust_zone)?
        .with_sandbox_levels(&device.sandbox_levels)?
        .with_concurrency(device.concurrency_limit, device.active_concurrency))
}

fn build_capabilities(
    value: &DeviceDeclaration,
    requirements: &Requirements,
) -> Result<CapabilitySnapshot, Box<dyn Error>> {
    let gpus = if value.gpu.present {
        vec![GpuCapability::new(
            "gpu-parity",
            "unknown",
            value.gpu.memory_bytes,
            value.gpu.memory_bytes,
        )?]
    } else {
        if value.gpu.memory_bytes != 0 {
            return Err("GPU memory must be zero when GPU is absent".into());
        }
        Vec::new()
    };
    Ok(CapabilitySnapshot::new(
        value.os.clone(),
        value.architecture.clone(),
        value.available_cpu_cores.max(requirements.min_cpu_cores),
        value.available_cpu_cores,
        value
            .available_memory_bytes
            .max(requirements.min_memory_bytes),
        value.available_memory_bytes,
        value
            .available_storage_bytes
            .max(requirements.min_storage_bytes),
        value.available_storage_bytes,
        gpus,
        value.runtimes.clone(),
    )?)
}

impl DecisionOutput {
    fn from_domain(decision: &forge_runtime_domain::DevicePlacementDecision) -> Self {
        let (matches_requirements, exclusion_reasons) = match decision.disposition() {
            DevicePlacementDisposition::Eligible => (true, Vec::new()),
            DevicePlacementDisposition::Excluded(reasons) => (
                false,
                reasons.iter().map(|reason| reason_code(*reason)).collect(),
            ),
        };
        Self {
            device_id: decision.device_id().as_str().to_owned(),
            instance_id: decision.instance_id().as_str().to_owned(),
            matches_requirements,
            exclusion_reasons,
        }
    }
}

fn reason_code(reason: DevicePlacementExclusion) -> String {
    match reason {
        DevicePlacementExclusion::MemoryCapacityInsufficient => "memory_insufficient",
        DevicePlacementExclusion::StorageCapacityInsufficient => "storage_insufficient",
        DevicePlacementExclusion::RunnerOffline => "declared_offline",
        DevicePlacementExclusion::HeartbeatObservedInFuture => "snapshot_declared_from_future",
        DevicePlacementExclusion::HeartbeatStale => "snapshot_stale",
        DevicePlacementExclusion::CapabilityLeaseExpired => "declared_lease_expired",
        DevicePlacementExclusion::CapabilityLeaseInvalid => "declared_lease_invalid",
        DevicePlacementExclusion::GpuCountInsufficient => "gpu_missing",
        reason => reason.as_str(),
    }
    .to_owned()
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
        return Err(format!("device placement input exceeds {MAX_INPUT_BYTES} bytes").into());
    }
    Ok(bytes)
}
