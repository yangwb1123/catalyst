use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{self, Read},
    path::Path,
};

use serde_json::{Map, Value};

use super::{RemoteError, validation};

#[path = "remote_session_observation_shape.rs"]
mod shape;

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_DEVICES: usize = 128;

/// Reads the caller supplied request used by the authenticated session
/// observation route. The request contains no credentials, leases, or target
/// selection; it is only a bounded declaration for a pure preview.
pub(super) fn read_request(input: &str) -> Result<Value, RemoteError> {
    let bytes = read_bounded_input(input)?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| RemoteError("remote session observation input is invalid JSON".into()))?;
    validate_request(&value)?;
    Ok(value)
}

pub(super) fn read_tui_request(input: &str) -> Result<Value, RemoteError> {
    if input == "-" {
        return Err(RemoteError(
            "remote TUI session observation preview requires a file path; '-' belongs to the standalone CLI"
                .into(),
        ));
    }
    read_request(input)
}

pub(super) fn validate_request(value: &Value) -> Result<(), RemoteError> {
    let object = value.as_object().ok_or_else(|| {
        RemoteError("remote session observation input must be a JSON object".into())
    })?;
    if !has_exact_fields(
        object,
        [
            "owner",
            "conversation_id",
            "run_id",
            "placement",
            "candidates",
        ],
    ) {
        return Err(RemoteError(
            "remote session observation input has an invalid shape".into(),
        ));
    }
    let owner = object
        .get("owner")
        .ok_or_else(|| RemoteError("remote session observation owner is missing".into()))?;
    validate_owner(owner)?;
    let conversation_id = string_field(object, "conversation_id")?;
    let run_id = string_field(object, "run_id")?;
    validation::validate_conversation_id(conversation_id)?;
    validation::validate_entity_id(run_id, "Run")?;

    let placement = object
        .get("placement")
        .ok_or_else(|| RemoteError("remote session observation placement is missing".into()))?;
    super::placement::validate_request(placement)?;
    let placement_object = placement
        .as_object()
        .ok_or_else(|| RemoteError("remote session observation placement is invalid".into()))?;
    shape::validate_complete_placement(placement)?;
    if placement_object.get("owner") != Some(owner) {
        return Err(RemoteError(
            "remote session observation placement owner does not match the request owner".into(),
        ));
    }
    let evaluated_at_ms = placement_object
        .get("evaluated_at_ms")
        .and_then(Value::as_i64)
        .filter(|value| *value > 0)
        .ok_or_else(|| RemoteError("remote session observation timestamp is invalid".into()))?;
    if evaluated_at_ms > 9_007_199_254_740_991 {
        return Err(RemoteError(
            "remote session observation timestamp exceeds the safe integer range".into(),
        ));
    }

    let placement_devices = placement_object
        .get("devices")
        .and_then(Value::as_array)
        .ok_or_else(|| RemoteError("remote session observation devices are invalid".into()))?;
    if placement_devices.len() > MAX_DEVICES {
        return Err(RemoteError(
            "remote session observation has too many devices".into(),
        ));
    }
    let candidates = object
        .get("candidates")
        .and_then(Value::as_array)
        .ok_or_else(|| RemoteError("remote session observation candidates are invalid".into()))?;
    if candidates.len() > MAX_DEVICES || candidates.len() != placement_devices.len() {
        return Err(RemoteError(
            "remote session observation candidate and placement device counts differ".into(),
        ));
    }

    let mut candidate_devices = BTreeMap::new();
    let mut instance_ids = BTreeSet::new();
    for candidate in candidates {
        let candidate = candidate
            .as_object()
            .ok_or_else(|| RemoteError("remote session observation candidate is invalid".into()))?;
        if !has_exact_fields(candidate, ["instance_id", "device"]) {
            return Err(RemoteError(
                "remote session observation candidate has an invalid shape".into(),
            ));
        }
        let instance_id = candidate
            .get("instance_id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                RemoteError("remote session observation instance ID is invalid".into())
            })?;
        validation::validate_entity_id(instance_id, "Runner instance")?;
        if !instance_ids.insert(instance_id.to_owned()) {
            return Err(RemoteError(
                "remote session observation contains duplicate instance IDs".into(),
            ));
        }
        let device = candidate
            .get("device")
            .ok_or_else(|| RemoteError("remote session observation device is missing".into()))?;
        let device_object = device
            .as_object()
            .ok_or_else(|| RemoteError("remote session observation device is invalid".into()))?;
        let device_id = device_object
            .get("device_id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| RemoteError("remote session observation device ID is invalid".into()))?;
        validation::validate_entity_id(device_id, "Device")?;
        if device_object.get("owner") != Some(owner) {
            return Err(RemoteError(
                "remote session observation device owner does not match the request owner".into(),
            ));
        }
        if candidate_devices
            .insert(
                device_id.to_owned(),
                (instance_id.to_owned(), device.clone()),
            )
            .is_some()
        {
            return Err(RemoteError(
                "remote session observation contains duplicate device IDs".into(),
            ));
        }
    }

    let mut placement_ids = BTreeSet::new();
    for device in placement_devices {
        let device_object = device.as_object().ok_or_else(|| {
            RemoteError("remote session observation placement device is invalid".into())
        })?;
        let device_id = device_object
            .get("device_id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| RemoteError("remote session observation device ID is invalid".into()))?;
        if !placement_ids.insert(device_id.to_owned()) {
            return Err(RemoteError(
                "remote session observation contains duplicate placement device IDs".into(),
            ));
        }
        let Some((_, candidate_device)) = candidate_devices.get(device_id) else {
            return Err(RemoteError(
                "remote session observation placement devices differ from candidates".into(),
            ));
        };
        if candidate_device != device {
            return Err(RemoteError(
                "remote session observation placement device declaration differs from candidate"
                    .into(),
            ));
        }
    }
    if placement_ids != candidate_devices.keys().cloned().collect() {
        return Err(RemoteError(
            "remote session observation placement devices differ from candidates".into(),
        ));
    }
    Ok(())
}

pub(super) fn validate_response(value: &Value, request: &Value) -> Result<(), RemoteError> {
    validate_request(request)?;
    let response = value.as_object().ok_or_else(|| {
        RemoteError("Forge API returned an invalid session device observation".into())
    })?;
    if !has_exact_fields(
        response,
        [
            "schema_version",
            "evaluation_mode",
            "owner",
            "conversation_id",
            "run_id",
            "evaluated_at_ms",
            "owner_declaration_unverified",
            "inventory",
            "placement_observation",
            "resource_summary",
            "selected_device_id",
            "selected_instance_id",
            "authority",
        ],
    ) {
        return Err(RemoteError(
            "Forge API returned an invalid session device observation shape".into(),
        ));
    }
    crate::device_resource_summary_command::session_observation::validate_value(value)
        .map_err(|error| RemoteError(error.to_string()))?;

    let request_object = request.as_object().ok_or_else(|| {
        RemoteError("remote session observation input must be a JSON object".into())
    })?;
    for field in ["owner", "conversation_id", "run_id"] {
        if response.get(field) != request_object.get(field) {
            return Err(RemoteError(format!(
                "Forge API returned a session observation with a different {field}"
            )));
        }
    }
    let request_timestamp = request_object["placement"]
        .get("evaluated_at_ms")
        .and_then(Value::as_i64)
        .ok_or_else(|| RemoteError("remote session observation timestamp is invalid".into()))?;
    if response.get("evaluated_at_ms").and_then(Value::as_i64) != Some(request_timestamp) {
        return Err(RemoteError(
            "Forge API returned a session observation with a different evaluation time".into(),
        ));
    }

    let candidates = request_object["candidates"]
        .as_array()
        .ok_or_else(|| RemoteError("remote session observation candidates are invalid".into()))?;
    let inventory = response["inventory"]
        .get("devices")
        .and_then(Value::as_array)
        .ok_or_else(|| RemoteError("Forge API returned invalid inventory devices".into()))?;
    let mut expected = BTreeMap::new();
    for candidate in candidates {
        let candidate_object = candidate
            .as_object()
            .ok_or_else(|| RemoteError("remote session observation candidate is invalid".into()))?;
        let instance_id = candidate_object["instance_id"].as_str().ok_or_else(|| {
            RemoteError("remote session observation instance ID is invalid".into())
        })?;
        let device = candidate_object["device"].clone();
        let device_id = device["device_id"]
            .as_str()
            .ok_or_else(|| RemoteError("remote session observation device ID is invalid".into()))?;
        expected.insert(device_id.to_owned(), (instance_id.to_owned(), device));
    }
    let mut actual = BTreeMap::new();
    for candidate in inventory {
        let candidate_object = candidate.as_object().ok_or_else(|| {
            RemoteError("Forge API returned an invalid inventory candidate".into())
        })?;
        let instance_id = candidate_object["instance_id"].as_str().ok_or_else(|| {
            RemoteError("Forge API returned an invalid inventory instance ID".into())
        })?;
        let device = candidate_object
            .get("device")
            .ok_or_else(|| RemoteError("Forge API returned an invalid inventory device".into()))?;
        let device_id = device["device_id"].as_str().ok_or_else(|| {
            RemoteError("Forge API returned an invalid inventory device ID".into())
        })?;
        actual.insert(
            device_id.to_owned(),
            (instance_id.to_owned(), device.clone()),
        );
    }
    if actual != expected {
        return Err(RemoteError(
            "Forge API returned inventory declarations different from the request".into(),
        ));
    }
    Ok(())
}

pub(super) fn render_human(value: &Value, writer: &mut impl io::Write) -> io::Result<()> {
    let summary = &value["resource_summary"];
    let inventory = value["inventory"]["devices"]
        .as_array()
        .map(|devices| {
            devices
                .iter()
                .filter_map(|candidate| {
                    let device = candidate["device"].get("device_id")?.as_str()?;
                    Some((device, candidate))
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    writeln!(
        writer,
        "offline session device observation [{}] at {}",
        value["schema_version"].as_str().unwrap_or("unknown"),
        value["evaluated_at_ms"].as_i64().unwrap_or_default()
    )?;
    writeln!(
        writer,
        "owner={} conversation={} run={}",
        value["owner"]["subject"].as_str().unwrap_or("unknown"),
        value["conversation_id"].as_str().unwrap_or("unknown"),
        value["run_id"].as_str().unwrap_or("unknown")
    )?;
    writeln!(
        writer,
        "resources: devices={} runner_instances={} cpu={} memory={} storage={} gpus={} gpu_memory={} eligible_devices={} eligible_instances={}",
        summary["device_count"].as_u64().unwrap_or_default(),
        summary["runner_instance_count"]
            .as_u64()
            .unwrap_or_default(),
        summary["available_cpu_cores"].as_u64().unwrap_or_default(),
        summary["available_memory_bytes"]
            .as_u64()
            .unwrap_or_default(),
        summary["available_storage_bytes"]
            .as_u64()
            .unwrap_or_default(),
        summary["available_gpu_count"].as_u64().unwrap_or_default(),
        summary["available_gpu_memory_bytes"]
            .as_u64()
            .unwrap_or_default(),
        summary["eligible_device_count"]
            .as_u64()
            .unwrap_or_default(),
        summary["eligible_instance_count"]
            .as_u64()
            .unwrap_or_default()
    )?;
    writeln!(
        writer,
        "authority: identity_verified=false heartbeat_persisted=false inventory_authoritative=false reservation_created=false execution_authorized=false dispatch_performed=false"
    )?;
    for decision in value["placement_observation"]["decisions"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let device = decision["device_id"].as_str().unwrap_or("unknown");
        let instance = decision["instance_id"].as_str().unwrap_or("unknown");
        let resources = inventory
            .get(device)
            .map(|candidate| {
                let declaration = &candidate["device"];
                format!(
                    " resources=cpu:{} memory:{} storage:{} gpu:{} gpu_memory:{}",
                    declaration["available_cpu_cores"]
                        .as_u64()
                        .unwrap_or_default(),
                    declaration["available_memory_bytes"]
                        .as_u64()
                        .unwrap_or_default(),
                    declaration["available_storage_bytes"]
                        .as_u64()
                        .unwrap_or_default(),
                    declaration["gpu"]["present"].as_bool().unwrap_or(false),
                    declaration["gpu"]["memory_bytes"]
                        .as_u64()
                        .unwrap_or_default(),
                )
            })
            .unwrap_or_default();
        if decision["matches_requirements"].as_bool().unwrap_or(false) {
            writeln!(writer, "{device}/{instance}: matches{resources}")?;
        } else {
            let reasons = decision["exclusion_reasons"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(",");
            writeln!(
                writer,
                "{device}/{instance}: excluded ({reasons}){resources}"
            )?;
        }
    }
    Ok(())
}

fn has_exact_fields<const N: usize>(object: &Map<String, Value>, fields: [&str; N]) -> bool {
    object.len() == N && fields.iter().all(|field| object.contains_key(*field))
}

fn string_field<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RemoteError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| RemoteError(format!("remote session observation {field} is invalid")))
}

fn validate_owner(value: &Value) -> Result<(), RemoteError> {
    let object = value
        .as_object()
        .ok_or_else(|| RemoteError("remote session observation owner is invalid".into()))?;
    if !has_exact_fields(object, ["issuer", "subject", "tenant_id"])
        || ["issuer", "subject", "tenant_id"]
            .iter()
            .any(|field| object[*field].as_str().is_none_or(str::is_empty))
    {
        return Err(RemoteError(
            "remote session observation owner is invalid".into(),
        ));
    }
    Ok(())
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, RemoteError> {
    let mut bytes = Vec::with_capacity(MAX_INPUT_BYTES.min(64 * 1024));
    if input == "-" {
        io::stdin()
            .lock()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("could not read remote session observation stdin".into()))?;
    } else {
        let path = Path::new(input);
        let metadata = std::fs::metadata(path)
            .map_err(|_| RemoteError("could not read remote session observation input".into()))?;
        if !metadata.is_file() {
            return Err(RemoteError(
                "remote session observation input must be a regular file".into(),
            ));
        }
        File::open(path)
            .map_err(|_| RemoteError("could not read remote session observation input".into()))?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("could not read remote session observation input".into()))?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(RemoteError(format!(
            "remote session observation input exceeds {MAX_INPUT_BYTES} bytes"
        )));
    }
    Ok(bytes)
}
