use std::io::Write;

use serde_json::Value;

use super::super::state::io_error;
use super::super::{RemoteClient, RemoteError};

/// Reads the candidate lifecycle registry only after an explicit TUI command.
/// It is intentionally absent from startup and `sync`.
pub(super) async fn show<W: Write>(
    client: &RemoteClient,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if !matches!(argument.trim(), "show" | "read") {
        writeln!(
            writer,
            "Use lifecycle-registry show. This explicit candidate read is GET-only."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    let value = match client.read_lifecycle_registry().await {
        Ok(value) => value,
        Err(error) => {
            writeln!(writer, "Remote lifecycle-registry request failed: {error}")
                .map_err(io_error)?;
            return Ok(());
        }
    };
    write_output(&value, writer)
}

fn write_output<W: Write>(value: &Value, writer: &mut W) -> Result<(), RemoteError> {
    let owner = value
        .get("owner")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RemoteError("Forge API returned an invalid lifecycle registry candidate".into())
        })?;
    let states = value
        .get("states")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RemoteError("Forge API returned an invalid lifecycle registry candidate".into())
        })?;
    writeln!(
        writer,
        "remote lifecycle registry [{}] owner={}/{} tenant={} states={}",
        value
            .get("schema_version")
            .and_then(Value::as_str)
            .unwrap_or("unknown"),
        owner.get("issuer").and_then(Value::as_str).unwrap_or(""),
        owner.get("subject").and_then(Value::as_str).unwrap_or(""),
        owner.get("tenant_id").and_then(Value::as_str).unwrap_or(""),
        states.len()
    )
    .map_err(io_error)?;
    for state in states {
        write_state(state, writer)?;
    }
    writeln!(
        writer,
        "  observation_only: owner_authenticated=false inventory_authoritative=false reservation_created=false execution_authorized=false dispatch_performed=false"
    )
    .map_err(io_error)
}

fn write_state<W: Write>(state: &Value, writer: &mut W) -> Result<(), RemoteError> {
    let device = state
        .get("device")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RemoteError("Forge API returned an invalid lifecycle registry candidate".into())
        })?;
    let heartbeat = state
        .get("heartbeat")
        .and_then(Value::as_object)
        .and_then(|value| value.get("instance"))
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RemoteError("Forge API returned an invalid lifecycle registry candidate".into())
        })?;
    let capabilities = heartbeat.get("capabilities").and_then(Value::as_object);
    writeln!(
        writer,
        "  device={} instance={} revision={} os={} arch={} cpu={}/{} memory={}/{} storage={}/{}",
        device
            .get("device_id")
            .and_then(Value::as_str)
            .unwrap_or(""),
        heartbeat
            .get("instance_id")
            .and_then(Value::as_str)
            .unwrap_or(""),
        state
            .get("revision")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        capability_text(capabilities, "os"),
        capability_text(capabilities, "architecture"),
        capability_number(capabilities, "available_cpu_cores"),
        capability_number(capabilities, "cpu_cores"),
        capability_number(capabilities, "available_memory_bytes"),
        capability_number(capabilities, "memory_bytes"),
        capability_number(capabilities, "available_storage_bytes"),
        capability_number(capabilities, "storage_bytes"),
    )
    .map_err(io_error)?;
    Ok(())
}

fn capability_number(capabilities: Option<&serde_json::Map<String, Value>>, field: &str) -> u64 {
    capabilities
        .and_then(|value| value.get(field))
        .and_then(Value::as_u64)
        .unwrap_or_default()
}
fn capability_text<'a>(
    capabilities: Option<&'a serde_json::Map<String, Value>>,
    field: &str,
) -> &'a str {
    capabilities
        .and_then(|value| value.get(field))
        .and_then(Value::as_str)
        .unwrap_or("")
}
