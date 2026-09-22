//! Bounded local decoder for the owner-bound device credential lifecycle
//! candidate response.  This is a metadata-only reader: it never sends the
//! candidate POST, reads a bearer, creates credential material, or grants
//! device/execution authority.

use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

use serde::{Deserialize, Serialize};

use crate::{args::DeviceCommand, device_json_unique::reject_duplicate_keys};

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_OWNER_PART_BYTES: usize = 512;
const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_DIGEST_BYTES: usize = 64;
const MIN_LIFETIME_MS: u64 = 1_000;
const MAX_LIFETIME_MS: u64 = 3_600_000;
const SCHEMA_VERSION: &str = "forge.device-credential-lifecycle/v1";
const EVALUATION_MODE: &str = "pure_device_credential_lifecycle";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeviceCredentialCandidateOutput {
    schema_version: String,
    evaluation_mode: String,
    owner: Owner,
    device_id: String,
    action: Action,
    revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    previous: Option<State>,
    next: State,
    preview_only: bool,
    candidate_published: bool,
    authority: Authority,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Owner {
    issuer: String,
    subject: String,
    tenant_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Action {
    Issue,
    Revoke,
    Rotate,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct State {
    credential_id: String,
    device_id: String,
    owner: Owner,
    approval_state: String,
    credential_state: String,
    key_id: String,
    public_key_sha256: String,
    key_generation: u64,
    issued_at_ms: u64,
    expires_at_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Authority {
    owner_binding_matched: bool,
    owner_authenticated: bool,
    credential_material_made: bool,
    persisted: bool,
    inventory_authoritative: bool,
    execution_authorized: bool,
}

pub(crate) fn execute(
    command: &DeviceCommand,
) -> Result<DeviceCredentialCandidateOutput, Box<dyn Error>> {
    let DeviceCommand::CredentialCandidatePreview { input: input_path } = command else {
        return Err("device credential-candidate preview command is required".into());
    };
    let bytes = read_bounded_input(input_path)?;
    reject_duplicate_keys(&bytes).map_err(|error| {
        format!("device credential-candidate input contains duplicate JSON keys: {error}")
    })?;
    let output: DeviceCredentialCandidateOutput = serde_json::from_slice(&bytes)
        .map_err(|error| format!("device credential-candidate input is invalid JSON: {error}"))?;
    validate_output(&output)?;
    Ok(output)
}

/// Decodes an already obtained candidate response without performing I/O.
/// Callers that obtained a response over an explicitly reviewed transport
/// must still enforce their own authenticated-owner binding at that boundary.
#[allow(dead_code)]
pub(crate) fn decode_remote_response(
    value: &serde_json::Value,
) -> Result<DeviceCredentialCandidateOutput, Box<dyn Error>> {
    let output: DeviceCredentialCandidateOutput = serde_json::from_value(value.clone())
        .map_err(|error| format!("device credential-candidate response is invalid: {error}"))?;
    validate_output(&output)?;
    Ok(output)
}

/// Validates one explicitly requested HTTP candidate response against the
/// caller's request image.  The authenticated owner is established by the
/// Forge server's bearer boundary; this value layer still binds the response
/// to the requested device, action, and lifecycle revision before rendering.
pub(crate) fn validate_remote_response(
    value: &serde_json::Value,
    request: &serde_json::Value,
) -> Result<DeviceCredentialCandidateOutput, Box<dyn Error>> {
    let request: RemoteRequest = serde_json::from_value(request.clone())
        .map_err(|error| format!("device credential-candidate request is invalid: {error}"))?;
    validate_remote_request_fields(&request)?;
    let output = decode_remote_response(value)?;
    if output.device_id != request.device_id
        || output.action != request.action
        || output.revision != request.expected_device_revision
    {
        return Err("device credential-candidate response is bound to another request".into());
    }
    match request.action {
        Action::Issue => {
            if output.previous.is_some()
                || output.next.credential_id != request.credential_id
                || output.next.key_id != request.key_id
                || output.next.public_key_sha256 != request.public_key_sha256
                || output.next.key_generation != request.key_generation
                || output.next.approval_state != request.approval_state
                || output.next.issued_at_ms != request.issued_at_ms
                || output.next.expires_at_ms != request.expires_at_ms
            {
                return Err(
                    "device credential-candidate response drifted from issue request".into(),
                );
            }
        }
        Action::Revoke => {
            if output.previous.is_none()
                || output.next.credential_state != "revoked"
                || (!request.approval_state.is_empty()
                    && output.next.approval_state != request.approval_state)
            {
                return Err("device credential-candidate revoke response is invalid".into());
            }
        }
        Action::Rotate => {
            if output.previous.is_none()
                || output.next.credential_id != request.next_credential_id
                || output.next.key_id != request.next_key_id
                || output.next.public_key_sha256 != request.next_public_key_sha256
                || output.next.issued_at_ms != request.issued_at_ms
                || output.next.expires_at_ms != request.expires_at_ms
                || (!request.approval_state.is_empty()
                    && output.next.approval_state != request.approval_state)
            {
                return Err(
                    "device credential-candidate response drifted from rotate request".into(),
                );
            }
        }
    }
    Ok(output)
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoteRequest {
    device_id: String,
    action: Action,
    approval_state: String,
    credential_id: String,
    key_id: String,
    public_key_sha256: String,
    key_generation: u64,
    issued_at_ms: u64,
    expires_at_ms: u64,
    next_credential_id: String,
    next_key_id: String,
    next_public_key_sha256: String,
    observed_at_ms: u64,
    expected_device_revision: u64,
}

pub(crate) fn validate_remote_request(value: &serde_json::Value) -> Result<(), Box<dyn Error>> {
    let request: RemoteRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("device credential-candidate request is invalid: {error}"))?;
    validate_remote_request_fields(&request)
}

fn validate_remote_request_fields(request: &RemoteRequest) -> Result<(), Box<dyn Error>> {
    if !valid_transport_identifier(&request.device_id)
        || request.observed_at_ms > MAX_SAFE_JSON_INTEGER
        || request.expected_device_revision == 0
        || request.expected_device_revision > MAX_SAFE_JSON_INTEGER
    {
        return Err("device credential-candidate request is invalid".into());
    }
    match request.action {
        Action::Issue => {
            if !valid_transport_identifier(&request.credential_id)
                || !valid_transport_identifier(&request.key_id)
                || !valid_digest(&request.public_key_sha256)
                || request.key_generation == 0
                || request.key_generation > MAX_SAFE_JSON_INTEGER
                || !valid_approval(&request.approval_state)
                || request.approval_state == "revoked"
                || !valid_window_with_observation(
                    request.issued_at_ms,
                    request.expires_at_ms,
                    request.observed_at_ms,
                )
                || !request.next_credential_id.is_empty()
                || !request.next_key_id.is_empty()
                || !request.next_public_key_sha256.is_empty()
            {
                return Err("device credential-candidate issue request is invalid".into());
            }
        }
        Action::Revoke => {
            if (!request.approval_state.is_empty() && !valid_approval(&request.approval_state))
                || !request.credential_id.is_empty()
                || !request.key_id.is_empty()
                || !request.public_key_sha256.is_empty()
                || request.key_generation != 0
                || request.issued_at_ms != 0
                || request.expires_at_ms != 0
                || !request.next_credential_id.is_empty()
                || !request.next_key_id.is_empty()
                || !request.next_public_key_sha256.is_empty()
            {
                return Err("device credential-candidate revoke request is invalid".into());
            }
        }
        Action::Rotate => {
            if (!request.approval_state.is_empty() && !valid_approval(&request.approval_state))
                || !request.credential_id.is_empty()
                || !request.key_id.is_empty()
                || !request.public_key_sha256.is_empty()
                || request.key_generation != 0
                || !valid_transport_identifier(&request.next_credential_id)
                || !valid_transport_identifier(&request.next_key_id)
                || !valid_digest(&request.next_public_key_sha256)
                || !valid_window_with_observation(
                    request.issued_at_ms,
                    request.expires_at_ms,
                    request.observed_at_ms,
                )
            {
                return Err("device credential-candidate rotate request is invalid".into());
            }
        }
    }
    Ok(())
}

fn valid_approval(value: &str) -> bool {
    matches!(value, "pending" | "approved" | "revoked")
}

fn valid_window_with_observation(
    issued_at_ms: u64,
    expires_at_ms: u64,
    observed_at_ms: u64,
) -> bool {
    valid_window(issued_at_ms, expires_at_ms)
        && issued_at_ms <= observed_at_ms
        && observed_at_ms < expires_at_ms
}

fn valid_transport_identifier(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_IDENTIFIER_BYTES {
        return false;
    }
    value.chars().enumerate().all(|(index, character)| {
        character.is_ascii_alphanumeric()
            || (index > 0 && matches!(character, '.' | '_' | ':' | '-'))
    })
}

pub(crate) fn write_output(
    output: &DeviceCredentialCandidateOutput,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    writeln!(
        writer,
        "offline device credential candidate [{}] action={:?} device={} revision={}",
        output.schema_version, output.action, output.device_id, output.revision
    )?;
    writeln!(
        writer,
        "owner={}/{}/{} credential={} key={} generation={} approval={} state={} window={}..{} preview_only=true candidate_published=true",
        output.owner.issuer,
        output.owner.subject,
        output.owner.tenant_id,
        output.next.credential_id,
        output.next.key_id,
        output.next.key_generation,
        output.next.approval_state,
        output.next.credential_state,
        output.next.issued_at_ms,
        output.next.expires_at_ms
    )?;
    writeln!(
        writer,
        "authority: owner_binding_matched=false owner_authenticated=false credential_material_made=false persisted=false inventory_authoritative=false execution_authorized=false"
    )
}

pub(crate) fn write_remote_output(
    output: &DeviceCredentialCandidateOutput,
    writer: &mut impl Write,
) -> io::Result<()> {
    writeln!(
        writer,
        "remote device credential candidate [{}] action={:?} device={} revision={}",
        output.schema_version, output.action, output.device_id, output.revision
    )?;
    writeln!(
        writer,
        "owner={}/{}/{} credential={} key={} generation={} approval={} state={} window={}..{} preview_only=true candidate_published=true",
        output.owner.issuer,
        output.owner.subject,
        output.owner.tenant_id,
        output.next.credential_id,
        output.next.key_id,
        output.next.key_generation,
        output.next.approval_state,
        output.next.credential_state,
        output.next.issued_at_ms,
        output.next.expires_at_ms
    )?;
    writeln!(
        writer,
        "authority: owner_binding_matched=false owner_authenticated=false credential_material_made=false persisted=false inventory_authoritative=false execution_authorized=false"
    )
}

fn validate_output(output: &DeviceCredentialCandidateOutput) -> Result<(), Box<dyn Error>> {
    if output.schema_version != SCHEMA_VERSION
        || output.evaluation_mode != EVALUATION_MODE
        || !valid_owner(&output.owner)
        || !valid_identifier(&output.device_id)
        || output.revision == 0
        || output.revision > MAX_SAFE_JSON_INTEGER
        || !output.preview_only
        || !output.candidate_published
        || !authority_is_false(output.authority)
        || !valid_state(&output.next)
        || output.next.owner != output.owner
        || output.next.device_id != output.device_id
    {
        return Err("device credential-candidate response is invalid".into());
    }

    match output.action {
        Action::Issue => {
            if output.previous.is_some() || output.next.credential_state != "active" {
                return Err("device credential issue candidate is invalid".into());
            }
        }
        Action::Revoke => {
            let Some(previous) = &output.previous else {
                return Err("device credential revoke candidate lacks previous state".into());
            };
            if !valid_state(previous)
                || previous.owner != output.owner
                || previous.device_id != output.device_id
                || output.next.credential_state != "revoked"
                || !same_metadata_except_state(previous, &output.next)
            {
                return Err("device credential revoke candidate is invalid".into());
            }
        }
        Action::Rotate => {
            let Some(previous) = &output.previous else {
                return Err("device credential rotate candidate lacks previous state".into());
            };
            if !valid_state(previous)
                || previous.owner != output.owner
                || previous.device_id != output.device_id
                || output.next.credential_state != "active"
                || output.next.key_generation != previous.key_generation.saturating_add(1)
                || output.next.key_generation == 0
                || (output.next.credential_id == previous.credential_id
                    && output.next.key_id == previous.key_id
                    && output.next.public_key_sha256 == previous.public_key_sha256)
                || output.next.approval_state != previous.approval_state
                || output.next.device_id != previous.device_id
                || output.next.owner != previous.owner
            {
                return Err("device credential rotate candidate is invalid".into());
            }
        }
    }
    Ok(())
}

fn valid_state(state: &State) -> bool {
    valid_identifier(&state.credential_id)
        && valid_identifier(&state.device_id)
        && valid_owner(&state.owner)
        && matches!(
            state.approval_state.as_str(),
            "pending" | "approved" | "revoked"
        )
        && matches!(
            state.credential_state.as_str(),
            "active" | "expired" | "revoked"
        )
        && valid_identifier(&state.key_id)
        && valid_digest(&state.public_key_sha256)
        && state.key_generation > 0
        && state.key_generation <= MAX_SAFE_JSON_INTEGER
        && valid_window(state.issued_at_ms, state.expires_at_ms)
}

fn same_metadata_except_state(previous: &State, next: &State) -> bool {
    previous.credential_id == next.credential_id
        && previous.device_id == next.device_id
        && previous.owner == next.owner
        && previous.approval_state == next.approval_state
        && previous.key_id == next.key_id
        && previous.public_key_sha256 == next.public_key_sha256
        && previous.key_generation == next.key_generation
        && previous.issued_at_ms == next.issued_at_ms
        && previous.expires_at_ms == next.expires_at_ms
}

fn valid_window(issued_at_ms: u64, expires_at_ms: u64) -> bool {
    expires_at_ms > issued_at_ms
        && expires_at_ms <= MAX_SAFE_JSON_INTEGER
        && expires_at_ms - issued_at_ms >= MIN_LIFETIME_MS
        && expires_at_ms - issued_at_ms <= MAX_LIFETIME_MS
}

fn valid_digest(value: &str) -> bool {
    value.len() == MAX_DIGEST_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
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
            || (index > 0 && matches!(character, '.' | '_' | ':' | '-'))
    })
}

fn authority_is_false(authority: Authority) -> bool {
    !authority.owner_binding_matched
        && !authority.owner_authenticated
        && !authority.credential_material_made
        && !authority.persisted
        && !authority.inventory_authoritative
        && !authority.execution_authorized
}

fn read_bounded_input(input: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::with_capacity(MAX_INPUT_BYTES.min(64 * 1024));
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
            format!("device credential-candidate input exceeds {MAX_INPUT_BYTES} bytes").into(),
        );
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "device_credential_candidate_command_tests.rs"]
mod tests;
