//! Strict authenticated consumer for the Run/Attempt/lease dispatch preflight
//! candidate.
//!
//! The server endpoint is a private, test-only value adapter.  This module
//! keeps that boundary explicit: the input is a bounded caller declaration,
//! the POST is sent once, and the response is decoded as the strict
//! metadata-only domain observation.  No target is selected and no authority
//! bit is accepted from the transport.

#[path = "remote_run_attempt_lease_dispatch_preflight/request_validation.rs"]
mod request_validation;
use request_validation::{
    same_owner, valid_digest, valid_idempotency_key, valid_identifier, valid_owner,
    valid_placement, valid_route_identifier,
};

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
#[allow(
    clippy::struct_excessive_bools,
    reason = "Published wire predicates are separate booleans; changing their representation would change the protocol."
)]
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
    validate_request_model(&request)?;
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
    let expected_lease_active = plan
        .lease
        .is_active(placement.evaluated_at_ms.cast_unsigned());
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
        || observation.evaluated_at_ms != placement.evaluated_at_ms.cast_unsigned()
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
    validate_request_model(&request)?;
    Ok(request)
}

pub(super) fn validate_request(value: &Value) -> Result<(), RemoteError> {
    decode_request(value).map(|_| ())
}

fn validate_request_model(request: &PreflightRequest) -> Result<(), RemoteError> {
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
#[path = "remote_run_attempt_lease_dispatch_preflight/tests.rs"]
mod tests;

#[cfg(test)]
pub(super) fn test_request() -> Value {
    tests::request()
}
