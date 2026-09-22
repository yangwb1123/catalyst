//! Strict authenticated consumer for the Run/Attempt/lease dispatch preflight
//! candidate.
//!
//! The server endpoint is a private, test-only value adapter.  This module
//! keeps that boundary explicit: the input is a bounded caller declaration,
//! the POST is sent once, and the response is decoded as the strict
//! metadata-only domain observation.  No target is selected and no authority
//! bit is accepted from the transport.

use std::{
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use forge_runtime_domain::execution::{
    lease::LeaseGrant,
    run_attempt_lease_dispatch_preflight::{RunAttemptLeaseDispatchPreflightObservation, decode},
};
use serde::Deserialize;
use serde_json::Value;

use super::RemoteError;

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_CANDIDATES: usize = 128;
const REQUEST_SCHEMA_VERSION: &str = "forge.device-placement-dry-run/v1";
const INTENT_SCHEMA_VERSION: &str = "forge.runner-execution-intent/v1";
const INTENT_EVALUATION_MODE: &str = "pure_runner_binding_only";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct OwnerWire {
    issuer: String,
    subject: String,
    tenant_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PreflightRequest {
    owner: OwnerWire,
    conversation_id: String,
    run_id: String,
    run_status: String,
    dispatch_plan: DispatchPlanRequest,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DispatchPlanRequest {
    attempt_state: String,
    placement_request: PlacementRequest,
    runner_execution_intent: IntentObservation,
    lease: LeaseGrant,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IntentObservation {
    schema_version: String,
    evaluation_mode: String,
    owner: OwnerWire,
    conversation_id: String,
    prompt_id: String,
    run_id: String,
    attempt_id: String,
    command_id: String,
    target_id: String,
    command_sha256: String,
    idempotency_key: String,
    prompt_run_binding_valid: bool,
    runner_command_binding_valid: bool,
    preview_only: bool,
    selected_target_id: Option<String>,
    authority: IntentAuthority,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct IntentAuthority {
    device_identity_verified: bool,
    command_persisted: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
    audit_published: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlacementRequest {
    schema_version: String,
    evaluated_at_ms: i64,
    owner: OwnerWire,
    max_snapshot_age_ms: i64,
    requirements: PlacementRequirements,
    devices: Vec<PlacementDevice>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlacementRequirements {
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

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlacementDevice {
    device_id: String,
    owner: OwnerWire,
    approval_state: String,
    cordon_state: String,
    liveness: String,
    snapshot_observed_at_ms: i64,
    lease_expires_at_ms: i64,
    os: String,
    architecture: String,
    available_cpu_cores: u32,
    available_memory_bytes: u64,
    available_storage_bytes: u64,
    runtimes: Vec<String>,
    gpu: GpuDeclaration,
    data_residency_zones: Vec<String>,
    trust_zone: String,
    sandbox_levels: Vec<String>,
    concurrency_limit: u16,
    active_concurrency: u16,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GpuDeclaration {
    present: bool,
    memory_bytes: u64,
    runtime: String,
}

/// Reads and validates one bounded caller declaration. `-` is supported by
/// the standalone CLI; the TUI deliberately requires a regular file.
pub(super) fn read_request(input: &str) -> Result<Value, RemoteError> {
    let bytes = read_bounded_input(input)?;
    crate::device_json_unique::reject_duplicate_keys(&bytes).map_err(|_| {
        RemoteError("remote Run/Attempt/lease preflight input has duplicate JSON keys".into())
    })?;
    let request: PreflightRequest = serde_json::from_slice(&bytes)
        .map_err(|_| RemoteError("remote Run/Attempt/lease preflight input is invalid".into()))?;
    validate_request(&request)?;
    serde_json::from_slice(&bytes)
        .map_err(|_| RemoteError("remote Run/Attempt/lease preflight input is invalid JSON".into()))
}

/// Alias used by the TUI to make the file-only input policy visible at its
/// call site while retaining the same strict decoder as the CLI.
pub(super) fn read_tui_request(input: &str) -> Result<Value, RemoteError> {
    if input == "-" {
        return Err(RemoteError(
            "remote Run/Attempt/lease preflight TUI input must be a file".into(),
        ));
    }
    read_request(input)
}

pub(super) fn conversation_and_run(request: &Value) -> Result<(String, String), RemoteError> {
    let request = decode_request(request)?;
    Ok((request.conversation_id, request.run_id))
}

pub(super) fn validate_response(
    value: &Value,
    request: &Value,
    conversation_id: &str,
    run_id: &str,
) -> Result<(), RemoteError> {
    let request = decode_request(request)?;
    if request.conversation_id != conversation_id || request.run_id != run_id {
        return Err(RemoteError(
            "remote Run/Attempt/lease preflight request does not match the URL path".into(),
        ));
    }
    let encoded = serde_json::to_vec(value).map_err(|_| {
        RemoteError("Forge API returned an invalid Run/Attempt/lease preflight".into())
    })?;
    let observation: RunAttemptLeaseDispatchPreflightObservation =
        decode(&encoded).map_err(|_| {
            RemoteError("Forge API returned an invalid Run/Attempt/lease preflight".into())
        })?;
    let plan = &request.dispatch_plan;
    let placement = &plan.placement_request;
    let intent = &plan.runner_execution_intent;
    let expected_lease_active = plan.lease.is_active(placement.evaluated_at_ms as u64);
    if !same_owner(&observation.owner, &request.owner)
        || observation.conversation_id != conversation_id
        || observation.run_id != run_id
        || observation.run_status != request.run_status
        || observation.attempt_id != intent.attempt_id
        || observation.attempt_state != plan.attempt_state
        || observation.command_id != intent.command_id
        || observation.intent_target_id != intent.target_id
        || observation.lease_epoch != plan.lease.epoch
        || observation.lease_active != expected_lease_active
        || observation.evaluated_at_ms != placement.evaluated_at_ms as u64
        || observation.candidate_count as usize != placement.devices.len()
        || !observation.preview_only
        || observation.selected_target_id.is_some()
    {
        return Err(RemoteError(
            "Forge API returned a Run/Attempt/lease preflight with a different binding".into(),
        ));
    }
    Ok(())
}

pub(super) fn render_human(value: &Value, writer: &mut impl Write) -> io::Result<()> {
    let bytes = serde_json::to_vec(value).expect("validated preflight observation is serializable");
    let observation: RunAttemptLeaseDispatchPreflightObservation =
        decode(&bytes).expect("validated preflight observation");
    writeln!(
        writer,
        "Run/Attempt/lease dispatch preflight [{}]",
        observation.schema_version
    )?;
    writeln!(
        writer,
        "owner={} conversation={} run={} status={} attempt={} state={} command={} target={} lease_epoch={}",
        observation.owner.subject,
        observation.conversation_id,
        observation.run_id,
        observation.run_status,
        observation.attempt_id,
        observation.attempt_state,
        observation.command_id,
        observation.intent_target_id,
        observation.lease_epoch
    )?;
    writeln!(
        writer,
        "candidates={} ready={} declarative_preflight_ready={} selected_target_id=null evaluated_at_ms={}",
        observation.candidate_count,
        observation.declarative_ready_count,
        observation.declarative_preflight_ready,
        observation.evaluated_at_ms
    )?;
    writeln!(
        writer,
        "rejection_reasons={:?}",
        observation.rejection_reasons
    )?;
    writeln!(
        writer,
        "preview_only=true authority: identity_verified=false run_authoritative=false attempt_persisted=false lease_issued=false reservation_created=false execution_authorized=false dispatch_performed=false audit_published=false"
    )
}

fn decode_request(value: &Value) -> Result<PreflightRequest, RemoteError> {
    let encoded = serde_json::to_vec(value)
        .map_err(|_| RemoteError("remote Run/Attempt/lease preflight input is invalid".into()))?;
    let request: PreflightRequest = serde_json::from_slice(&encoded)
        .map_err(|_| RemoteError("remote Run/Attempt/lease preflight input is invalid".into()))?;
    validate_request(&request)?;
    Ok(request)
}

fn validate_request(request: &PreflightRequest) -> Result<(), RemoteError> {
    if !valid_owner(&request.owner)
        || !valid_route_identifier(&request.conversation_id)
        || !valid_route_identifier(&request.run_id)
        || !matches!(
            request.run_status.as_str(),
            "nonterminal" | "completed" | "cancelled" | "limit_exceeded" | "failed"
        )
    {
        return Err(invalid_request());
    }
    let plan = &request.dispatch_plan;
    if !matches!(
        plan.attempt_state.as_str(),
        "requested"
            | "accepted"
            | "starting"
            | "running"
            | "interrupted"
            | "completed"
            | "failed"
            | "uncertain"
    ) || !valid_placement(&plan.placement_request, &request.owner)
        || !valid_intent(plan, request)
        || plan.placement_request.owner != request.owner
        || plan.lease.validate().is_err()
        || plan.lease.epoch > MAX_SAFE_INTEGER
        || plan.lease.issued_at_ms > MAX_SAFE_INTEGER
        || plan.lease.expires_at_ms > MAX_SAFE_INTEGER
        || plan.lease.attempt_id != plan.runner_execution_intent.attempt_id
        || plan.lease.target_id != plan.runner_execution_intent.target_id
    {
        return Err(invalid_request());
    }
    Ok(())
}

fn valid_intent(plan: &DispatchPlanRequest, request: &PreflightRequest) -> bool {
    let intent = &plan.runner_execution_intent;
    valid_owner(&intent.owner)
        && intent.owner == request.owner
        && intent.schema_version == INTENT_SCHEMA_VERSION
        && intent.evaluation_mode == INTENT_EVALUATION_MODE
        && intent.conversation_id == request.conversation_id
        && valid_identifier(&intent.prompt_id)
        && intent.run_id == request.run_id
        && valid_identifier(&intent.attempt_id)
        && valid_identifier(&intent.command_id)
        && valid_identifier(&intent.target_id)
        && valid_digest(&intent.command_sha256)
        && valid_idempotency_key(&intent.idempotency_key)
        && intent.prompt_run_binding_valid
        && intent.runner_command_binding_valid
        && intent.preview_only
        && intent.selected_target_id.is_none()
        && intent.authority == IntentAuthority::default()
}

fn valid_placement(value: &PlacementRequest, expected_owner: &OwnerWire) -> bool {
    value.schema_version == REQUEST_SCHEMA_VERSION
        && value.evaluated_at_ms > 0
        && (value.evaluated_at_ms as u64) <= MAX_SAFE_INTEGER
        && value.max_snapshot_age_ms > 0
        && (value.max_snapshot_age_ms as u64) <= 86_400_000
        && valid_owner(&value.owner)
        && value.owner == *expected_owner
        && valid_requirements(&value.requirements)
        && value.devices.len() <= MAX_CANDIDATES
        && value
            .devices
            .iter()
            .all(|device| valid_device(device, expected_owner))
        && value.devices.iter().enumerate().all(|(index, device)| {
            value.devices[..index]
                .iter()
                .all(|previous| previous.device_id != device.device_id)
        })
}

fn valid_requirements(value: &PlacementRequirements) -> bool {
    valid_token(&value.os)
        && valid_token(&value.architecture)
        && value.min_cpu_cores > 0
        && value.min_memory_bytes > 0
        && value.min_memory_bytes <= MAX_SAFE_INTEGER
        && value.min_storage_bytes > 0
        && value.min_storage_bytes <= MAX_SAFE_INTEGER
        && valid_token(&value.runtime)
        && valid_zones(&value.data_residency_zones)
        && matches!(
            value.minimum_trust_zone.as_str(),
            "untrusted" | "low" | "standard" | "high" | "restricted"
        )
        && matches!(
            value.sandbox_floor.as_str(),
            "process" | "container" | "microvm"
        )
        && value.concurrency_slots > 0
        && valid_gpu_requirement(&value.gpu)
}

fn valid_gpu_requirement(value: &GpuRequirement) -> bool {
    if value.required {
        value.min_memory_bytes <= MAX_SAFE_INTEGER
            && (value.runtime.is_empty() || valid_token(&value.runtime))
    } else {
        value.min_memory_bytes == 0 && value.runtime.is_empty()
    }
}

fn valid_device(value: &PlacementDevice, expected_owner: &OwnerWire) -> bool {
    let _ = (
        value.available_cpu_cores,
        value.concurrency_limit,
        value.active_concurrency,
    );
    valid_identifier(&value.device_id)
        && valid_owner(&value.owner)
        && value.owner == *expected_owner
        && matches!(
            value.approval_state.as_str(),
            "approved" | "pending" | "revoked" | "unknown"
        )
        && matches!(
            value.cordon_state.as_str(),
            "clear" | "cordoned" | "unknown"
        )
        && matches!(value.liveness.as_str(), "online" | "offline" | "unknown")
        && value.snapshot_observed_at_ms >= 0
        && (value.snapshot_observed_at_ms as u64) <= MAX_SAFE_INTEGER
        && value.lease_expires_at_ms >= 0
        && (value.lease_expires_at_ms as u64) <= MAX_SAFE_INTEGER
        && valid_token(&value.os)
        && valid_token(&value.architecture)
        && value.available_memory_bytes <= MAX_SAFE_INTEGER
        && value.available_storage_bytes <= MAX_SAFE_INTEGER
        && value.runtimes.len() <= 32
        && value.runtimes.iter().all(|item| valid_token(item))
        && valid_gpu_declaration(&value.gpu)
        && valid_zones(&value.data_residency_zones)
        && matches!(
            value.trust_zone.as_str(),
            "untrusted" | "low" | "standard" | "high" | "restricted" | "unknown"
        )
        && value.sandbox_levels.len() <= 32
        && value
            .sandbox_levels
            .iter()
            .all(|item| matches!(item.as_str(), "process" | "container" | "microvm"))
}

fn valid_gpu_declaration(value: &GpuDeclaration) -> bool {
    if value.present {
        value.memory_bytes <= MAX_SAFE_INTEGER
            && (value.runtime.is_empty() || valid_token(&value.runtime))
    } else {
        value.memory_bytes == 0 && value.runtime.is_empty()
    }
}

fn valid_owner(value: &OwnerWire) -> bool {
    valid_owner_part(&value.issuer)
        && valid_owner_part(&value.subject)
        && valid_owner_part(&value.tenant_id)
}

fn valid_owner_part(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.chars().enumerate().all(|(index, character)| {
            character.is_ascii_alphanumeric()
                || (index > 0 && matches!(character, '.' | '_' | ':' | '-' | '+' | '/'))
        })
}

fn valid_route_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.chars().enumerate().all(|(index, character)| {
            character.is_ascii_alphanumeric()
                || (index > 0 && matches!(character, '.' | '_' | '-' | '+'))
        })
}

fn valid_idempotency_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | ':' | '-' | '+')
        })
}

fn valid_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || ". _:+/-".replace(' ', "").contains(character)
        })
}

fn valid_zones(values: &[String]) -> bool {
    values.len() <= 32
        && values.iter().all(|value| {
            !value.is_empty()
                && value.len() <= 64
                && value.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
                })
        })
        && values
            .iter()
            .enumerate()
            .all(|(index, value)| values[..index].iter().all(|previous| previous != value))
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn same_owner(value: &forge_runtime_domain::ConversationOwner, expected: &OwnerWire) -> bool {
    value.issuer == expected.issuer
        && value.subject == expected.subject
        && value.tenant_id == expected.tenant_id
}

fn invalid_request() -> RemoteError {
    RemoteError("remote Run/Attempt/lease preflight input is invalid".into())
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, RemoteError> {
    let mut bytes = Vec::with_capacity(MAX_INPUT_BYTES.min(64 * 1024));
    if input == "-" {
        io::stdin()
            .lock()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| {
                RemoteError("could not read remote Run/Attempt/lease preflight stdin".into())
            })?;
    } else {
        let path = Path::new(input);
        let metadata = std::fs::metadata(path).map_err(|_| {
            RemoteError("could not read remote Run/Attempt/lease preflight input".into())
        })?;
        if !metadata.is_file() {
            return Err(RemoteError(
                "remote Run/Attempt/lease preflight input must be a regular file".into(),
            ));
        }
        File::open(path)
            .map_err(|_| {
                RemoteError("could not read remote Run/Attempt/lease preflight input".into())
            })?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| {
                RemoteError("could not read remote Run/Attempt/lease preflight input".into())
            })?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(RemoteError(format!(
            "remote Run/Attempt/lease preflight input exceeds {MAX_INPUT_BYTES} bytes"
        )));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{conversation_and_run, read_request, render_human, validate_response};

    pub(super) fn request() -> Value {
        let owner =
            json!({"issuer":"https://id.example","subject":"user-1","tenant_id":"tenant-1"});
        let intent = json!({
            "schema_version": "forge.runner-execution-intent/v1",
            "evaluation_mode": "pure_runner_binding_only",
            "owner": owner,
            "conversation_id": "conversation-001",
            "prompt_id": "prompt-001",
            "run_id": "run-001",
            "attempt_id": "attempt-001",
            "command_id": "command-001",
            "target_id": "runner-1",
            "command_sha256": "42ed02a535113450e6f2cc757fb9b4e2cce6143724274191bbae159e9ea8de7a",
            "idempotency_key": "run-001:attempt-001:command-001",
            "prompt_run_binding_valid": true,
            "runner_command_binding_valid": true,
            "preview_only": true,
            "selected_target_id": null,
            "authority": {
                "device_identity_verified": false,
                "command_persisted": false,
                "reservation_created": false,
                "execution_authorized": false,
                "dispatch_performed": false,
                "audit_published": false
            }
        });
        let placement = json!({
            "schema_version":"forge.device-placement-dry-run/v1",
            "evaluated_at_ms":200500,
            "owner":owner,
            "max_snapshot_age_ms":86400000,
            "requirements": {
                "os":"linux","architecture":"x86_64","min_cpu_cores":1,
                "min_memory_bytes":1024,"min_storage_bytes":1024,"runtime":"forge",
                "gpu":{"required":false,"min_memory_bytes":0,"runtime":""},
                "data_residency_zones":["us"],"minimum_trust_zone":"standard",
                "sandbox_floor":"process","concurrency_slots":1
            },
            "devices":[
                {
                    "device_id":"runner-1","owner":owner,"approval_state":"approved","cordon_state":"clear","liveness":"online",
                    "snapshot_observed_at_ms":200000,"lease_expires_at_ms":201000,"os":"linux","architecture":"x86_64",
                    "available_cpu_cores":4,"available_memory_bytes":4096,"available_storage_bytes":4096,"runtimes":["forge"],
                    "gpu":{"present":false,"memory_bytes":0,"runtime":""},"data_residency_zones":["us"],"trust_zone":"standard",
                    "sandbox_levels":["process"],"concurrency_limit":2,"active_concurrency":0
                },
                {
                    "device_id":"runner-2","owner":owner,"approval_state":"approved","cordon_state":"clear","liveness":"online",
                    "snapshot_observed_at_ms":200000,"lease_expires_at_ms":201000,"os":"linux","architecture":"x86_64",
                    "available_cpu_cores":4,"available_memory_bytes":4096,"available_storage_bytes":4096,"runtimes":["forge"],
                    "gpu":{"present":false,"memory_bytes":0,"runtime":""},"data_residency_zones":["us"],"trust_zone":"standard",
                    "sandbox_levels":["process"],"concurrency_limit":2,"active_concurrency":0
                }
            ]
        });
        json!({
            "owner":owner,"conversation_id":"conversation-001","run_id":"run-001","run_status":"nonterminal",
            "dispatch_plan":{
                "attempt_state":"accepted","placement_request":placement,"runner_execution_intent":intent,
                "lease":{"v":1,"attempt_id":"attempt-001","target_id":"runner-1","epoch":1,"fencing_token":"fence-001","issued_at_ms":199500,"expires_at_ms":205500}
            }
        })
    }

    fn response() -> Value {
        serde_json::from_str(include_str!(
            "../../../../docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-v1.json"
        ))
        .expect("preflight fixture")
    }

    #[test]
    fn request_is_strict_and_path_bound() {
        let value = request();
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), serde_json::to_vec(&value).unwrap()).unwrap();
        let decoded = read_request(file.path().to_str().unwrap()).unwrap();
        assert_eq!(
            conversation_and_run(&decoded).unwrap(),
            ("conversation-001".to_owned(), "run-001".to_owned())
        );
        let duplicate = serde_json::to_string(&value).unwrap().replacen(
            "\"run_status\":\"nonterminal\"",
            "\"run_status\":\"nonterminal\",\"run_status\":\"nonterminal\"",
            1,
        );
        std::fs::write(file.path(), duplicate).unwrap();
        assert!(read_request(file.path().to_str().unwrap()).is_err());

        let mut path_confused = request();
        path_confused["conversation_id"] = Value::String("conversation/001".to_owned());
        std::fs::write(file.path(), serde_json::to_vec(&path_confused).unwrap()).unwrap();
        assert!(read_request(file.path().to_str().unwrap()).is_err());
    }

    #[test]
    fn canonical_request_fixture_is_accepted() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            file.path(),
            include_bytes!(
                "../../../../docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-request-v1.json"
            ),
        )
        .unwrap();
        let decoded = read_request(file.path().to_str().unwrap()).unwrap();
        assert_eq!(
            conversation_and_run(&decoded).unwrap(),
            ("conversation-001".to_owned(), "run-001".to_owned())
        );
    }

    #[test]
    fn response_requires_bindings_and_false_authority() {
        let request = request();
        let response = response();
        validate_response(&response, &request, "conversation-001", "run-001").unwrap();
        let mut foreign = response.clone();
        foreign["run_id"] = json!("run-002");
        assert!(validate_response(&foreign, &request, "conversation-001", "run-001").is_err());
        let mut selected = response.clone();
        selected["selected_target_id"] = json!("runner-1");
        assert!(validate_response(&selected, &request, "conversation-001", "run-001").is_err());
        let mut authority = response.clone();
        authority["authority"]["dispatch_performed"] = json!(true);
        assert!(validate_response(&authority, &request, "conversation-001", "run-001").is_err());
    }

    #[test]
    fn human_output_is_metadata_only() {
        let mut output = Vec::new();
        render_human(&response(), &mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("declarative_preflight_ready=true selected_target_id=null"));
        assert!(output.contains("dispatch_performed=false"));
        assert!(!output.contains("fence-001"));
        assert!(!output.contains("argv"));
    }
}

#[cfg(test)]
pub(super) fn test_request() -> Value {
    tests::request()
}
