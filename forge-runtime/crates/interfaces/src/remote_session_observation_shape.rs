use serde_json::{Map, Value};

use super::{RemoteError, has_exact_fields, validate_owner};

const MAX_ARRAY_ITEMS: usize = 32;

/// Validates the complete nested placement shape used by the authenticated
/// session-observation route. The standalone placement preview intentionally
/// keeps its older permissive request parser for compatibility; this helper
/// keeps the session-bound client contract aligned with the Go route's strict
/// decoder before any network request is sent.
pub(super) fn validate_complete_placement(value: &Value) -> Result<(), RemoteError> {
    let object = value
        .as_object()
        .ok_or_else(|| RemoteError("remote session observation placement is invalid".into()))?;
    if !has_exact_fields(
        object,
        [
            "schema_version",
            "evaluated_at_ms",
            "owner",
            "max_snapshot_age_ms",
            "requirements",
            "devices",
        ],
    ) || object.get("schema_version").and_then(Value::as_str)
        != Some("forge.device-placement-dry-run/v1")
    {
        return Err(RemoteError(
            "remote session observation placement has an invalid shape".into(),
        ));
    }
    required_i64(object, "evaluated_at_ms")?;
    required_i64(object, "max_snapshot_age_ms")?;
    validate_owner(required_value(object, "owner")?)?;
    validate_requirements(required_value(object, "requirements")?)?;

    let devices = required_array(object, "devices")?;
    if devices.len() > 128 {
        return Err(RemoteError(
            "remote session observation placement has too many devices".into(),
        ));
    }
    for device in devices {
        validate_device(device)?;
    }
    Ok(())
}

fn validate_requirements(value: &Value) -> Result<(), RemoteError> {
    let object = value
        .as_object()
        .ok_or_else(|| RemoteError("remote session observation requirements are invalid".into()))?;
    if !has_exact_fields(
        object,
        [
            "os",
            "architecture",
            "min_cpu_cores",
            "min_memory_bytes",
            "min_storage_bytes",
            "runtime",
            "gpu",
            "data_residency_zones",
            "minimum_trust_zone",
            "sandbox_floor",
            "concurrency_slots",
        ],
    ) {
        return Err(RemoteError(
            "remote session observation requirements have an invalid shape".into(),
        ));
    }
    required_string(object, "os")?;
    required_string(object, "architecture")?;
    required_unsigned(object, "min_cpu_cores")?;
    required_unsigned(object, "min_memory_bytes")?;
    required_unsigned(object, "min_storage_bytes")?;
    required_string(object, "runtime")?;
    validate_gpu_requirement(required_value(object, "gpu")?)?;
    required_string_array(object, "data_residency_zones")?;
    required_string(object, "minimum_trust_zone")?;
    required_string(object, "sandbox_floor")?;
    required_unsigned(object, "concurrency_slots")?;
    Ok(())
}

fn validate_gpu_requirement(value: &Value) -> Result<(), RemoteError> {
    let object = value.as_object().ok_or_else(|| {
        RemoteError("remote session observation GPU requirement is invalid".into())
    })?;
    if !has_exact_fields(object, ["required", "min_memory_bytes", "runtime"]) {
        return Err(RemoteError(
            "remote session observation GPU requirement has an invalid shape".into(),
        ));
    }
    required_bool(object, "required")?;
    required_unsigned(object, "min_memory_bytes")?;
    required_string(object, "runtime")?;
    Ok(())
}

fn validate_device(value: &Value) -> Result<(), RemoteError> {
    let object = value.as_object().ok_or_else(|| {
        RemoteError("remote session observation device declaration is invalid".into())
    })?;
    if !has_exact_fields(
        object,
        [
            "device_id",
            "owner",
            "approval_state",
            "cordon_state",
            "liveness",
            "snapshot_observed_at_ms",
            "lease_expires_at_ms",
            "os",
            "architecture",
            "available_cpu_cores",
            "available_memory_bytes",
            "available_storage_bytes",
            "runtimes",
            "gpu",
            "data_residency_zones",
            "trust_zone",
            "sandbox_levels",
            "concurrency_limit",
            "active_concurrency",
        ],
    ) {
        return Err(RemoteError(
            "remote session observation device declaration has an invalid shape".into(),
        ));
    }
    validate_device_fields(object)
}

fn validate_device_fields(object: &Map<String, Value>) -> Result<(), RemoteError> {
    required_string(object, "device_id")?;
    validate_owner(required_value(object, "owner")?)?;
    required_string(object, "approval_state")?;
    required_string(object, "cordon_state")?;
    required_string(object, "liveness")?;
    required_i64(object, "snapshot_observed_at_ms")?;
    required_i64(object, "lease_expires_at_ms")?;
    required_string(object, "os")?;
    required_string(object, "architecture")?;
    required_unsigned(object, "available_cpu_cores")?;
    required_unsigned(object, "available_memory_bytes")?;
    required_unsigned(object, "available_storage_bytes")?;
    required_string_array(object, "runtimes")?;
    validate_gpu_declaration(required_value(object, "gpu")?)?;
    required_string_array(object, "data_residency_zones")?;
    required_string(object, "trust_zone")?;
    required_string_array(object, "sandbox_levels")?;
    required_unsigned(object, "concurrency_limit")?;
    required_unsigned(object, "active_concurrency")?;
    Ok(())
}

fn validate_gpu_declaration(value: &Value) -> Result<(), RemoteError> {
    let object = value.as_object().ok_or_else(|| {
        RemoteError("remote session observation GPU declaration is invalid".into())
    })?;
    if !has_exact_fields(object, ["present", "memory_bytes", "runtime"]) {
        return Err(RemoteError(
            "remote session observation GPU declaration has an invalid shape".into(),
        ));
    }
    required_bool(object, "present")?;
    required_unsigned(object, "memory_bytes")?;
    required_string(object, "runtime")?;
    Ok(())
}

fn required_value<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a Value, RemoteError> {
    object
        .get(field)
        .ok_or_else(|| RemoteError(format!("remote session observation {field} is missing")))
}

fn required_string(object: &Map<String, Value>, field: &str) -> Result<(), RemoteError> {
    if required_value(object, field)?.as_str().is_none() {
        return Err(RemoteError(format!(
            "remote session observation {field} is invalid"
        )));
    }
    Ok(())
}

fn required_bool(object: &Map<String, Value>, field: &str) -> Result<(), RemoteError> {
    if required_value(object, field)?.as_bool().is_none() {
        return Err(RemoteError(format!(
            "remote session observation {field} is invalid"
        )));
    }
    Ok(())
}

fn required_i64(object: &Map<String, Value>, field: &str) -> Result<(), RemoteError> {
    if required_value(object, field)?.as_i64().is_none() {
        return Err(RemoteError(format!(
            "remote session observation {field} is invalid"
        )));
    }
    Ok(())
}

fn required_unsigned(object: &Map<String, Value>, field: &str) -> Result<(), RemoteError> {
    if required_value(object, field)?.as_u64().is_none() {
        return Err(RemoteError(format!(
            "remote session observation {field} is invalid"
        )));
    }
    Ok(())
}

fn required_array<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a Vec<Value>, RemoteError> {
    let array = required_value(object, field)?
        .as_array()
        .ok_or_else(|| RemoteError(format!("remote session observation {field} is invalid")))?;
    if array.len() > MAX_ARRAY_ITEMS {
        return Err(RemoteError(format!(
            "remote session observation {field} has too many items"
        )));
    }
    Ok(array)
}

fn required_string_array(object: &Map<String, Value>, field: &str) -> Result<(), RemoteError> {
    if required_array(object, field)?
        .iter()
        .any(|value| value.as_str().is_none())
    {
        return Err(RemoteError(format!(
            "remote session observation {field} is invalid"
        )));
    }
    Ok(())
}
