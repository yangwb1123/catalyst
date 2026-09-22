//! Bounded Rust consumer for the Go Runner dispatch-plan preview.
//!
//! The input is already an observation produced by the Go value contract. It
//! is decoded and revalidated locally so a CLI consumer cannot turn a
//! candidate list into a selected target, reservation, lease authority, or
//! dispatch request.

use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{args::DeviceCommand, device_json_unique::reject_duplicate_keys};

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_CANDIDATES: usize = 128;
const MAX_OWNER_PART_BYTES: usize = 512;
const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_TIMESTAMP_MS: i64 = 9_007_199_254_740_991;
const SCHEMA_VERSION: &str = "forge.runner-dispatch-plan-preview/v1";
const EVALUATION_MODE: &str = "pure_dispatch_plan_preview_only";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RunnerDispatchPlanPreviewOutput {
    schema_version: String,
    evaluation_mode: String,
    #[serde(rename = "owner_declaration")]
    owner: Owner,
    conversation_id: String,
    run_id: String,
    attempt_id: String,
    attempt_state: String,
    attempt_state_admissible: bool,
    command_id: String,
    command_sha256: String,
    #[serde(rename = "intent_target_id")]
    target_id: String,
    lease_epoch: u64,
    lease_active: bool,
    evaluated_at_ms: i64,
    candidate_count: usize,
    declarative_ready_count: usize,
    candidates: Vec<Candidate>,
    selected_target_id: Option<String>,
    preview_only: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
    authority: Authority,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Owner {
    issuer: String,
    subject: String,
    tenant_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Candidate {
    target_id: String,
    attributes_unverified: bool,
    matches_requirements: bool,
    lease_target_match: bool,
    lease_active: bool,
    attempt_state_admissible: bool,
    declarative_ready: bool,
    reasons: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Authority {
    device_identity_verified: bool,
    attempt_persisted: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
    audit_published: bool,
}

pub(crate) fn execute(
    command: &DeviceCommand,
) -> Result<RunnerDispatchPlanPreviewOutput, Box<dyn Error>> {
    let DeviceCommand::RunnerDispatchPlanPreview { input: input_path } = command else {
        return Err("device Runner dispatch-plan preview command is required".into());
    };
    let bytes = read_bounded_input(input_path)?;
    reject_duplicate_keys(&bytes).map_err(|error| {
        format!("Runner dispatch-plan preview input contains duplicate JSON keys: {error}")
    })?;
    let output: RunnerDispatchPlanPreviewOutput = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Runner dispatch-plan preview input is invalid JSON: {error}"))?;
    validate_output(&output)?;
    Ok(output)
}

pub(crate) fn write_output(
    output: &RunnerDispatchPlanPreviewOutput,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    writeln!(
        writer,
        "offline Runner dispatch-plan preview [{}]",
        output.schema_version
    )?;
    writeln!(
        writer,
        "owner={}/{}/{} conversation={} run={} attempt={} command={} target={}",
        output.owner.issuer,
        output.owner.subject,
        output.owner.tenant_id,
        output.conversation_id,
        output.run_id,
        output.attempt_id,
        output.command_id,
        output.target_id
    )?;
    writeln!(
        writer,
        "plan: attempt_state={} admissible={} lease_active={} candidates={} declarative_ready={} selected_target=none evaluated_at_ms={}",
        output.attempt_state,
        output.attempt_state_admissible,
        output.lease_active,
        output.candidate_count,
        output.declarative_ready_count,
        output.evaluated_at_ms
    )?;
    for candidate in &output.candidates {
        writeln!(
            writer,
            "candidate {}: requirements_match={} lease_target_match={} lease_active={} attempt_state_admissible={} declarative_ready={} reasons={}",
            candidate.target_id,
            candidate.matches_requirements,
            candidate.lease_target_match,
            candidate.lease_active,
            candidate.attempt_state_admissible,
            candidate.declarative_ready,
            candidate.reasons.join(",")
        )?;
    }
    writeln!(
        writer,
        "authority: device_identity_verified=false attempt_persisted=false reservation_created=false execution_authorized=false dispatch_performed=false audit_published=false"
    )
}

fn validate_output(output: &RunnerDispatchPlanPreviewOutput) -> Result<(), Box<dyn Error>> {
    if output.schema_version != SCHEMA_VERSION
        || output.evaluation_mode != EVALUATION_MODE
        || !valid_owner(&output.owner)
        || !valid_identifier(&output.conversation_id)
        || !valid_identifier(&output.run_id)
        || !valid_identifier(&output.attempt_id)
        || !valid_identifier(&output.command_id)
        || !valid_identifier(&output.target_id)
        || !valid_digest(&output.command_sha256)
        || !valid_attempt_state(&output.attempt_state)
        || output.attempt_state_admissible != dispatchable_attempt_state(&output.attempt_state)
        || output.lease_epoch == 0
        || output.evaluated_at_ms <= 0
        || output.evaluated_at_ms > MAX_TIMESTAMP_MS
        || output.selected_target_id.is_some()
        || !output.preview_only
        || output.reservation_created
        || output.execution_authorized
        || output.dispatch_performed
        || !authority_is_false(output.authority)
        || output.candidate_count != output.candidates.len()
        || output.candidate_count > MAX_CANDIDATES
    {
        return Err("Runner dispatch-plan preview observation is invalid".into());
    }

    let mut ready_count = 0;
    for (index, candidate) in output.candidates.iter().enumerate() {
        if !valid_identifier(&candidate.target_id)
            || !candidate.attributes_unverified
            || candidate.lease_target_match != (candidate.target_id == output.target_id)
            || candidate.lease_active != output.lease_active
            || candidate.attempt_state_admissible != output.attempt_state_admissible
            || candidate.declarative_ready
                != (candidate.matches_requirements
                    && candidate.lease_target_match
                    && candidate.lease_active
                    && candidate.attempt_state_admissible)
            || !sorted_unique(&candidate.reasons)
            || (candidate.declarative_ready && !candidate.reasons.is_empty())
        {
            return Err(format!(
                "Runner dispatch-plan preview candidate {} is invalid",
                candidate.target_id
            )
            .into());
        }
        if index > 0 && output.candidates[index - 1].target_id >= candidate.target_id {
            return Err("Runner dispatch-plan preview candidates are not sorted uniquely".into());
        }
        if candidate.declarative_ready {
            ready_count += 1;
        }
    }
    if ready_count != output.declarative_ready_count {
        return Err("Runner dispatch-plan preview ready count mismatch".into());
    }
    Ok(())
}

/// Validates the strict observation returned by the authenticated candidate.
///
/// The remote transport shares the same decoder as the standalone device
/// command so a response cannot gain authority merely by coming from HTTP.
pub(crate) fn validate_remote_response(value: &Value) -> Result<(), Box<dyn Error>> {
    let output: RunnerDispatchPlanPreviewOutput = serde_json::from_value(value.clone())
        .map_err(|error| format!("Runner dispatch-plan preview response is invalid: {error}"))?;
    validate_output(&output)
}

/// Renders an already validated authenticated observation without exposing
/// command payloads, fencing material, or an execution decision.
pub(crate) fn write_remote_output(value: &Value, writer: &mut impl Write) -> io::Result<()> {
    let output: RunnerDispatchPlanPreviewOutput = serde_json::from_value(value.clone())
        .expect("validated Runner dispatch-plan preview response");
    write_output(&output, false, writer)
}

fn valid_owner(owner: &Owner) -> bool {
    valid_owner_part(&owner.issuer)
        && valid_owner_part(&owner.subject)
        && valid_owner_part(&owner.tenant_id)
}

fn valid_owner_part(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_OWNER_PART_BYTES
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn valid_identifier(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_IDENTIFIER_BYTES {
        return false;
    }
    value.chars().enumerate().all(|(index, character)| {
        character.is_ascii_alphanumeric()
            || (index > 0 && matches!(character, '.' | '_' | ':' | '-' | '+' | '/'))
    })
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn valid_attempt_state(value: &str) -> bool {
    matches!(
        value,
        "requested"
            | "accepted"
            | "starting"
            | "running"
            | "interrupted"
            | "completed"
            | "failed"
            | "uncertain"
    )
}

fn dispatchable_attempt_state(value: &str) -> bool {
    matches!(value, "accepted" | "starting" | "running")
}

fn sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}

fn authority_is_false(authority: Authority) -> bool {
    !authority.device_identity_verified
        && !authority.attempt_persisted
        && !authority.reservation_created
        && !authority.execution_authorized
        && !authority.dispatch_performed
        && !authority.audit_published
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::new();
    if input == "-" {
        io::stdin()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
    } else {
        File::open(Path::new(input))?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(
            format!("Runner dispatch-plan preview input exceeds {MAX_INPUT_BYTES} bytes").into(),
        );
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "device_runner_dispatch_plan_command_tests.rs"]
mod tests;
