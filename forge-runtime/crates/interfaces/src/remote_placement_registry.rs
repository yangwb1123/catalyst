//! Strict consumer for the owner-scoped registry placement preview candidate.
//!
//! The HTTP boundary accepts one requirements object and returns a v2 value
//! evaluation over the server's owner-scoped lifecycle registry.  This module
//! treats the response as an observation only: it never selects, reserves,
//! schedules, dispatches, or executes work.

use std::{
    collections::HashSet,
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::RemoteError;

const MAX_INPUT_BYTES: usize = 512 * 1024;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_DECISIONS: usize = 128;
const MAX_ARRAY_ITEMS: usize = 32;
const MAX_EXCLUSION_REASONS: usize = 64;
const MAX_TOKEN_BYTES: usize = 128;
const MAX_OWNER_PART_BYTES: usize = 512;
const MAX_GPU_MEMORY_BYTES: u64 = MAX_SAFE_INTEGER;
const SCHEMA_VERSION: &str = "forge.device-inventory-placement-evaluation/v2";
const EVALUATION_MODE: &str = "offline_static_only";
const SOURCE_SCHEMA_VERSION: &str = "forge.device-inventory-observation/v2";
const NOTICE: &str = "Every owner, state, timestamp, resource, GPU, reservation, residency, trust, sandbox, and concurrency value is an unverified caller declaration. This read-only comparison selects no target and grants no execution authority.";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    requirements: Requirements,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Requirements {
    os: String,
    architecture: String,
    min_cpu_cores: u32,
    min_memory_bytes: u64,
    min_storage_bytes: u64,
    runtime: String,
    gpu: GpuRequirement,
    data_residency_zones: Vec<String>,
    minimum_trust_zone: String,
    sandbox_floor: String,
    concurrency_slots: u16,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GpuRequirement {
    required: bool,
    min_memory_bytes: u64,
    runtime: String,
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
struct Decision {
    revision: u64,
    generation: u64,
    heartbeat_sequence: u64,
    device_id: String,
    instance_id: String,
    reservation_state: String,
    gpu_count: usize,
    available_gpu_memory_bytes: u64,
    matches_requirements: bool,
    exclusion_reasons: Vec<String>,
    owner_declaration_unverified: bool,
    device_attributes_unverified: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
struct Authority {
    identity_verified: bool,
    heartbeat_persisted: bool,
    inventory_authoritative: bool,
    placement_selected: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct PreviewResult {
    schema_version: String,
    evaluation_mode: String,
    source_schema_version: String,
    evaluation_owner: Owner,
    evaluated_at_ms: u64,
    notice: String,
    decisions: Vec<Decision>,
    eligible_candidate_count: usize,
    selected_device_id: Option<String>,
    selected_instance_id: Option<String>,
    authority: Authority,
}

pub(super) fn read_request(input: &str) -> Result<Value, RemoteError> {
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes).map_err(|_| {
        RemoteError("remote registry placement input contains duplicate JSON keys".into())
    })?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| RemoteError("remote registry placement input is invalid JSON".into()))?;
    validate_request(&value)?;
    Ok(value)
}

pub(super) fn read_tui_request(input: &str) -> Result<Value, RemoteError> {
    if input == "-" {
        return Err(RemoteError(
            "remote TUI registry placement preview requires a file path; '-' belongs to the standalone CLI"
                .into(),
        ));
    }
    read_request(input)
}

pub(super) fn validate_request(value: &Value) -> Result<(), RemoteError> {
    let object = value.as_object().ok_or_else(invalid_request)?;
    if !exact_fields(object, ["requirements"]) {
        return Err(invalid_request());
    }
    let request: Request = serde_json::from_value(value.clone()).map_err(|_| invalid_request())?;
    validate_requirements(&request.requirements).map_err(|()| invalid_request())
}

pub(super) fn validate_response(value: &Value) -> Result<PreviewResult, RemoteError> {
    let result: PreviewResult = serde_json::from_value(value.clone()).map_err(|_| {
        RemoteError("Forge API returned an invalid registry placement preview".into())
    })?;
    if result.schema_version != SCHEMA_VERSION
        || result.evaluation_mode != EVALUATION_MODE
        || result.source_schema_version != SOURCE_SCHEMA_VERSION
        || !valid_owner(&result.evaluation_owner)
        || result.evaluated_at_ms == 0
        || result.evaluated_at_ms > MAX_SAFE_INTEGER
        || result.notice != NOTICE
        || result.decisions.len() > MAX_DECISIONS
        || result.selected_device_id.is_some()
        || result.selected_instance_id.is_some()
        || result.authority != Authority::default()
        || result.eligible_candidate_count > result.decisions.len()
    {
        return Err(invalid_response());
    }

    validate_decisions(&result)?;
    Ok(result)
}

pub(super) fn render_human(result: &PreviewResult, writer: &mut impl Write) -> io::Result<()> {
    writeln!(
        writer,
        "registry placement preview [{}] owner={}/{} tenant={} at {} decisions={} eligible={} selected=none",
        result.schema_version,
        result.evaluation_owner.issuer,
        result.evaluation_owner.subject,
        result.evaluation_owner.tenant_id,
        result.evaluated_at_ms,
        result.decisions.len(),
        result.eligible_candidate_count
    )?;
    for decision in &result.decisions {
        if decision.matches_requirements {
            writeln!(
                writer,
                "{} / {}: matches revision={} generation={} heartbeat={} reservation={} gpus={} available_gpu_memory={}",
                decision.device_id,
                decision.instance_id,
                decision.revision,
                decision.generation,
                decision.heartbeat_sequence,
                decision.reservation_state,
                decision.gpu_count,
                decision.available_gpu_memory_bytes
            )?;
        } else {
            writeln!(
                writer,
                "{} / {}: excluded ({})",
                decision.device_id,
                decision.instance_id,
                decision.exclusion_reasons.join(",")
            )?;
        }
    }
    writeln!(
        writer,
        "preview_only=true authority: identity_verified=false heartbeat_persisted=false inventory_authoritative=false placement_selected=false reservation_created=false execution_authorized=false dispatch_performed=false"
    )
}

fn exact_fields<const N: usize>(object: &Map<String, Value>, fields: [&str; N]) -> bool {
    object.len() == N && fields.into_iter().all(|field| object.contains_key(field))
}

fn validate_requirements(value: &Requirements) -> Result<(), ()> {
    if !valid_token(&value.os)
        || !valid_token(&value.architecture)
        || value.min_cpu_cores == 0
        || value.min_memory_bytes == 0
        || value.min_memory_bytes > MAX_SAFE_INTEGER
        || value.min_storage_bytes == 0
        || value.min_storage_bytes > MAX_SAFE_INTEGER
        || !valid_token(&value.runtime)
        || !value.gpu.runtime.is_empty()
        || !matches!(
            value.minimum_trust_zone.as_str(),
            "untrusted" | "low" | "standard" | "high" | "restricted"
        )
        || !matches!(
            value.sandbox_floor.as_str(),
            "process" | "container" | "microvm"
        )
        || value.concurrency_slots == 0
        || value.data_residency_zones.is_empty()
        || value.data_residency_zones.len() > MAX_ARRAY_ITEMS
        || !sorted_unique(&value.data_residency_zones, valid_zone)
        || if value.gpu.required {
            value.gpu.min_memory_bytes > MAX_SAFE_INTEGER
        } else {
            value.gpu.min_memory_bytes != 0 || !value.gpu.runtime.is_empty()
        }
    {
        return Err(());
    }
    Ok(())
}

fn valid_owner(value: &Owner) -> bool {
    [&value.issuer, &value.subject, &value.tenant_id]
        .into_iter()
        .all(|part| {
            !part.is_empty()
                && part.len() <= MAX_OWNER_PART_BYTES
                && part.trim() == part
                && !part.chars().any(char::is_control)
        })
}

fn valid_identifier(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_TOKEN_BYTES {
        return false;
    }
    value.chars().enumerate().all(|(index, character)| {
        character.is_ascii_alphanumeric()
            || (index > 0 && matches!(character, '.' | '_' | ':' | '-'))
    })
}

fn valid_token(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_TOKEN_BYTES {
        return false;
    }
    value.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | ':' | '+' | '/' | '-')
    })
}

fn valid_zone(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
}

fn sorted_unique(values: &[String], valid: fn(&str) -> bool) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1]) && values.iter().all(|value| valid(value))
}

fn valid_counter(value: u64) -> bool {
    value > 0 && value <= MAX_SAFE_INTEGER
}

fn invalid_request() -> RemoteError {
    RemoteError("remote registry placement input is invalid".into())
}

fn invalid_response() -> RemoteError {
    RemoteError("Forge API returned an invalid registry placement preview".into())
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, RemoteError> {
    let mut bytes = Vec::with_capacity(MAX_INPUT_BYTES.min(64 * 1024));
    if input == "-" {
        io::stdin()
            .lock()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("could not read registry placement stdin".into()))?;
    } else {
        let path = Path::new(input);
        let metadata = std::fs::metadata(path)
            .map_err(|_| RemoteError("could not read registry placement input".into()))?;
        if !metadata.is_file() {
            return Err(RemoteError(
                "registry placement input must be a regular file".into(),
            ));
        }
        File::open(path)
            .map_err(|_| RemoteError("could not read registry placement input".into()))?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| RemoteError("could not read registry placement input".into()))?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(RemoteError(format!(
            "registry placement input exceeds {MAX_INPUT_BYTES} bytes"
        )));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request() -> Value {
        json!({
            "requirements": {
                "os": "linux", "architecture": "amd64", "min_cpu_cores": 1,
                "min_memory_bytes": 1, "min_storage_bytes": 1, "runtime": "oci",
                "gpu": {"required": false, "min_memory_bytes": 0, "runtime": ""},
                "data_residency_zones": ["us-west"], "minimum_trust_zone": "standard",
                "sandbox_floor": "container", "concurrency_slots": 1
            }
        })
    }

    fn response() -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "evaluation_mode": EVALUATION_MODE,
            "source_schema_version": SOURCE_SCHEMA_VERSION,
            "evaluation_owner": {"issuer":"https://id.example","subject":"user-a","tenant_id":"tenant-a"},
            "evaluated_at_ms": 1800000000000_u64,
            "notice": NOTICE,
            "decisions": [{
                "revision": 1, "generation": 1, "heartbeat_sequence": 1,
                "device_id": "device-a", "instance_id": "runner-a", "reservation_state": "none",
                "gpu_count": 0, "available_gpu_memory_bytes": 0,
                "matches_requirements": true, "exclusion_reasons": [],
                "owner_declaration_unverified": true, "device_attributes_unverified": true
            }],
            "eligible_candidate_count": 1,
            "selected_device_id": null, "selected_instance_id": null,
            "authority": Authority::default()
        })
    }

    #[test]
    fn request_requires_exact_requirements_shape() {
        validate_request(&request()).unwrap();
        let mut unknown = request();
        unknown["unexpected"] = json!(true);
        assert!(validate_request(&unknown).is_err());
        let mut owner = request();
        owner["owner"] = json!({"issuer":"https://id.example"});
        assert!(validate_request(&owner).is_err());
    }

    #[test]
    fn response_binds_v2_owner_time_decisions_and_authority() {
        let result = validate_response(&response()).unwrap();
        assert_eq!(result.evaluated_at_ms, 1800000000000);
        let mut foreign = response();
        foreign["evaluation_owner"]["subject"] = json!("other-user");
        assert!(validate_response(&foreign).is_ok());

        let mut authority = response();
        authority["authority"]["placement_selected"] = json!(true);
        assert!(validate_response(&authority).is_err());

        let mut order = response();
        order["decisions"][0]["exclusion_reasons"] = json!(["z", "a"]);
        order["decisions"][0]["matches_requirements"] = json!(false);
        assert!(validate_response(&order).is_err());
    }

    #[test]
    fn response_rejects_selected_target_or_count_drift() {
        let mut selected = response();
        selected["selected_device_id"] = json!("device-a");
        assert!(validate_response(&selected).is_err());
        let mut count = response();
        count["eligible_candidate_count"] = json!(0);
        assert!(validate_response(&count).is_err());
    }
}

fn validate_decisions(result: &PreviewResult) -> Result<(), RemoteError> {
    let mut previous: Option<(&str, &str)> = None;
    let mut devices = HashSet::with_capacity(result.decisions.len());
    let mut instances = HashSet::with_capacity(result.decisions.len());
    let mut eligible = 0;
    for decision in &result.decisions {
        let key = (decision.device_id.as_str(), decision.instance_id.as_str());
        if previous.is_some_and(|value| value >= key)
            || !devices.insert(decision.device_id.as_str())
            || !instances.insert(decision.instance_id.as_str())
            || !valid_counter(decision.revision)
            || !valid_counter(decision.generation)
            || !valid_counter(decision.heartbeat_sequence)
            || !valid_identifier(&decision.device_id)
            || !valid_identifier(&decision.instance_id)
            || !matches!(decision.reservation_state.as_str(), "none" | "reserved")
            || decision.gpu_count > MAX_ARRAY_ITEMS
            || decision.available_gpu_memory_bytes > MAX_GPU_MEMORY_BYTES
            || !decision.owner_declaration_unverified
            || !decision.device_attributes_unverified
            || decision.exclusion_reasons.len() > MAX_EXCLUSION_REASONS
            || decision
                .exclusion_reasons
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || decision
                .exclusion_reasons
                .iter()
                .any(|reason| !valid_token(reason))
            || decision.matches_requirements != decision.exclusion_reasons.is_empty()
        {
            return Err(invalid_response());
        }
        if decision.matches_requirements {
            eligible += 1;
        }
        previous = Some(key);
    }
    if eligible != result.eligible_candidate_count {
        return Err(invalid_response());
    }
    Ok(())
}
