use super::wire::{CapabilitiesFixture, Case, RunnerFixture, StateFixture};
use forge_runtime_domain::{
    CapabilitySnapshot, Device, DeviceApprovalState, DeviceId, GpuCapability,
    PersistedInventoryDevice, RunnerInstance, RunnerInstanceId, RunnerLiveness, SnapshotOwner,
    TenantId,
};
use std::error::Error;

#[derive(Clone, Debug)]
pub(super) struct ParsedState {
    pub(super) revision: u64,
    pub(super) device: PersistedInventoryDevice,
    pub(super) runner: RunnerInstance,
}

pub(super) fn parse_state(value: &StateFixture) -> Result<ParsedState, Box<dyn Error>> {
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

pub(super) fn case_state(base: &ParsedState, case: &Case) -> Result<ParsedState, Box<dyn Error>> {
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

pub(super) fn replacement_runner(
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

pub(super) fn evaluation_owner(
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
