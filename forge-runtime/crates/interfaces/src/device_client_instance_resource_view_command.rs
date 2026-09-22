//! Bounded Rust consumer for the composed client-instance/resource view.
//! This is a local metadata decoder only; it is not inventory authority,
//! scheduling, reservation, dispatch, or Runner access.

use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use serde::{Deserialize, Serialize};

use crate::{args::DeviceCommand, device_json_unique::reject_duplicate_keys};

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_INSTANCES: usize = 128;
const MAX_DEVICES: usize = 128;
const MAX_SESSION_IDS: usize = 128;
const MAX_OWNER_PART_BYTES: usize = 512;
const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_TOKEN_BYTES: usize = 128;
const MAX_TIMESTAMP_MS: i64 = 9_007_199_254_740_991;
const SCHEMA_VERSION: &str = "forge.client-instance-resource-view/v1";
const EVALUATION_MODE: &str = "owner_bound_instance_resource_view_only";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClientInstanceResourceViewOutput {
    schema_version: String,
    evaluation_mode: String,
    #[serde(rename = "owner_declaration")]
    owner: Owner,
    owner_declaration_unverified: bool,
    instances: Vec<Instance>,
    devices: Vec<Device>,
    device_attributes_unverified: bool,
    read_only: bool,
    authority: Authority,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Owner {
    issuer: String,
    subject: String,
    tenant_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Instance {
    instance_id: String,
    client_kind: String,
    session_ids: Vec<String>,
    observed_at_ms: i64,
    status: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Device {
    device_id: String,
    runner_instance_id: String,
    owner: Owner,
    revision: u64,
    generation: u64,
    heartbeat_sequence: u64,
    observed_at_ms: i64,
    approval_state: String,
    cordon_state: String,
    reservation_state: String,
    liveness: String,
    os: String,
    architecture: String,
    cpu_cores: u32,
    available_cpu_cores: u32,
    memory_bytes: u64,
    available_memory_bytes: u64,
    storage_bytes: u64,
    available_storage_bytes: u64,
    gpu_count: u32,
    available_gpu_memory_bytes: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Authority {
    owner_authenticated: bool,
    session_read_authorized: bool,
    prompt_write_authorized: bool,
    device_identity_verified: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
    audit_published: bool,
}

pub(crate) fn execute(
    command: &DeviceCommand,
) -> Result<ClientInstanceResourceViewOutput, Box<dyn Error>> {
    let DeviceCommand::ClientInstanceResourceViewPreview { input: input_path } = command else {
        return Err("device client-instance/resource-view preview command is required".into());
    };
    let bytes = read_bounded_input(input_path)?;
    reject_duplicate_keys(&bytes).map_err(|error| {
        format!("client-instance/resource-view input contains duplicate JSON keys: {error}")
    })?;
    let output: ClientInstanceResourceViewOutput = serde_json::from_slice(&bytes)
        .map_err(|error| format!("client-instance/resource-view input is invalid JSON: {error}"))?;
    validate_output(&output)?;
    Ok(output)
}

/// Validates the same strict observation shape for the authenticated
/// candidate reader. The response remains a metadata-only value and is never
/// promoted to inventory, scheduling, reservation, or execution authority.
pub(crate) fn validate_remote_response(value: &serde_json::Value) -> Result<(), Box<dyn Error>> {
    let output: ClientInstanceResourceViewOutput = serde_json::from_value(value.clone())
        .map_err(|error| format!("client-instance/resource-view response is invalid: {error}"))?;
    validate_output(&output)
}

pub(crate) fn write_output(
    output: &ClientInstanceResourceViewOutput,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    write_human_output(output, "offline", writer)
}

/// Renders a validated response returned by the explicitly enabled remote
/// candidate. Resource values remain unverified display metadata.
pub(crate) fn write_remote_output(
    value: &serde_json::Value,
    writer: &mut impl Write,
) -> Result<(), Box<dyn Error>> {
    let output: ClientInstanceResourceViewOutput = serde_json::from_value(value.clone())
        .map_err(|error| format!("client-instance/resource-view response is invalid: {error}"))?;
    validate_output(&output)?;
    write_human_output(&output, "remote", writer)?;
    Ok(())
}

fn write_human_output(
    output: &ClientInstanceResourceViewOutput,
    source: &str,
    writer: &mut impl Write,
) -> io::Result<()> {
    writeln!(
        writer,
        "{source} client-instance/resource-view [{}]",
        output.schema_version
    )?;
    writeln!(
        writer,
        "owner={}/{}/{} instances={} devices={} read_only=true device_attributes_unverified=true",
        output.owner.issuer,
        output.owner.subject,
        output.owner.tenant_id,
        output.instances.len(),
        output.devices.len()
    )?;
    for instance in &output.instances {
        writeln!(
            writer,
            "instance {}: client_kind={} status={} observed_at_ms={} sessions={}",
            instance.instance_id,
            instance.client_kind,
            instance.status,
            instance.observed_at_ms,
            instance.session_ids.join(",")
        )?;
    }
    for device in &output.devices {
        writeln!(
            writer,
            "device {}: runner={} revision={} generation={} heartbeat={} liveness={} reservation={} cpu={}/{} memory={}/{} gpu_count={} gpu_memory_available={}",
            device.device_id,
            device.runner_instance_id,
            device.revision,
            device.generation,
            device.heartbeat_sequence,
            device.liveness,
            device.reservation_state,
            device.available_cpu_cores,
            device.cpu_cores,
            device.available_memory_bytes,
            device.memory_bytes,
            device.gpu_count,
            device.available_gpu_memory_bytes
        )?;
    }
    writeln!(
        writer,
        "authority: owner_authenticated=false session_read_authorized=false prompt_write_authorized=false device_identity_verified=false reservation_created=false execution_authorized=false dispatch_performed=false audit_published=false"
    )
}

fn validate_output(output: &ClientInstanceResourceViewOutput) -> Result<(), Box<dyn Error>> {
    if output.schema_version != SCHEMA_VERSION
        || output.evaluation_mode != EVALUATION_MODE
        || !valid_owner(&output.owner)
        || !output.owner_declaration_unverified
        || !output.device_attributes_unverified
        || !output.read_only
        || !authority_is_false(output.authority)
        || output.instances.len() > MAX_INSTANCES
        || output.devices.len() > MAX_DEVICES
    {
        return Err("client-instance/resource-view observation is invalid".into());
    }
    for (index, instance) in output.instances.iter().enumerate() {
        if !valid_instance(instance)
            || (index > 0 && output.instances[index - 1].instance_id >= instance.instance_id)
        {
            return Err(format!("client instance {:?} is invalid", instance.instance_id).into());
        }
    }
    let mut devices = std::collections::HashSet::new();
    let mut runners = std::collections::HashSet::new();
    for (index, device) in output.devices.iter().enumerate() {
        if !valid_device(device, &output.owner)
            || (index > 0
                && (output.devices[index - 1].device_id > device.device_id
                    || (output.devices[index - 1].device_id == device.device_id
                        && output.devices[index - 1].runner_instance_id
                            >= device.runner_instance_id)))
        {
            return Err(format!("device {:?} is invalid", device.device_id).into());
        }
        if !devices.insert(device.device_id.as_str())
            || !runners.insert(device.runner_instance_id.as_str())
        {
            return Err("client-instance/resource-view contains duplicate device or Runner".into());
        }
    }
    Ok(())
}

fn valid_instance(instance: &Instance) -> bool {
    valid_identifier(&instance.instance_id)
        && matches!(
            instance.client_kind.as_str(),
            "cli" | "tui" | "web" | "app" | "mobile"
        )
        && instance.session_ids.len() <= MAX_SESSION_IDS
        && instance.observed_at_ms > 0
        && instance.observed_at_ms <= MAX_TIMESTAMP_MS
        && matches!(
            instance.status.as_str(),
            "active" | "idle" | "offline" | "unknown"
        )
        && instance
            .session_ids
            .iter()
            .enumerate()
            .all(|(index, session_id)| {
                valid_identifier(session_id)
                    && (index == 0 || instance.session_ids[index - 1] < *session_id)
            })
}

fn valid_device(device: &Device, owner: &Owner) -> bool {
    valid_identifier(&device.device_id)
        && valid_identifier(&device.runner_instance_id)
        && device.owner == *owner
        && safe_counter(device.revision)
        && safe_counter(device.generation)
        && safe_counter(device.heartbeat_sequence)
        && device.observed_at_ms > 0
        && device.observed_at_ms <= MAX_TIMESTAMP_MS
        && matches!(
            device.approval_state.as_str(),
            "approved" | "pending" | "revoked" | "unknown"
        )
        && matches!(
            device.cordon_state.as_str(),
            "clear" | "cordoned" | "unknown"
        )
        && matches!(
            device.reservation_state.as_str(),
            "none" | "reserved" | "unknown"
        )
        && matches!(device.liveness.as_str(), "online" | "offline" | "unknown")
        && valid_token(&device.os)
        && valid_token(&device.architecture)
        && device.available_cpu_cores <= device.cpu_cores
        && safe_counter(device.memory_bytes)
        && device.available_memory_bytes <= device.memory_bytes
        && safe_counter(device.storage_bytes)
        && device.available_storage_bytes <= device.storage_bytes
        && safe_counter(device.available_gpu_memory_bytes)
}

fn safe_counter(value: u64) -> bool {
    value <= MAX_TIMESTAMP_MS as u64
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
            || (index > 0 && matches!(character, '.' | '_' | ':' | '-' | '+' | '/'))
    })
}

fn valid_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_TOKEN_BYTES
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '.' | '_' | ':' | '-' | '+' | '/')
        })
}

fn authority_is_false(authority: Authority) -> bool {
    !authority.owner_authenticated
        && !authority.session_read_authorized
        && !authority.prompt_write_authorized
        && !authority.device_identity_verified
        && !authority.reservation_created
        && !authority.execution_authorized
        && !authority.dispatch_performed
        && !authority.audit_published
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::new();
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
        return Err("client-instance/resource-view input exceeds 2 MiB".into());
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "device_client_instance_resource_view_command_tests.rs"]
mod tests;
