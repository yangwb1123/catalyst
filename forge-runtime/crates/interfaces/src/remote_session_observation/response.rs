use std::collections::BTreeMap;

use serde_json::{Map, Value};

use super::{CandidateDevices, RemoteError, has_exact_fields, validate_request};

pub(in super::super) fn validate_response(
    value: &Value,
    request: &Value,
) -> Result<(), RemoteError> {
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

    let request_object = validate_binding(response, request)?;
    validate_inventory(response, request_object)
}

fn validate_binding<'a>(
    response: &Map<String, Value>,
    request: &'a Value,
) -> Result<&'a Map<String, Value>, RemoteError> {
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

    Ok(request_object)
}

fn validate_inventory(
    response: &Map<String, Value>,
    request_object: &Map<String, Value>,
) -> Result<(), RemoteError> {
    let candidates = request_object["candidates"]
        .as_array()
        .ok_or_else(|| RemoteError("remote session observation candidates are invalid".into()))?;
    let inventory = response["inventory"]
        .get("devices")
        .and_then(Value::as_array)
        .ok_or_else(|| RemoteError("Forge API returned invalid inventory devices".into()))?;
    let expected = expected_inventory(candidates)?;
    let actual = actual_inventory(inventory)?;
    if actual != expected {
        return Err(RemoteError(
            "Forge API returned inventory declarations different from the request".into(),
        ));
    }
    Ok(())
}

fn expected_inventory(candidates: &[Value]) -> Result<CandidateDevices, RemoteError> {
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
    Ok(expected)
}

fn actual_inventory(inventory: &[Value]) -> Result<CandidateDevices, RemoteError> {
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
    Ok(actual)
}
