use std::{
    io::{self, Read, Write},
    process::ExitCode,
};

use crate::runtime_domain::{
    Conversation, ConversationScope, GroupProjectMember, HubEntity, HubSnapshot,
    HubSnapshotAtCursor, HubStoreError, Project, SessionGroup,
};
use forge_runtime_application::HubError;
use serde::Serialize;

use super::{API_VERSION, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES};

#[derive(Serialize)]
struct SuccessEnvelope<T> {
    api_version: &'static str,
    request_id: String,
    ok: bool,
    result: T,
}

#[derive(Serialize)]
struct ErrorEnvelope {
    api_version: &'static str,
    request_id: String,
    ok: bool,
    error: RpcError,
}

#[derive(Serialize)]
struct RpcError {
    code: &'static str,
    message: &'static str,
}

#[derive(Serialize)]
pub(super) struct SnapshotAtCursor {
    snapshot: SnapshotProjection,
    cursor: u64,
}

#[derive(Serialize)]
struct SnapshotProjection {
    scope: ConversationScope,
    projects: Vec<ProjectProjection>,
    conversations: Vec<Conversation>,
    groups: Vec<SessionGroup>,
    group_project_members: Vec<GroupProjectMember>,
}

#[derive(Serialize)]
struct ProjectProjection {
    id: String,
    name: String,
    created_at_ms: u64,
}

pub(super) fn read_framed_request(mut reader: impl Read) -> Result<Vec<u8>, &'static str> {
    let mut framed = Vec::new();
    reader
        .by_ref()
        .take((MAX_REQUEST_BYTES + 1) as u64)
        .read_to_end(&mut framed)
        .map_err(|_| "invalid_framing")?;
    if framed.len() > MAX_REQUEST_BYTES {
        return Err("request_too_large");
    }
    if framed.pop() != Some(b'\n') || framed.contains(&b'\n') || framed.contains(&b'\r') {
        return Err("invalid_framing");
    }
    Ok(framed)
}

pub(super) fn with_api_version(mut response: Vec<u8>, api_version: &'static str) -> Vec<u8> {
    if api_version == API_VERSION {
        return response;
    }
    let Ok(mut envelope) = serde_json::from_slice::<serde_json::Value>(&response) else {
        return response;
    };
    if let Some(object) = envelope.as_object_mut() {
        object.insert(
            "api_version".into(),
            serde_json::Value::String(api_version.into()),
        );
    }
    if let Ok(versioned) = serde_json::to_vec(&envelope) {
        response = versioned;
    }
    response
}

pub(super) fn owned_error_response(request_id: &str, error: &HubError) -> Vec<u8> {
    let code = match error {
        HubError::Store(HubStoreError::NotFound {
            entity:
                HubEntity::Conversation
                | HubEntity::Prompt
                | HubEntity::ProjectExecutionConsent
                | HubEntity::PendingRunIntent,
            ..
        }) => "not_found",
        HubError::Store(HubStoreError::Conflict { .. }) => "conflict",
        HubError::Store(HubStoreError::Unavailable { .. }) => "storage_unavailable",
        HubError::Store(HubStoreError::Corrupt { .. }) => "storage_corrupt",
        _ => "invalid_owned_request",
    };
    error_response(request_id, code)
}

pub(super) fn snapshot_projection(snapshot: HubSnapshotAtCursor) -> SnapshotAtCursor {
    SnapshotAtCursor {
        snapshot: project_snapshot(snapshot.snapshot),
        cursor: snapshot.cursor,
    }
}

fn project_snapshot(snapshot: HubSnapshot) -> SnapshotProjection {
    SnapshotProjection {
        scope: snapshot.scope,
        projects: snapshot
            .projects
            .into_iter()
            .map(|project: Project| ProjectProjection {
                id: project.id,
                name: project.name,
                created_at_ms: project.created_at_ms,
            })
            .collect(),
        conversations: snapshot.conversations,
        groups: snapshot.groups,
        group_project_members: snapshot.group_project_members,
    }
}

pub(super) fn success_response<T: Serialize>(request_id: &str, result: T) -> Vec<u8> {
    let envelope = SuccessEnvelope {
        api_version: API_VERSION,
        request_id: request_id.to_owned(),
        ok: true,
        result,
    };
    let mut output = CappedWriter::new(MAX_RESPONSE_BYTES - 1);
    if serde_json::to_writer(&mut output, &envelope).is_ok() {
        output.bytes
    } else if output.exceeded {
        error_response(request_id, "response_too_large")
    } else {
        error_response(request_id, "response_encoding_failed")
    }
}

pub(super) fn error_response(request_id: &str, code: &'static str) -> Vec<u8> {
    let message = match code {
        "unsupported_version" => "protocol version is unsupported",
        "invalid_limit" => "change page limit is outside the allowed range",
        "invalid_cursor" => "Conversation bootstrap cursor is invalid",
        "invalid_prompt_request" => "Prompt page request is invalid",
        "invalid_owned_conversation_request" => "owner Conversation request is invalid",
        "invalid_owned_prompt_request" => "owner Prompt request is invalid",
        "invalid_project_execution_consent_request" => {
            "Project execution consent request is invalid"
        }
        "invalid_owned_run_request" => "owner Run request is invalid",
        "write_rejected" => "Runtime Hub write was rejected",
        "not_found" => "owned resource was not found",
        "conflict" => "resource state conflicts with request",
        "storage_unavailable" => "Runtime Hub storage is unavailable",
        "storage_corrupt" => "Runtime Hub storage cannot serve the request",
        "invalid_owned_request" => "owner-scoped request is invalid",
        "invalid_request_id" => "request ID is invalid",
        "request_too_large" => "request exceeds the size limit",
        "invalid_framing" | "invalid_request" => "request framing or JSON is invalid",
        "response_too_large" => "response exceeds the size limit",
        _ => "Runtime Hub query failed",
    };
    serde_json::to_vec(&ErrorEnvelope {
        api_version: API_VERSION,
        request_id: request_id.to_owned(),
        ok: false,
        error: RpcError { code, message },
    })
    .unwrap_or_else(|_| b"{\"api_version\":\"forgeos.runtime-bridge/v1\",\"request_id\":\"invalid\",\"ok\":false,\"error\":{\"code\":\"response_encoding_failed\",\"message\":\"Runtime Hub query failed\"}}".to_vec())
}

pub(super) fn bound_response(mut response: Vec<u8>, request_id: &str) -> Vec<u8> {
    if response.len() + 1 > MAX_RESPONSE_BYTES {
        response = error_response(request_id, "response_too_large");
    }
    response.push(b'\n');
    response
}

struct CappedWriter {
    bytes: Vec<u8>,
    limit: usize,
    exceeded: bool,
}

impl CappedWriter {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(limit.min(8 * 1024)),
            limit,
            exceeded: false,
        }
    }
}

impl Write for CappedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.bytes.len().saturating_add(bytes.len()) > self.limit {
            self.exceeded = true;
            return Err(io::Error::other("RPC response limit exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) fn write_response(response: &[u8]) -> ExitCode {
    match io::stdout().lock().write_all(response) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
