use super::MAX_SAFE_INTEGER;
use super::wire::{Case, Owner, Requirements, SourceCapabilities, SourceCase, SourceFixture};
use forge_runtime_domain::{
    CapabilitySnapshot, Device, DeviceApprovalState, DeviceId, DevicePlacementPolicy,
    DevicePlacementRequirements, GpuCapability, PersistedInventoryDevice, RunnerInstance,
    RunnerInstanceId, RunnerLiveness, SnapshotOwner, TenantId,
    build_persisted_inventory_placement_input, restore_persisted_inventory,
};
use std::error::Error;

pub(super) fn owner(value: &Owner) -> Result<SnapshotOwner, Box<dyn Error>> {
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

pub(super) fn requirements(
    value: &Requirements,
) -> Result<DevicePlacementRequirements, Box<dyn Error>> {
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

pub(super) fn build_input(
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
    let (device_id, persisted) = build_device(source, source_case, case, owner_value)?;
    let runner = build_runner(source, source_case, case, device_id)?;
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

fn build_device(
    source: &SourceFixture,
    source_case: &SourceCase,
    case: &Case,
    owner_value: &SnapshotOwner,
) -> Result<(DeviceId, PersistedInventoryDevice), Box<dyn Error>> {
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
    Ok((device_id, persisted))
}

fn build_runner(
    source: &SourceFixture,
    source_case: &SourceCase,
    case: &Case,
    device_id: DeviceId,
) -> Result<RunnerInstance, Box<dyn Error>> {
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
    Ok(runner)
}
