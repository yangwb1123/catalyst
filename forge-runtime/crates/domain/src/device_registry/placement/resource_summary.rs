use std::collections::{HashMap, HashSet};
use std::fmt;

use super::{DevicePlacementCandidate, SessionPlacementAuthority, SessionPlacementObservation};

pub const DEVICE_RESOURCE_SUMMARY_SCHEMA_VERSION: &str = "forge.device-resource-summary/v1";
pub const DEVICE_RESOURCE_SUMMARY_NOTICE: &str = "Every owner, instance, resource, placement, and eligibility value is an unverified caller declaration. This read-only summary aggregates declarations, selects no target, and grants no execution authority.";
const EVALUATION_MODE: &str = "offline_static_only";
const MAX_RESOURCE_SUMMARY_DECLARATIONS: usize = 128;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// Caller-declared owner metadata for one resource aggregate.
pub type DeviceResourceSummaryOwner = super::SessionPlacementOwner;

/// Inputs to the pure resource aggregate. Inventory and placement are values
/// supplied by the caller; neither establishes authority or performs work.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceResourceSummaryRequest {
    pub owner: DeviceResourceSummaryOwner,
    pub inventory: Vec<DevicePlacementCandidate>,
    pub placement: SessionPlacementObservation,
}

/// Stable totals across declared devices and Runner instances. Eligibility is
/// copied from placement declarations and never becomes target selection.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceResourceSummary {
    pub schema_version: &'static str,
    pub evaluation_mode: &'static str,
    pub owner: DeviceResourceSummaryOwner,
    pub conversation_id: String,
    pub run_id: String,
    pub evaluated_at_ms: u64,
    pub owner_declaration_unverified: bool,
    pub inventory_declarations_unverified: bool,
    pub placement_declaration_unverified: bool,
    pub notice: &'static str,
    pub device_count: usize,
    pub runner_instance_count: usize,
    pub available_cpu_cores: u64,
    pub available_memory_bytes: u64,
    pub available_storage_bytes: u64,
    pub available_gpu_count: usize,
    pub available_gpu_memory_bytes: u64,
    pub eligible_device_count: usize,
    pub eligible_instance_count: usize,
    pub selected_device_id: Option<String>,
    pub selected_instance_id: Option<String>,
    pub authority: SessionPlacementAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceResourceSummaryError {
    InvalidBinding,
    InvalidInventory,
    InvalidPlacement,
    ResourceOverflow,
}

impl fmt::Display for DeviceResourceSummaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "device resource summary is invalid: {self:?}")
    }
}

impl std::error::Error for DeviceResourceSummaryError {}

/// Aggregates caller-declared resources and placement eligibility in stable
/// device/instance order without selection, reservation, authorization, or
/// dispatch.
#[allow(clippy::too_many_lines)]
///
/// # Errors
///
/// Returns an error when owner/session bindings, inventory pairs, placement
/// pairs, or aggregate arithmetic are invalid.
pub fn observe_device_resource_summary(
    input: DeviceResourceSummaryRequest,
) -> Result<DeviceResourceSummary, DeviceResourceSummaryError> {
    if input.inventory.len() > MAX_RESOURCE_SUMMARY_DECLARATIONS
        || !valid_placement(&input.placement, &input.owner)
    {
        return Err(DeviceResourceSummaryError::InvalidBinding);
    }

    let mut by_device = HashMap::with_capacity(input.inventory.len());
    let mut instance_ids = HashSet::with_capacity(input.inventory.len());
    for candidate in &input.inventory {
        if candidate.device().tenant_id() != &input.owner.tenant_id
            || !instance_ids.insert(candidate.instance().instance_id().clone())
            || by_device
                .insert(candidate.device().id().clone(), candidate)
                .is_some()
        {
            return Err(DeviceResourceSummaryError::InvalidInventory);
        }
    }
    if input.placement.decisions.len() != input.inventory.len() {
        return Err(DeviceResourceSummaryError::InvalidBinding);
    }

    let mut decisions = input.placement.decisions.clone();
    decisions.sort_by(|left, right| {
        left.device_id
            .cmp(&right.device_id)
            .then_with(|| left.instance_id.cmp(&right.instance_id))
    });
    let mut device_ids = HashSet::with_capacity(decisions.len());
    let mut decision_instances = HashSet::with_capacity(decisions.len());
    let mut eligible_devices = 0;
    let mut eligible_instances = 0;
    for decision in decisions {
        if !device_ids.insert(decision.device_id.clone())
            || !decision_instances.insert(decision.instance_id.clone())
        {
            return Err(DeviceResourceSummaryError::InvalidPlacement);
        }
        let Ok(device_id) = super::super::model::DeviceId::parse(decision.device_id.clone()) else {
            return Err(DeviceResourceSummaryError::InvalidPlacement);
        };
        let Some(candidate) = by_device.get(&device_id) else {
            return Err(DeviceResourceSummaryError::InvalidBinding);
        };
        if candidate.instance().instance_id().as_str() != decision.instance_id {
            return Err(DeviceResourceSummaryError::InvalidBinding);
        }
        if decision.matches_requirements {
            eligible_devices += 1;
            eligible_instances += 1;
        }
    }

    let mut available_cpu_cores = 0;
    let mut available_memory_bytes = 0;
    let mut available_storage_bytes = 0;
    let mut available_gpu_count = 0;
    let mut available_gpu_memory_bytes = 0;
    for candidate in &input.inventory {
        let capabilities = candidate.instance().capabilities();
        available_cpu_cores = checked_add(
            available_cpu_cores,
            u64::from(capabilities.available_cpu_cores()),
        )?;
        available_memory_bytes = checked_add(
            available_memory_bytes,
            capabilities.available_memory_bytes(),
        )?;
        available_storage_bytes = checked_add(
            available_storage_bytes,
            capabilities.available_storage_bytes(),
        )?;
        for gpu in capabilities.gpus() {
            available_gpu_count += 1;
            available_gpu_memory_bytes =
                checked_add(available_gpu_memory_bytes, gpu.memory_bytes())?;
        }
    }
    let count = input.inventory.len();
    Ok(DeviceResourceSummary {
        schema_version: DEVICE_RESOURCE_SUMMARY_SCHEMA_VERSION,
        evaluation_mode: EVALUATION_MODE,
        owner: input.owner,
        conversation_id: input.placement.conversation_id,
        run_id: input.placement.run_id,
        evaluated_at_ms: input.placement.evaluated_at_ms,
        owner_declaration_unverified: true,
        inventory_declarations_unverified: true,
        placement_declaration_unverified: true,
        notice: DEVICE_RESOURCE_SUMMARY_NOTICE,
        device_count: count,
        runner_instance_count: count,
        available_cpu_cores,
        available_memory_bytes,
        available_storage_bytes,
        available_gpu_count,
        available_gpu_memory_bytes,
        eligible_device_count: eligible_devices,
        eligible_instance_count: eligible_instances,
        selected_device_id: None,
        selected_instance_id: None,
        authority: SessionPlacementAuthority::default(),
    })
}

fn valid_placement(
    placement: &SessionPlacementObservation,
    owner: &DeviceResourceSummaryOwner,
) -> bool {
    placement.schema_version == super::SESSION_PLACEMENT_OBSERVATION_SCHEMA_VERSION
        && placement.evaluation_mode == EVALUATION_MODE
        && valid_owner(&placement.owner)
        && placement.owner == *owner
        && valid_identifier(&placement.conversation_id)
        && valid_identifier(&placement.run_id)
        && placement.evaluated_at_ms > 0
        && placement.evaluated_at_ms <= MAX_SAFE_INTEGER
        && placement.owner_declaration_unverified
        && placement.device_attributes_unverified
        && placement.selected_device_id.is_none()
        && placement.selected_instance_id.is_none()
        && placement.authority == SessionPlacementAuthority::default()
}

fn valid_owner(owner: &DeviceResourceSummaryOwner) -> bool {
    valid_owner_part(&owner.issuer) && valid_owner_part(&owner.subject)
}

fn valid_owner_part(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn valid_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= super::super::MAX_DEVICE_IDENTIFIER_BYTES
        && bytes[0].is_ascii_alphanumeric()
        && bytes.iter().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'+' | b'/' | b'-')
        })
}

fn checked_add(left: u64, right: u64) -> Result<u64, DeviceResourceSummaryError> {
    left.checked_add(right)
        .filter(|value| *value <= MAX_SAFE_INTEGER)
        .ok_or(DeviceResourceSummaryError::ResourceOverflow)
}
