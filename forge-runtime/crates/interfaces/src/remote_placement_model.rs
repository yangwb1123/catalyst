use std::collections::BTreeSet;

use serde::Deserialize;

pub(super) const REQUEST_SCHEMA: &str = "forge.device-placement-dry-run/v1";
pub(super) const MAX_DEVICES: usize = 128;
pub(super) const MAX_ARRAY_ITEMS: usize = 32;
pub(super) const MAX_EXCLUSION_REASONS: usize = 64;
pub(super) const MAX_OWNER_PART_BYTES: usize = 512;
pub(super) const MAX_TOKEN_BYTES: usize = 128;
pub(super) const MAX_SNAPSHOT_AGE_MS: i64 = 24 * 60 * 60 * 1000;
pub(super) const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct Owner {
    pub(super) issuer: String,
    pub(super) subject: String,
    pub(super) tenant_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PlacementRequest {
    pub(super) schema_version: String,
    pub(super) evaluated_at_ms: i64,
    pub(super) owner: Owner,
    pub(super) max_snapshot_age_ms: i64,
    pub(super) requirements: Requirements,
    pub(super) devices: Vec<Device>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Requirements {
    pub(super) os: String,
    pub(super) architecture: String,
    pub(super) min_cpu_cores: u32,
    pub(super) min_memory_bytes: u64,
    pub(super) min_storage_bytes: u64,
    pub(super) runtime: String,
    pub(super) gpu: GpuRequirement,
    pub(super) data_residency_zones: Vec<String>,
    pub(super) minimum_trust_zone: String,
    pub(super) sandbox_floor: String,
    pub(super) concurrency_slots: u16,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GpuRequirement {
    pub(super) required: bool,
    pub(super) min_memory_bytes: u64,
    pub(super) runtime: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Device {
    pub(super) device_id: String,
    pub(super) owner: Owner,
    pub(super) approval_state: String,
    pub(super) cordon_state: String,
    pub(super) liveness: String,
    pub(super) snapshot_observed_at_ms: i64,
    pub(super) lease_expires_at_ms: i64,
    pub(super) os: String,
    pub(super) architecture: String,
    pub(super) available_cpu_cores: u32,
    pub(super) available_memory_bytes: u64,
    pub(super) available_storage_bytes: u64,
    pub(super) runtimes: Vec<String>,
    pub(super) gpu: GpuDeclaration,
    pub(super) data_residency_zones: Vec<String>,
    pub(super) trust_zone: String,
    pub(super) sandbox_levels: Vec<String>,
    pub(super) concurrency_limit: u16,
    pub(super) active_concurrency: u16,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GpuDeclaration {
    pub(super) present: bool,
    pub(super) memory_bytes: u64,
    pub(super) runtime: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct DeviceResult {
    pub(super) device_id: String,
    pub(super) attributes_unverified: bool,
    pub(super) matches_requirements: bool,
    pub(super) exclusion_reasons: Vec<String>,
}

pub(super) fn decode_request(value: &serde_json::Value) -> Result<PlacementRequest, String> {
    let request: PlacementRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("remote placement input has an invalid shape: {error}"))?;
    validate_request(&request)?;
    Ok(request)
}

pub(super) fn validate_request(request: &PlacementRequest) -> Result<(), String> {
    if request.schema_version != REQUEST_SCHEMA
        || request.evaluated_at_ms <= 0
        || request.evaluated_at_ms > MAX_SAFE_INTEGER
        || request.max_snapshot_age_ms <= 0
        || request.max_snapshot_age_ms > MAX_SNAPSHOT_AGE_MS
        || request.devices.len() > MAX_DEVICES
        || !valid_owner(&request.owner)
        || !valid_requirements(&request.requirements)
    {
        return Err("remote placement input is invalid".into());
    }
    let mut ids = BTreeSet::new();
    for device in &request.devices {
        if !valid_device(device) || !ids.insert(device.device_id.as_str()) {
            return Err("remote placement input contains an invalid or duplicate device".into());
        }
    }
    Ok(())
}

pub(super) fn evaluate(request: &PlacementRequest) -> Vec<DeviceResult> {
    let mut results = request
        .devices
        .iter()
        .map(|device| {
            let mut reasons = Vec::new();
            append_state_reasons(&mut reasons, request, device);
            append_resource_reasons(&mut reasons, &request.requirements, device);
            append_policy_reasons(&mut reasons, &request.requirements, device);
            reasons.sort();
            DeviceResult {
                device_id: device.device_id.clone(),
                attributes_unverified: true,
                matches_requirements: reasons.is_empty(),
                exclusion_reasons: reasons,
            }
        })
        .collect::<Vec<_>>();
    results.sort_by(|left, right| left.device_id.cmp(&right.device_id));
    results
}

fn valid_owner(owner: &Owner) -> bool {
    [
        owner.issuer.as_str(),
        owner.subject.as_str(),
        owner.tenant_id.as_str(),
    ]
    .into_iter()
    .all(valid_owner_part)
}

fn valid_owner_part(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_OWNER_PART_BYTES
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn valid_requirements(value: &Requirements) -> bool {
    valid_token(&value.os)
        && valid_token(&value.architecture)
        && value.min_cpu_cores > 0
        && value.min_memory_bytes > 0
        && value.min_storage_bytes > 0
        && safe_u64(value.min_memory_bytes)
        && safe_u64(value.min_storage_bytes)
        && valid_token(&value.runtime)
        && valid_trust_zone(&value.minimum_trust_zone)
        && valid_sandbox_level(&value.sandbox_floor)
        && value.concurrency_slots > 0
        && !value.data_residency_zones.is_empty()
        && valid_unique_tokens(&value.data_residency_zones, valid_zone)
        && valid_gpu_requirement(&value.gpu)
}

fn valid_gpu_requirement(value: &GpuRequirement) -> bool {
    if value.required {
        safe_u64(value.min_memory_bytes)
            && (value.runtime.is_empty() || valid_token(&value.runtime))
    } else {
        value.min_memory_bytes == 0 && value.runtime.is_empty()
    }
}

fn valid_device(value: &Device) -> bool {
    valid_device_id(&value.device_id)
        && valid_owner(&value.owner)
        && valid_approval(&value.approval_state)
        && valid_cordon_state(&value.cordon_state)
        && valid_liveness(&value.liveness)
        && value.snapshot_observed_at_ms >= 0
        && value.snapshot_observed_at_ms <= MAX_SAFE_INTEGER
        && value.lease_expires_at_ms >= 0
        && value.lease_expires_at_ms <= MAX_SAFE_INTEGER
        && valid_token(&value.os)
        && valid_token(&value.architecture)
        && safe_u64(value.available_memory_bytes)
        && safe_u64(value.available_storage_bytes)
        && valid_unique_tokens(&value.runtimes, valid_token)
        && valid_gpu_declaration(&value.gpu)
        && valid_unique_tokens(&value.data_residency_zones, valid_zone)
        && valid_device_trust_zone(&value.trust_zone)
        && valid_unique_tokens(&value.sandbox_levels, valid_sandbox_level)
}

fn valid_gpu_declaration(value: &GpuDeclaration) -> bool {
    if value.present {
        safe_u64(value.memory_bytes) && (value.runtime.is_empty() || valid_token(&value.runtime))
    } else {
        value.memory_bytes == 0 && value.runtime.is_empty()
    }
}

fn safe_u64(value: u64) -> bool {
    value <= MAX_SAFE_INTEGER as u64
}

fn valid_unique_tokens(values: &[String], valid: fn(&str) -> bool) -> bool {
    if values.len() > MAX_ARRAY_ITEMS {
        return false;
    }
    let mut seen = BTreeSet::new();
    values
        .iter()
        .all(|value| valid(value) && seen.insert(value.as_str()))
}

pub(super) fn valid_device_id(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_TOKEN_BYTES {
        return false;
    }
    value.chars().enumerate().all(|(index, character)| {
        character.is_ascii_alphanumeric()
            || (index > 0 && matches!(character, '.' | '_' | ':' | '-'))
    })
}

pub(super) fn valid_token(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_TOKEN_BYTES {
        return false;
    }
    value.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | ':' | '+' | '/' | '-')
    })
}

fn valid_zone(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
}

fn valid_trust_zone(value: &str) -> bool {
    matches!(
        value,
        "untrusted" | "low" | "standard" | "high" | "restricted"
    )
}

fn valid_device_trust_zone(value: &str) -> bool {
    valid_trust_zone(value) || value == "unknown"
}

fn valid_sandbox_level(value: &str) -> bool {
    matches!(value, "process" | "container" | "microvm")
}

fn valid_approval(value: &str) -> bool {
    matches!(value, "approved" | "pending" | "revoked" | "unknown")
}

fn valid_cordon_state(value: &str) -> bool {
    matches!(value, "clear" | "cordoned" | "unknown")
}

fn valid_liveness(value: &str) -> bool {
    matches!(value, "online" | "offline" | "unknown")
}

fn append_state_reasons(reasons: &mut Vec<String>, request: &PlacementRequest, device: &Device) {
    if device.owner != request.owner {
        reasons.push("owner_mismatch".into());
    }
    match device.approval_state.as_str() {
        "approved" => {}
        "pending" => reasons.push("approval_pending".into()),
        "revoked" => reasons.push("device_revoked".into()),
        _ => reasons.push("approval_unconfirmed".into()),
    }
    match device.cordon_state.as_str() {
        "clear" => {}
        "cordoned" => reasons.push("device_cordoned".into()),
        _ => reasons.push("cordon_unconfirmed".into()),
    }
    match device.liveness.as_str() {
        "online" => {}
        "offline" => reasons.push("declared_offline".into()),
        _ => reasons.push("liveness_unconfirmed".into()),
    }
    if device.snapshot_observed_at_ms > request.evaluated_at_ms {
        reasons.push("snapshot_declared_from_future".into());
    } else if request.evaluated_at_ms - device.snapshot_observed_at_ms > request.max_snapshot_age_ms
    {
        reasons.push("snapshot_stale".into());
    }
    if device.lease_expires_at_ms <= request.evaluated_at_ms {
        reasons.push("declared_lease_expired".into());
    }
}

fn append_resource_reasons(reasons: &mut Vec<String>, requirement: &Requirements, device: &Device) {
    if device.os != requirement.os {
        reasons.push("os_mismatch".into());
    }
    if device.architecture != requirement.architecture {
        reasons.push("architecture_mismatch".into());
    }
    if device.available_cpu_cores < requirement.min_cpu_cores {
        reasons.push("cpu_cores_insufficient".into());
    }
    if device.available_memory_bytes < requirement.min_memory_bytes {
        reasons.push("memory_insufficient".into());
    }
    if device.available_storage_bytes < requirement.min_storage_bytes {
        reasons.push("storage_insufficient".into());
    }
    if !device
        .runtimes
        .iter()
        .any(|runtime| runtime == &requirement.runtime)
    {
        reasons.push("runtime_missing".into());
    }
    if requirement.gpu.required {
        if !device.gpu.present {
            reasons.push("gpu_missing".into());
        } else {
            if device.gpu.memory_bytes < requirement.gpu.min_memory_bytes {
                reasons.push("gpu_memory_insufficient".into());
            }
            if !requirement.gpu.runtime.is_empty() && device.gpu.runtime != requirement.gpu.runtime
            {
                reasons.push("gpu_runtime_mismatch".into());
            }
        }
    }
    if device.active_concurrency > device.concurrency_limit
        || requirement.concurrency_slots
            > device
                .concurrency_limit
                .saturating_sub(device.active_concurrency)
    {
        reasons.push("concurrency_capacity_insufficient".into());
    }
}

fn append_policy_reasons(reasons: &mut Vec<String>, requirement: &Requirements, device: &Device) {
    if !requirement.data_residency_zones.iter().any(|zone| {
        device
            .data_residency_zones
            .iter()
            .any(|actual| actual == zone)
    }) {
        reasons.push("data_residency_zone_mismatch".into());
    }
    let trust_rank = |value: &str| match value {
        "untrusted" => Some(0),
        "low" => Some(1),
        "standard" => Some(2),
        "high" => Some(3),
        "restricted" => Some(4),
        _ => None,
    };
    match (
        trust_rank(&device.trust_zone),
        trust_rank(&requirement.minimum_trust_zone),
    ) {
        (None, _) => reasons.push("trust_zone_unconfirmed".into()),
        (Some(actual), Some(minimum)) if actual < minimum => {
            reasons.push("trust_zone_below_minimum".into())
        }
        (Some(_), Some(_)) => {}
        (_, None) => unreachable!("validated minimum trust zone"),
    }
    let sandbox_rank = |value: &str| match value {
        "process" => 1,
        "container" => 2,
        "microvm" => 3,
        _ => 0,
    };
    if !device
        .sandbox_levels
        .iter()
        .any(|level| sandbox_rank(level) >= sandbox_rank(&requirement.sandbox_floor))
    {
        reasons.push("sandbox_floor_unmet".into());
    }
}
