use super::{
    GpuDeclaration, GpuRequirement, MAX_CANDIDATES, MAX_SAFE_INTEGER, OwnerWire, PlacementDevice,
    PlacementRequest, PlacementRequirements, REQUEST_SCHEMA_VERSION,
};

pub(super) fn valid_placement(value: &PlacementRequest, expected_owner: &OwnerWire) -> bool {
    value.schema_version == REQUEST_SCHEMA_VERSION
        && value.evaluated_at_ms > 0
        && value.evaluated_at_ms.cast_unsigned() <= MAX_SAFE_INTEGER
        && value.max_snapshot_age_ms > 0
        && value.max_snapshot_age_ms.cast_unsigned() <= 86_400_000
        && valid_owner(&value.owner)
        && value.owner == *expected_owner
        && valid_requirements(&value.requirements)
        && value.devices.len() <= MAX_CANDIDATES
        && value
            .devices
            .iter()
            .all(|device| valid_device(device, expected_owner))
        && value.devices.iter().enumerate().all(|(index, device)| {
            value.devices[..index]
                .iter()
                .all(|previous| previous.device_id != device.device_id)
        })
}

pub(super) fn valid_requirements(value: &PlacementRequirements) -> bool {
    valid_token(&value.os)
        && valid_token(&value.architecture)
        && value.min_cpu_cores > 0
        && value.min_memory_bytes > 0
        && value.min_memory_bytes <= MAX_SAFE_INTEGER
        && value.min_storage_bytes > 0
        && value.min_storage_bytes <= MAX_SAFE_INTEGER
        && valid_token(&value.runtime)
        && valid_zones(&value.data_residency_zones)
        && matches!(
            value.minimum_trust_zone.as_str(),
            "untrusted" | "low" | "standard" | "high" | "restricted"
        )
        && matches!(
            value.sandbox_floor.as_str(),
            "process" | "container" | "microvm"
        )
        && value.concurrency_slots > 0
        && valid_gpu_requirement(&value.gpu)
}

pub(super) fn valid_gpu_requirement(value: &GpuRequirement) -> bool {
    if value.required {
        value.min_memory_bytes <= MAX_SAFE_INTEGER
            && (value.runtime.is_empty() || valid_token(&value.runtime))
    } else {
        value.min_memory_bytes == 0 && value.runtime.is_empty()
    }
}

pub(super) fn valid_device(value: &PlacementDevice, expected_owner: &OwnerWire) -> bool {
    let _ = (
        value.available_cpu_cores,
        value.concurrency_limit,
        value.active_concurrency,
    );
    valid_identifier(&value.device_id)
        && valid_owner(&value.owner)
        && value.owner == *expected_owner
        && matches!(
            value.approval_state.as_str(),
            "approved" | "pending" | "revoked" | "unknown"
        )
        && matches!(
            value.cordon_state.as_str(),
            "clear" | "cordoned" | "unknown"
        )
        && matches!(value.liveness.as_str(), "online" | "offline" | "unknown")
        && value.snapshot_observed_at_ms >= 0
        && value.snapshot_observed_at_ms.cast_unsigned() <= MAX_SAFE_INTEGER
        && value.lease_expires_at_ms >= 0
        && value.lease_expires_at_ms.cast_unsigned() <= MAX_SAFE_INTEGER
        && valid_token(&value.os)
        && valid_token(&value.architecture)
        && value.available_memory_bytes <= MAX_SAFE_INTEGER
        && value.available_storage_bytes <= MAX_SAFE_INTEGER
        && value.runtimes.len() <= 32
        && value.runtimes.iter().all(|item| valid_token(item))
        && valid_gpu_declaration(&value.gpu)
        && valid_zones(&value.data_residency_zones)
        && matches!(
            value.trust_zone.as_str(),
            "untrusted" | "low" | "standard" | "high" | "restricted" | "unknown"
        )
        && value.sandbox_levels.len() <= 32
        && value
            .sandbox_levels
            .iter()
            .all(|item| matches!(item.as_str(), "process" | "container" | "microvm"))
}

pub(super) fn valid_gpu_declaration(value: &GpuDeclaration) -> bool {
    if value.present {
        value.memory_bytes <= MAX_SAFE_INTEGER
            && (value.runtime.is_empty() || valid_token(&value.runtime))
    } else {
        value.memory_bytes == 0 && value.runtime.is_empty()
    }
}

pub(super) fn valid_owner(value: &OwnerWire) -> bool {
    valid_owner_part(&value.issuer)
        && valid_owner_part(&value.subject)
        && valid_owner_part(&value.tenant_id)
}

pub(super) fn valid_owner_part(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

pub(super) fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.chars().enumerate().all(|(index, character)| {
            character.is_ascii_alphanumeric()
                || (index > 0 && matches!(character, '.' | '_' | ':' | '-' | '+' | '/'))
        })
}

pub(super) fn valid_route_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.chars().enumerate().all(|(index, character)| {
            character.is_ascii_alphanumeric()
                || (index > 0 && matches!(character, '.' | '_' | '-' | '+'))
        })
}

pub(super) fn valid_idempotency_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | ':' | '-' | '+')
        })
}

pub(super) fn valid_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || ". _:+/-".replace(' ', "").contains(character)
        })
}

pub(super) fn valid_zones(values: &[String]) -> bool {
    values.len() <= 32
        && values.iter().all(|value| {
            !value.is_empty()
                && value.len() <= 64
                && value.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
                })
        })
        && values
            .iter()
            .enumerate()
            .all(|(index, value)| values[..index].iter().all(|previous| previous != value))
}

pub(super) fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

pub(super) fn same_owner(
    value: &forge_runtime_domain::ConversationOwner,
    expected: &OwnerWire,
) -> bool {
    value.issuer == expected.issuer
        && value.subject == expected.subject
        && value.tenant_id == expected.tenant_id
}
