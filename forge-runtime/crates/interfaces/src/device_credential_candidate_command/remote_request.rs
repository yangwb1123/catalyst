use serde::Deserialize;
use std::error::Error;

use super::{
    Action, DeviceCredentialCandidateOutput, MAX_IDENTIFIER_BYTES, MAX_SAFE_JSON_INTEGER,
    decode_remote_response, valid_digest, valid_window,
};

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
    validate_response_action(&output, &request)?;
    Ok(output)
}

fn validate_response_action(
    output: &DeviceCredentialCandidateOutput,
    request: &RemoteRequest,
) -> Result<(), Box<dyn Error>> {
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
    Ok(())
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
        Action::Issue => validate_issue_request(request),
        Action::Revoke => validate_revoke_request(request),
        Action::Rotate => validate_rotate_request(request),
    }
}

fn validate_issue_request(request: &RemoteRequest) -> Result<(), Box<dyn Error>> {
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
    Ok(())
}

fn validate_revoke_request(request: &RemoteRequest) -> Result<(), Box<dyn Error>> {
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
    Ok(())
}

fn validate_rotate_request(request: &RemoteRequest) -> Result<(), Box<dyn Error>> {
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
