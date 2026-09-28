//! Strict reader for the candidate lifecycle-registry HTTP envelope.
//!
//! This module is deliberately a value decoder.  It does not enroll a device,
//! accept a heartbeat, publish inventory authority, select a target, or write
//! the registry.  The authenticated owner is established by Forge Core; the
//! client only checks that the returned owner and every nested value are
//! structurally and semantically self-consistent.

use std::collections::HashSet;

use serde::Deserialize;
use serde_json::Value;

use super::RemoteError;

pub(super) const SCHEMA_VERSION: &str = "forge.device-enrollment-heartbeat-lifecycle-file-set/v1";
const MAX_STATES: usize = 128;
const MAX_GPU_ITEMS: usize = 32;
const MAX_RUNTIME_ITEMS: usize = 64;
const MAX_OWNER_PART_BYTES: usize = 512;
const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_RUNTIME_NAME_BYTES: usize = 64;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MIN_CAPABILITY_LEASE_TTL_MS: u64 = 1_000;
const MAX_CAPABILITY_LEASE_TTL_MS: u64 = 600_000;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct Envelope {
    schema_version: String,
    owner: Owner,
    states: Vec<State>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct Owner {
    issuer: String,
    subject: String,
    tenant_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct State {
    revision: u64,
    owner: Owner,
    device: DeviceBinding,
    heartbeat: PersistedHeartbeat,
    inventory: PersistedInventory,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct DeviceBinding {
    device_id: String,
    owner: Owner,
    key_id: String,
    public_key_sha256: String,
    approval_state: String,
    credential_state: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct PersistedHeartbeat {
    revision: u64,
    instance: HeartbeatInstance,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct HeartbeatInstance {
    device_id: String,
    instance_id: String,
    generation: u64,
    heartbeat_sequence: u64,
    server_observed_at_ms: u64,
    capability_lease_expires_at_ms: u64,
    capabilities: CapabilitySnapshot,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct CapabilitySnapshot {
    os: String,
    architecture: String,
    cpu_cores: u32,
    available_cpu_cores: u32,
    memory_bytes: u64,
    available_memory_bytes: u64,
    storage_bytes: u64,
    available_storage_bytes: u64,
    gpus: Vec<GpuCapability>,
    runtimes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct GpuCapability {
    id: String,
    vendor: String,
    memory_bytes: u64,
    available_memory_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct PersistedInventory {
    revision: u64,
    device: InventoryDevice,
    runner: InventoryRunner,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct InventoryDevice {
    device_id: String,
    owner: SnapshotOwner,
    approval_state: String,
    cordon_state: String,
    reservation_state: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct InventoryRunner {
    device_id: String,
    instance_id: String,
    generation: u64,
    heartbeat_sequence: u64,
    server_observed_at_ms: u64,
    capability_lease_expires_at_ms: u64,
    liveness: String,
    capabilities: CapabilitySnapshot,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct SnapshotOwner {
    issuer: String,
    subject: String,
    tenant_id: String,
}

pub(super) fn validate_response(value: &Value) -> Result<(), RemoteError> {
    let envelope: Envelope = serde_json::from_value(value.clone()).map_err(|_| invalid())?;
    if envelope.schema_version != SCHEMA_VERSION
        || envelope.states.len() > MAX_STATES
        || !valid_owner(&envelope.owner)
    {
        return Err(invalid());
    }

    let mut previous_key: Option<(&str, &str)> = None;
    let mut devices = HashSet::with_capacity(envelope.states.len());
    let mut instances = HashSet::with_capacity(envelope.states.len());
    for state in &envelope.states {
        validate_state(state, &envelope.owner)?;
        let device = state.device.device_id.as_str();
        let instance = state.heartbeat.instance.instance_id.as_str();
        let key = (device, instance);
        if previous_key.is_some_and(|previous| previous >= key)
            || !devices.insert(device)
            || !instances.insert(instance)
        {
            return Err(invalid());
        }
        previous_key = Some(key);
    }
    Ok(())
}

fn validate_state(state: &State, owner: &Owner) -> Result<(), RemoteError> {
    if state.revision == 0
        || state.revision > MAX_SAFE_INTEGER
        || state.owner != *owner
        || state.device.owner != *owner
        || !valid_identifier(&state.device.device_id)
        || !valid_identifier(&state.device.key_id)
        || !valid_digest(&state.device.public_key_sha256)
        || !matches!(
            state.device.approval_state.as_str(),
            "pending" | "approved" | "revoked"
        )
        || !matches!(
            state.device.credential_state.as_str(),
            "active" | "expired" | "revoked"
        )
        || state.heartbeat.revision != state.revision
        || state.inventory.revision != state.revision
    {
        return Err(invalid());
    }
    let instance = &state.heartbeat.instance;
    if instance.device_id != state.device.device_id
        || !valid_identifier(&instance.instance_id)
        || !safe_counter(instance.generation)
        || !safe_counter(instance.heartbeat_sequence)
        || instance.server_observed_at_ms > MAX_SAFE_INTEGER
        || instance.capability_lease_expires_at_ms > MAX_SAFE_INTEGER
        || instance.capability_lease_expires_at_ms < instance.server_observed_at_ms
        || !matches!(
            instance
                .capability_lease_expires_at_ms
                .checked_sub(instance.server_observed_at_ms),
            Some(MIN_CAPABILITY_LEASE_TTL_MS..=MAX_CAPABILITY_LEASE_TTL_MS)
        )
    {
        return Err(invalid());
    }
    validate_capabilities(&instance.capabilities)?;
    validate_inventory_binding(state, owner)?;
    Ok(())
}

fn validate_capabilities(capabilities: &CapabilitySnapshot) -> Result<(), RemoteError> {
    if !valid_tag(&capabilities.os)
        || !valid_tag(&capabilities.architecture)
        || capabilities.cpu_cores == 0
        || capabilities.cpu_cores > 4_096
        || capabilities.available_cpu_cores > capabilities.cpu_cores
        || capabilities.memory_bytes == 0
        || capabilities.available_memory_bytes > capabilities.memory_bytes
        || capabilities.storage_bytes == 0
        || capabilities.available_storage_bytes > capabilities.storage_bytes
        || capabilities.memory_bytes > MAX_SAFE_INTEGER
        || capabilities.available_memory_bytes > MAX_SAFE_INTEGER
        || capabilities.storage_bytes > MAX_SAFE_INTEGER
        || capabilities.available_storage_bytes > MAX_SAFE_INTEGER
        || capabilities.gpus.len() > MAX_GPU_ITEMS
        || capabilities.runtimes.len() > MAX_RUNTIME_ITEMS
        || !capabilities
            .runtimes
            .iter()
            .all(|runtime| valid_tag(runtime))
        || !sorted_unique_strings(&capabilities.runtimes)
    {
        return Err(invalid());
    }
    let mut previous_gpu = None;
    for gpu in &capabilities.gpus {
        if !valid_identifier(&gpu.id)
            || !valid_label(&gpu.vendor)
            || gpu.memory_bytes == 0
            || gpu.available_memory_bytes > gpu.memory_bytes
            || gpu.memory_bytes > MAX_SAFE_INTEGER
            || gpu.available_memory_bytes > MAX_SAFE_INTEGER
            || previous_gpu.is_some_and(|previous| previous >= gpu.id.as_str())
        {
            return Err(invalid());
        }
        previous_gpu = Some(gpu.id.as_str());
    }
    Ok(())
}

fn snapshot_owner(owner: &Owner) -> SnapshotOwner {
    SnapshotOwner {
        issuer: owner.issuer.clone(),
        subject: owner.subject.clone(),
        tenant_id: owner.tenant_id.clone(),
    }
}

fn valid_owner(owner: &Owner) -> bool {
    valid_owner_part(&owner.issuer)
        && valid_owner_part(&owner.subject)
        && valid_owner_part(&owner.tenant_id)
}

fn valid_owner_part(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_OWNER_PART_BYTES
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn valid_identifier(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_IDENTIFIER_BYTES {
        return false;
    }
    value.chars().enumerate().all(|(index, character)| {
        character.is_ascii_alphanumeric()
            || (index > 0 && matches!(character, '.' | '_' | ':' | '-'))
    })
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_tag(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_RUNTIME_NAME_BYTES
        && value == value.to_ascii_lowercase()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | ':' | '-' | '+')
        })
}

fn valid_label(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && trimmed.len() <= MAX_RUNTIME_NAME_BYTES
        && !trimmed.chars().any(char::is_control)
}

fn safe_counter(value: u64) -> bool {
    value > 0 && value <= MAX_SAFE_INTEGER
}

fn sorted_unique_strings(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn invalid() -> RemoteError {
    RemoteError("Forge API returned an invalid lifecycle registry candidate".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn empty_owner_scoped_envelope_is_valid() {
        validate_response(&json!({
            "schema_version": SCHEMA_VERSION,
            "owner": {"issuer":"https://id.example","subject":"user-a","tenant_id":"tenant-a"},
            "states": []
        }))
        .unwrap();
    }

    #[test]
    fn valid_state_preserves_the_joined_device_heartbeat_and_inventory_image() {
        let owner = json!({
            "issuer": "https://id.example",
            "subject": "user-a",
            "tenant_id": "tenant-a"
        });
        let capabilities = json!({
            "os": "linux",
            "architecture": "amd64",
            "cpu_cores": 8,
            "available_cpu_cores": 7,
            "memory_bytes": 17179869184u64,
            "available_memory_bytes": 8589934592u64,
            "storage_bytes": 107374182400u64,
            "available_storage_bytes": 53687091200u64,
            "gpus": [{
                "id": "gpu-a",
                "vendor": "NVIDIA",
                "memory_bytes": 8589934592u64,
                "available_memory_bytes": 4294967296u64
            }],
            "runtimes": ["oci", "python"]
        });
        let heartbeat_instance = json!({
            "device_id": "device-a",
            "instance_id": "runner-a",
            "generation": 1,
            "heartbeat_sequence": 1,
            "server_observed_at_ms": 110000,
            "capability_lease_expires_at_ms": 170000,
            "capabilities": capabilities
        });
        let state = joined_state(&owner, &heartbeat_instance, &capabilities);
        validate_response(&json!({
            "schema_version": SCHEMA_VERSION,
            "owner": {"issuer":"https://id.example","subject":"user-a","tenant_id":"tenant-a"},
            "states": [state]
        }))
        .unwrap();
    }

    fn joined_state(owner: &Value, heartbeat_instance: &Value, capabilities: &Value) -> Value {
        json!({
            "revision": 1,
            "owner": owner,
            "device": {
                "device_id": "device-a",
                "owner": {"issuer":"https://id.example","subject":"user-a","tenant_id":"tenant-a"},
                "key_id": "key-a",
                "public_key_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "approval_state": "approved",
                "credential_state": "active"
            },
            "heartbeat": {"revision": 1, "instance": heartbeat_instance},
            "inventory": {
                "revision": 1,
                "device": {
                    "device_id": "device-a",
                    "owner": {"issuer":"https://id.example","subject":"user-a","tenant_id":"tenant-a"},
                    "approval_state": "approved",
                    "cordon_state": "clear",
                    "reservation_state": "none"
                },
                "runner": {
                    "device_id": "device-a",
                    "instance_id": "runner-a",
                    "generation": 1,
                    "heartbeat_sequence": 1,
                    "server_observed_at_ms": 110000,
                    "capability_lease_expires_at_ms": 170000,
                    "liveness": "online",
                    "capabilities": capabilities
                }
            }
        })
    }

    #[test]
    fn envelope_rejects_unknown_fields_foreign_owner_and_unsorted_states() {
        let owner =
            json!({"issuer":"https://id.example","subject":"user-a","tenant_id":"tenant-a"});
        let mut value = json!({
            "schema_version": SCHEMA_VERSION,
            "owner": owner,
            "states": []
        });
        value["unexpected"] = json!(true);
        assert!(validate_response(&value).is_err());

        let foreign =
            json!({"issuer":"https://id.example","subject":"other","tenant_id":"tenant-a"});
        let mut value = json!({
            "schema_version": SCHEMA_VERSION,
            "owner": {"issuer":"https://id.example","subject":"user-a","tenant_id":"tenant-a"},
            "states": []
        });
        value["owner"] = foreign;
        assert!(validate_response(&value).is_ok());

        value["schema_version"] = json!("forge.other/v1");
        assert!(validate_response(&value).is_err());
    }
}

fn validate_inventory_binding(state: &State, owner: &Owner) -> Result<(), RemoteError> {
    let instance = &state.heartbeat.instance;
    let inventory = &state.inventory;
    if inventory.device.device_id != state.device.device_id
        || inventory.device.owner != snapshot_owner(owner)
        || inventory.device.approval_state != state.device.approval_state
        || !matches!(
            inventory.device.approval_state.as_str(),
            "pending" | "approved" | "revoked"
        )
        || !matches!(inventory.device.cordon_state.as_str(), "clear" | "cordoned")
        || !matches!(
            inventory.device.reservation_state.as_str(),
            "none" | "reserved"
        )
        || inventory.runner.device_id != instance.device_id
        || inventory.runner.instance_id != instance.instance_id
        || inventory.runner.generation != instance.generation
        || inventory.runner.heartbeat_sequence != instance.heartbeat_sequence
        || inventory.runner.server_observed_at_ms != instance.server_observed_at_ms
        || inventory.runner.capability_lease_expires_at_ms
            != instance.capability_lease_expires_at_ms
        || inventory.runner.capabilities != instance.capabilities
        || !matches!(inventory.runner.liveness.as_str(), "online" | "offline")
        || !safe_counter(inventory.runner.generation)
        || !safe_counter(inventory.runner.heartbeat_sequence)
        || inventory.runner.server_observed_at_ms > MAX_SAFE_INTEGER
        || inventory.runner.capability_lease_expires_at_ms > MAX_SAFE_INTEGER
    {
        return Err(invalid());
    }
    Ok(())
}
