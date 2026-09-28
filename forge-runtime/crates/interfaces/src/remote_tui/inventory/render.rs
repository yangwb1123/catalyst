use super::{RemoteError, io_error};
use serde_json::Value;
use std::io::Write;

pub(in super::super) fn write_remote_inventory_v2<W: Write>(
    value: &Value,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let evaluated_at_ms = number(value, "evaluated_at_ms", 0);
    let devices = value
        .get("devices")
        .and_then(Value::as_array)
        .ok_or_else(|| RemoteError("Forge API returned an invalid v2 device inventory".into()))?;
    writeln!(
        writer,
        "remote device inventory [forge.device-inventory-observation/v2] at {evaluated_at_ms} devices={}",
        devices.len()
    )
    .map_err(io_error)?;
    writeln!(
        writer,
        "declarations: owner_unverified={} inventory_unverified={}",
        flag(value, "owner_declaration_unverified", false),
        flag(value, "inventory_declarations_unverified", false),
    )
    .map_err(io_error)?;
    writeln!(
        writer,
        "authority: identity_verified=false heartbeat_persisted=false inventory_authoritative=false reservation_created={} execution_authorized={} dispatch_performed={}",
        flag(value, "reservation_created", true),
        flag(value, "execution_authorized", true),
        flag(value, "dispatch_performed", true),
    )
    .map_err(io_error)?;
    for candidate in devices {
        render_device_v2(candidate, writer)?;
    }
    Ok(())
}

pub(in super::super) fn write_remote_inventory<W: Write>(
    value: &Value,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let schema = text(
        value,
        "schema_version",
        "forge.device-inventory-observation/v1",
    );
    let evaluated_at_ms = number(value, "evaluated_at_ms", 0);
    writeln!(
        writer,
        "remote device inventory [{schema}] at {evaluated_at_ms}"
    )
    .map_err(io_error)?;
    writeln!(
        writer,
        "declarations: owner_unverified={} inventory_unverified={}",
        flag(value, "owner_declaration_unverified", false),
        flag(value, "inventory_declarations_unverified", false),
    )
    .map_err(io_error)?;
    writeln!(
        writer,
        "authority: identity_verified=false heartbeat_persisted=false inventory_authoritative=false reservation_created=false execution_authorized={} dispatch_performed={}",
        flag(value, "execution_authorized", true),
        flag(value, "dispatch_performed", true),
    )
    .map_err(io_error)?;
    let Some(devices) = value.get("devices").and_then(Value::as_array) else {
        return Err(RemoteError(
            "Forge API returned an invalid device inventory".into(),
        ));
    };
    for candidate in devices {
        render_device(candidate, writer)?;
    }
    Ok(())
}

fn render_device_v2<W: Write>(candidate: &Value, writer: &mut W) -> Result<(), RemoteError> {
    candidate
        .as_object()
        .ok_or_else(|| RemoteError("Forge API returned an invalid v2 device inventory".into()))?;
    let object = candidate;
    let device = object
        .get("device")
        .filter(|value| value.is_object())
        .ok_or_else(|| RemoteError("Forge API returned an invalid v2 device inventory".into()))?;
    let instance = text(object, "instance_id", "");
    let device_id = text(device, "device_id", "");
    let gpus = device
        .get("gpus")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    writeln!(
        writer,
        "{} / {}: revision={} generation={} heartbeat={} reservation={} cpu={} memory={} storage={} gpus={}",
        device_id,
        instance,
        number(object, "revision", 0),
        number(object, "generation", 0),
        number(object, "heartbeat_sequence", 0),
        text(device, "reservation_state", ""),
        number(device, "available_cpu_cores", 0),
        number(device, "available_memory_bytes", 0),
        number(device, "available_storage_bytes", 0),
        gpus,
    )
    .map_err(io_error)?;
    Ok(())
}

fn render_device<W: Write>(candidate: &Value, writer: &mut W) -> Result<(), RemoteError> {
    let Some(device) = candidate.get("device") else {
        return Err(RemoteError(
            "Forge API returned an invalid device inventory".into(),
        ));
    };
    writeln!(
        writer,
        "{} / {}: approval={} cordon={} liveness={} cpu={} memory={} storage={} gpu={} gpu_memory={} runtimes={}",
        text(device, "device_id", "invalid"),
        text(candidate, "instance_id", "invalid"),
        text(device, "approval_state", "invalid"),
        text(device, "cordon_state", "invalid"),
        text(device, "liveness", "invalid"),
        number(device, "available_cpu_cores", 0),
        number(device, "available_memory_bytes", 0),
        number(device, "available_storage_bytes", 0),
        device
            .get("gpu")
            .and_then(|gpu| gpu.get("present"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        device
            .get("gpu")
            .and_then(|gpu| gpu.get("memory_bytes"))
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        device
            .get("runtimes")
            .and_then(Value::as_array)
            .map(|runtimes| {
                runtimes
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default(),
    )
    .map_err(io_error)?;
    Ok(())
}

fn text<'a>(value: &'a Value, field: &str, fallback: &'a str) -> &'a str {
    value.get(field).and_then(Value::as_str).unwrap_or(fallback)
}
fn number(value: &Value, field: &str, fallback: u64) -> u64 {
    value.get(field).and_then(Value::as_u64).unwrap_or(fallback)
}
fn flag(value: &Value, field: &str, fallback: bool) -> bool {
    value
        .get(field)
        .and_then(Value::as_bool)
        .unwrap_or(fallback)
}
