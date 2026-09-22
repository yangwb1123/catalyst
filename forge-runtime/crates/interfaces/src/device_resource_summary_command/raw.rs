use std::collections::{HashMap, HashSet};

use super::{
    Authority, DeviceDeclaration, EVALUATION_MODE, GpuDeclaration, InventoryCandidate,
    MAX_DECLARATIONS, Owner, PLACEMENT_SCHEMA, PlacementObservation,
};

pub(super) const NOTICE: &str = "Every owner, instance, resource, placement, and eligibility value is an unverified caller declaration. This read-only summary aggregates declarations, selects no target, and grants no execution authority.";
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ComputedSummary {
    pub(super) schema_version: &'static str,
    pub(super) evaluation_mode: &'static str,
    pub(super) owner: Owner,
    pub(super) conversation_id: String,
    pub(super) run_id: String,
    pub(super) evaluated_at_ms: u64,
    pub(super) owner_declaration_unverified: bool,
    pub(super) inventory_declarations_unverified: bool,
    pub(super) placement_declaration_unverified: bool,
    pub(super) notice: &'static str,
    pub(super) device_count: usize,
    pub(super) runner_instance_count: usize,
    pub(super) available_cpu_cores: u64,
    pub(super) available_memory_bytes: u64,
    pub(super) available_storage_bytes: u64,
    pub(super) available_gpu_count: usize,
    pub(super) available_gpu_memory_bytes: u64,
    pub(super) eligible_device_count: usize,
    pub(super) eligible_instance_count: usize,
    pub(super) selected_device_id: Option<String>,
    pub(super) selected_instance_id: Option<String>,
    pub(super) authority: Authority,
}

pub(super) fn summarize_declarations(
    owner: &Owner,
    inventory: &[InventoryCandidate],
    placement: &PlacementObservation,
) -> Result<ComputedSummary, Box<dyn std::error::Error>> {
    if !valid_owner(owner)
        || inventory.len() > MAX_DECLARATIONS
        || !valid_placement(placement, owner)
        || placement.decisions.len() != inventory.len()
    {
        return Err("device resource summary declaration binding is invalid".into());
    }
    let by_device = validate_inventory(owner, inventory)?;
    let eligible = count_eligible_decisions(placement, &by_device)?;
    let resources = aggregate_resources(inventory)?;
    Ok(build_summary(
        owner,
        inventory.len(),
        placement,
        eligible,
        resources,
    ))
}

fn validate_inventory<'a>(
    owner: &Owner,
    inventory: &'a [InventoryCandidate],
) -> Result<HashMap<&'a str, &'a InventoryCandidate>, Box<dyn std::error::Error>> {
    let mut by_device = HashMap::with_capacity(inventory.len());
    let mut instance_ids = HashSet::with_capacity(inventory.len());
    for candidate in inventory {
        validate_candidate(candidate, owner)?;
        if by_device
            .insert(candidate.device.device_id.as_str(), candidate)
            .is_some()
            || !instance_ids.insert(candidate.instance_id.as_str())
        {
            return Err("device resource summary contains duplicate inventory identifiers".into());
        }
    }
    Ok(by_device)
}

fn count_eligible_decisions<'a>(
    placement: &PlacementObservation,
    by_device: &HashMap<&'a str, &'a InventoryCandidate>,
) -> Result<(usize, usize), Box<dyn std::error::Error>> {
    let mut decisions = placement.decisions.iter().collect::<Vec<_>>();
    decisions.sort_by(|left, right| {
        left.device_id
            .cmp(&right.device_id)
            .then_with(|| left.instance_id.cmp(&right.instance_id))
    });
    let mut decision_devices = HashSet::with_capacity(decisions.len());
    let mut decision_instances = HashSet::with_capacity(decisions.len());
    let mut eligible_devices = 0;
    let mut eligible_instances = 0;
    for decision in decisions {
        if !valid_device_id(&decision.device_id)
            || !valid_session_identifier(&decision.instance_id)
            || !decision_devices.insert(decision.device_id.as_str())
            || !decision_instances.insert(decision.instance_id.as_str())
        {
            return Err("device resource summary placement decisions are invalid".into());
        }
        let Some(candidate) = by_device.get(decision.device_id.as_str()) else {
            return Err("device resource summary placement device is unbound".into());
        };
        if candidate.instance_id != decision.instance_id {
            return Err("device resource summary placement instance is unbound".into());
        }
        if decision.matches_requirements {
            eligible_devices += 1;
            eligible_instances += 1;
        }
    }
    Ok((eligible_devices, eligible_instances))
}

struct ResourceTotals {
    available_cpu_cores: u64,
    available_memory_bytes: u64,
    available_storage_bytes: u64,
    available_gpu_count: usize,
    available_gpu_memory_bytes: u64,
}

fn aggregate_resources(
    inventory: &[InventoryCandidate],
) -> Result<ResourceTotals, Box<dyn std::error::Error>> {
    let mut available_cpu_cores = 0;
    let mut available_memory_bytes = 0;
    let mut available_storage_bytes = 0;
    let mut available_gpu_count = 0;
    let mut available_gpu_memory_bytes = 0;
    for candidate in inventory {
        available_cpu_cores = checked_add(
            available_cpu_cores,
            u64::from(candidate.device.available_cpu_cores),
        )?;
        available_memory_bytes = checked_add(
            available_memory_bytes,
            candidate.device.available_memory_bytes,
        )?;
        available_storage_bytes = checked_add(
            available_storage_bytes,
            candidate.device.available_storage_bytes,
        )?;
        if candidate.device.gpu.present {
            available_gpu_count += 1;
            available_gpu_memory_bytes = checked_add(
                available_gpu_memory_bytes,
                candidate.device.gpu.memory_bytes,
            )?;
        }
    }
    Ok(ResourceTotals {
        available_cpu_cores,
        available_memory_bytes,
        available_storage_bytes,
        available_gpu_count,
        available_gpu_memory_bytes,
    })
}

fn build_summary(
    owner: &Owner,
    count: usize,
    placement: &PlacementObservation,
    (eligible_devices, eligible_instances): (usize, usize),
    resources: ResourceTotals,
) -> ComputedSummary {
    ComputedSummary {
        schema_version: "forge.device-resource-summary/v1",
        evaluation_mode: EVALUATION_MODE,
        owner: owner.clone(),
        conversation_id: placement.conversation_id.clone(),
        run_id: placement.run_id.clone(),
        evaluated_at_ms: placement.evaluated_at_ms,
        owner_declaration_unverified: true,
        inventory_declarations_unverified: true,
        placement_declaration_unverified: true,
        notice: NOTICE,
        device_count: count,
        runner_instance_count: count,
        available_cpu_cores: resources.available_cpu_cores,
        available_memory_bytes: resources.available_memory_bytes,
        available_storage_bytes: resources.available_storage_bytes,
        available_gpu_count: resources.available_gpu_count,
        available_gpu_memory_bytes: resources.available_gpu_memory_bytes,
        eligible_device_count: eligible_devices,
        eligible_instance_count: eligible_instances,
        selected_device_id: None,
        selected_instance_id: None,
        authority: Authority::default(),
    }
}

fn validate_candidate(
    candidate: &InventoryCandidate,
    owner: &Owner,
) -> Result<(), Box<dyn std::error::Error>> {
    if !valid_session_identifier(&candidate.instance_id)
        || candidate.device.owner != *owner
        || !valid_device(&candidate.device)
    {
        return Err(format!(
            "inventory candidate {} is invalid or owner-unbound",
            candidate.device.device_id
        )
        .into());
    }
    Ok(())
}

fn valid_placement(value: &PlacementObservation, owner: &Owner) -> bool {
    value.schema_version == PLACEMENT_SCHEMA
        && value.evaluation_mode == EVALUATION_MODE
        && value.owner == *owner
        && valid_session_identifier(&value.conversation_id)
        && valid_session_identifier(&value.run_id)
        && value.evaluated_at_ms > 0
        && value.evaluated_at_ms <= MAX_SAFE_INTEGER
        && value.owner_declaration_unverified
        && value.device_attributes_unverified
        && value.selected_device_id.is_none()
        && value.selected_instance_id.is_none()
        && value.authority == Authority::default()
}

fn valid_device(value: &DeviceDeclaration) -> bool {
    let _ = (value.concurrency_limit, value.active_concurrency);
    valid_device_id(&value.device_id)
        && valid_owner(&value.owner)
        && valid_approval(&value.approval_state)
        && valid_cordon(&value.cordon_state)
        && valid_liveness(&value.liveness)
        && value.snapshot_observed_at_ms <= MAX_SAFE_INTEGER
        && value.lease_expires_at_ms <= MAX_SAFE_INTEGER
        && valid_token(&value.os)
        && valid_token(&value.architecture)
        && value.available_memory_bytes <= MAX_SAFE_INTEGER
        && value.available_storage_bytes <= MAX_SAFE_INTEGER
        && valid_unique_tokens(&value.runtimes, valid_token)
        && valid_gpu(&value.gpu)
        && valid_unique_tokens(&value.data_residency_zones, valid_zone)
        && valid_device_trust_zone(&value.trust_zone)
        && valid_unique_tokens(&value.sandbox_levels, valid_sandbox_level)
}

fn valid_gpu(value: &GpuDeclaration) -> bool {
    if value.present {
        value.memory_bytes <= MAX_SAFE_INTEGER
            && (value.runtime.is_empty() || valid_token(&value.runtime))
    } else {
        value.memory_bytes == 0 && value.runtime.is_empty()
    }
}

fn valid_owner(value: &Owner) -> bool {
    [
        value.issuer.as_str(),
        value.subject.as_str(),
        value.tenant_id.as_str(),
    ]
    .into_iter()
    .all(valid_owner_part)
}

fn valid_owner_part(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn valid_device_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 128
        && bytes[0].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
}

fn valid_session_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 128
        && bytes[0].is_ascii_alphanumeric()
        && bytes.iter().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'+' | b'/' | b'-')
        })
}

fn valid_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'+' | b'/' | b'-')
        })
}

fn valid_zone(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn valid_unique_tokens(values: &[String], validator: fn(&str) -> bool) -> bool {
    if values.len() > 32 {
        return false;
    }
    let mut seen = HashSet::with_capacity(values.len());
    values
        .iter()
        .all(|value| validator(value) && seen.insert(value.as_str()))
}

fn valid_approval(value: &str) -> bool {
    matches!(value, "approved" | "pending" | "revoked" | "unknown")
}

fn valid_cordon(value: &str) -> bool {
    matches!(value, "clear" | "cordoned" | "unknown")
}

fn valid_liveness(value: &str) -> bool {
    matches!(value, "online" | "offline" | "unknown")
}

fn valid_device_trust_zone(value: &str) -> bool {
    matches!(
        value,
        "untrusted" | "low" | "standard" | "high" | "restricted" | "unknown"
    )
}

fn valid_sandbox_level(value: &str) -> bool {
    matches!(value, "process" | "container" | "microvm")
}

fn checked_add(left: u64, right: u64) -> Result<u64, Box<dyn std::error::Error>> {
    left.checked_add(right)
        .filter(|value| *value <= MAX_SAFE_INTEGER)
        .ok_or_else(|| "device resource summary exceeds the safe integer range".into())
}
