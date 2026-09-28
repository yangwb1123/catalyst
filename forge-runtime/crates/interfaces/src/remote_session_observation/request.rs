use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use super::{
    CandidateDevices, MAX_DEVICES, RemoteError, has_exact_fields, placement, shape, string_field,
    validate_owner, validation,
};

pub(in super::super) fn validate_request(value: &Value) -> Result<(), RemoteError> {
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

    let placement = validate_placement(object, owner)?;
    let (placement_devices, candidates) = device_lists(object, placement)?;
    let candidate_devices = collect_candidates(candidates, owner)?;
    compare_placement(placement_devices, &candidate_devices)
}

fn validate_placement<'a>(
    object: &'a Map<String, Value>,
    owner: &Value,
) -> Result<&'a Map<String, Value>, RemoteError> {
    let placement = object
        .get("placement")
        .ok_or_else(|| RemoteError("remote session observation placement is missing".into()))?;
    placement::validate_request(placement)?;
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

    Ok(placement_object)
}

fn device_lists<'a>(
    object: &'a Map<String, Value>,
    placement_object: &'a Map<String, Value>,
) -> Result<(&'a Vec<Value>, &'a Vec<Value>), RemoteError> {
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

    Ok((placement_devices, candidates))
}

fn collect_candidates(
    candidates: &[Value],
    owner: &Value,
) -> Result<CandidateDevices, RemoteError> {
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
        let instance_id = candidate_instance(candidate, &mut instance_ids)?;
        let (device_id, device) = candidate_device(candidate, owner)?;
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

    Ok(candidate_devices)
}

fn candidate_instance<'a>(
    candidate: &'a Map<String, Value>,
    instance_ids: &mut BTreeSet<String>,
) -> Result<&'a str, RemoteError> {
    let instance_id = candidate
        .get("instance_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| RemoteError("remote session observation instance ID is invalid".into()))?;
    validation::validate_entity_id(instance_id, "Runner instance")?;
    if !instance_ids.insert(instance_id.to_owned()) {
        return Err(RemoteError(
            "remote session observation contains duplicate instance IDs".into(),
        ));
    }
    Ok(instance_id)
}

fn candidate_device<'a>(
    candidate: &'a Map<String, Value>,
    owner: &Value,
) -> Result<(&'a str, &'a Value), RemoteError> {
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
    Ok((device_id, device))
}

fn compare_placement(
    placement_devices: &[Value],
    candidate_devices: &CandidateDevices,
) -> Result<(), RemoteError> {
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
