use std::{
    collections::BTreeSet,
    fs::File,
    io::{self, Read},
    path::Path,
};

use serde::Deserialize;
use serde_json::Value;

use super::RemoteError;
#[path = "remote_placement_model.rs"]
mod model;
use model::{
    DeviceResult, MAX_DEVICES, MAX_EXCLUSION_REASONS, MAX_SAFE_INTEGER, Owner, decode_request,
    evaluate, valid_device_id, valid_token,
};

const RESULT_SCHEMA: &str = "forge.device-placement-dry-run-result/v1";
const EVALUATION_MODE: &str = "offline_static_only";
const MAX_INPUT_BYTES: usize = 512 * 1024;
const NOTICE: &str = "All owner, approval, liveness, resource, residency, trust, and sandbox attributes are unverified caller declarations. This offline comparison selects no target and grants no execution authority.";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "Published wire predicates are separate booleans; changing their representation would change the protocol."
)]
pub(super) struct PreviewResult {
    schema_version: String,
    evaluation_mode: String,
    evaluated_at_ms: i64,
    owner_declaration: Owner,
    owner_declaration_unverified: bool,
    device_attributes_unverified: bool,
    notice: String,
    device_results: Vec<DeviceResult>,
    execution_authorized: bool,
    reservation_created: bool,
    dispatch_performed: bool,
}

pub(super) fn read_request(input: &str) -> Result<Value, RemoteError> {
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes)
        .map_err(|_| RemoteError("remote placement input contains duplicate JSON keys".into()))?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| RemoteError("remote placement input is invalid JSON".into()))?;
    validate_request(&value)?;
    Ok(value)
}

pub(super) fn read_tui_request(input: &str) -> Result<Value, RemoteError> {
    if input == "-" {
        return Err(RemoteError(
            "remote TUI placement preview requires a file path; '-' belongs to the standalone CLI"
                .into(),
        ));
    }
    read_request(input)
}

pub(super) fn validate_request(value: &Value) -> Result<(), RemoteError> {
    decode_request(value).map(|_| ()).map_err(RemoteError)
}

pub(super) fn validate_response(
    value: &Value,
    request: &Value,
) -> Result<PreviewResult, RemoteError> {
    let request = decode_request(request).map_err(RemoteError)?;
    let result: PreviewResult = serde_json::from_value(value.clone())
        .map_err(|_| RemoteError("Forge API returned an invalid placement preview".into()))?;
    if result.schema_version != RESULT_SCHEMA
        || result.evaluation_mode != EVALUATION_MODE
        || result.evaluated_at_ms <= 0
        || result.evaluated_at_ms > MAX_SAFE_INTEGER
        || result.evaluated_at_ms != request.evaluated_at_ms
        || result.owner_declaration != request.owner
        || !result.owner_declaration_unverified
        || !result.device_attributes_unverified
        || result.notice != NOTICE
        || result.execution_authorized
        || result.reservation_created
        || result.dispatch_performed
    {
        return Err(RemoteError(
            "Forge API returned a placement preview with invalid authority or binding".into(),
        ));
    }
    if result.device_results.len() > MAX_DEVICES {
        return Err(RemoteError(
            "Forge API returned too many placement results".into(),
        ));
    }
    let requested_ids = request
        .devices
        .iter()
        .map(|device| device.device_id.clone())
        .collect::<BTreeSet<_>>();
    let result_ids = placement_result_ids(&result)?;
    if result_ids != requested_ids {
        return Err(RemoteError(
            "Forge API returned placement results for a different device set".into(),
        ));
    }
    if result.device_results != evaluate(&request) {
        return Err(RemoteError(
            "Forge API returned placement results that differ from the caller declaration".into(),
        ));
    }
    Ok(result)
}

pub(super) fn render_human(result: &PreviewResult, writer: &mut impl io::Write) -> io::Result<()> {
    writeln!(
        writer,
        "offline placement preview [{}] at {}",
        result.schema_version, result.evaluated_at_ms
    )?;
    writeln!(
        writer,
        "authority: owner_declaration_unverified=true device_attributes_unverified=true execution_authorized=false reservation_created=false dispatch_performed=false"
    )?;
    for device in &result.device_results {
        if device.matches_requirements {
            writeln!(writer, "{}: matches", device.device_id)?;
        } else {
            writeln!(
                writer,
                "{}: excluded ({})",
                device.device_id,
                device.exclusion_reasons.join(",")
            )?;
        }
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
            .map_err(|_| RemoteError("could not read remote placement stdin".into()))?;
    } else {
        let path = Path::new(input);
        let metadata = std::fs::metadata(path)
            .map_err(|_| RemoteError("could not read remote placement input".into()))?;
        if !metadata.is_file() {
            return Err(RemoteError(
                "remote placement input must be a regular file".into(),
            ));
        }
        File::open(path)
            .map_err(|_| RemoteError("could not read remote placement input".into()))?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("could not read remote placement input".into()))?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(RemoteError(format!(
            "remote placement input exceeds {MAX_INPUT_BYTES} bytes"
        )));
    }
    Ok(bytes)
}

fn placement_result_ids(result: &PreviewResult) -> Result<BTreeSet<String>, RemoteError> {
    let mut result_ids = BTreeSet::new();
    let mut previous_id: Option<&str> = None;
    for device in &result.device_results {
        if !valid_device_id(&device.device_id)
            || !device.attributes_unverified
            || device.matches_requirements != device.exclusion_reasons.is_empty()
            || device.exclusion_reasons.len() > MAX_EXCLUSION_REASONS
            || previous_id.is_some_and(|previous| previous >= device.device_id.as_str())
            || device
                .exclusion_reasons
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || device
                .exclusion_reasons
                .iter()
                .any(|reason| !valid_token(reason))
        {
            return Err(RemoteError(
                "Forge API returned an invalid placement result ordering".into(),
            ));
        }
        if !result_ids.insert(device.device_id.clone()) {
            return Err(RemoteError(
                "Forge API returned duplicate placement result IDs".into(),
            ));
        }
        previous_id = Some(device.device_id.as_str());
    }
    Ok(result_ids)
}
