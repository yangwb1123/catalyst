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
            device.get("device_id").and_then(Value::as_str).unwrap_or(""),
            heartbeat
                .get("instance_id")
                .and_then(Value::as_str)
                .unwrap_or(""),
            state.get("revision").and_then(Value::as_u64).unwrap_or_default(),
            capabilities
                .and_then(|value| value.get("os"))
                .and_then(Value::as_str)
                .unwrap_or(""),
            capabilities
                .and_then(|value| value.get("architecture"))
                .and_then(Value::as_str)
                .unwrap_or(""),
            capabilities
                .and_then(|value| value.get("available_cpu_cores"))
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            capabilities
                .and_then(|value| value.get("cpu_cores"))
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            capabilities
                .and_then(|value| value.get("available_memory_bytes"))
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            capabilities
                .and_then(|value| value.get("memory_bytes"))
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            capabilities
                .and_then(|value| value.get("available_storage_bytes"))
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            capabilities
                .and_then(|value| value.get("storage_bytes"))
                .and_then(Value::as_u64)
                .unwrap_or_default(),
        )
        .map_err(io_error)?;
    }
    writeln!(
        writer,
        "  observation_only: owner_authenticated=false inventory_authoritative=false reservation_created=false execution_authorized=false dispatch_performed=false"
    )
    .map_err(io_error)
}
