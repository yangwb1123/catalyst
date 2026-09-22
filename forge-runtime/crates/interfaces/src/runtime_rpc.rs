use std::{
    env, io,
    path::{Component, Path, PathBuf},
    process::ExitCode,
    sync::Arc,
};

use crate::runtime_application::HubService;
use forge_runtime_application::HubError;
use forge_runtime_infrastructure::SqliteHubStore;
use serde::Serialize;

mod conversation_transport;
mod owned_operations;
mod pending_intent_transport;
mod validation;
mod wire;

use owned_operations::execute_owned_operation;

use conversation_transport::{ConversationChangesJsonSafe, ConversationTimestampsJsonSafe};
use validation::{Operation, RpcRequest, validate_request};
use wire::{
    bound_response, error_response, read_framed_request, snapshot_projection, success_response,
    with_api_version, write_response,
};

#[cfg(test)]
use crate::runtime_domain::MAX_CONVERSATION_PROMPT_PAGE_LIMIT;

const API_VERSION: &str = "forgeos.runtime-bridge/v1";
const WRITE_API_VERSION: &str = "forgeos.runtime-bridge/v2";
const MAX_READ_REQUEST_BYTES: usize = 16 * 1024;
const MAX_REQUEST_BYTES: usize = 2 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_REQUEST_ID_BYTES: usize = 128;
const MAX_CHANGE_LIMIT: usize = 128;

pub(crate) fn run_if_invoked() -> Option<ExitCode> {
    let mut args = env::args_os().skip(1);
    if args.next()?.to_str() != Some("--runtime-rpc") {
        return None;
    }
    let Some(database) = parse_database_path(args) else {
        eprintln!("invalid Runtime RPC invocation");
        return Some(ExitCode::from(2));
    };
    Some(match read_framed_request(io::stdin().lock()) {
        Ok(input) => write_response(&process_request(&database, &input)),
        Err(code) => write_response(&bound_response(error_response("invalid", code), "invalid")),
    })
}

fn parse_database_path(mut args: impl Iterator<Item = std::ffi::OsString>) -> Option<PathBuf> {
    if args.next()?.to_str()? != "--database" {
        return None;
    }
    let database = PathBuf::from(args.next()?);
    if args.next().is_some() || !database.is_absolute() {
        return None;
    }
    if database
        .components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return None;
    }
    Some(database)
}

fn process_request(database: &Path, input: &[u8]) -> Vec<u8> {
    if input.len() + 1 > MAX_REQUEST_BYTES {
        return bound_response(error_response("invalid", "request_too_large"), "invalid");
    }
    let request: RpcRequest = match serde_json::from_slice(input) {
        Ok(request) => request,
        Err(_) => {
            return bound_response(error_response("invalid", "invalid_request"), "invalid");
        }
    };
    let response_api_version = request.response_api_version();
    let (request_id, operation) = match validate_request(request, input.len()) {
        Ok(validated) => validated,
        Err((request_id, code)) => {
            return bound_response(
                with_api_version(error_response(&request_id, code), response_api_version),
                &request_id,
            );
        }
    };
    let store = match if operation.requires_write() {
        SqliteHubStore::open_existing_current_writable(database)
    } else {
        SqliteHubStore::open_existing_current_live_read_only(database)
    } {
        Ok(store) => Arc::new(store),
        Err(_) => {
            return bound_response(
                with_api_version(
                    error_response(&request_id, "query_failed"),
                    response_api_version,
                ),
                &request_id,
            );
        }
    };
    let service = HubService::new(store);
    let response = execute_operation(&service, &request_id, operation);
    bound_response(
        with_api_version(response, response_api_version),
        &request_id,
    )
}

fn execute_operation(service: &HubService, request_id: &str, operation: Operation) -> Vec<u8> {
    if operation.uses_owned_errors() {
        execute_owned_operation(service, request_id, operation)
    } else {
        execute_read_operation(service, request_id, operation)
    }
}

fn execute_read_operation(service: &HubService, request_id: &str, operation: Operation) -> Vec<u8> {
    match operation {
        Operation::SnapshotAtCursor => snapshot_response(request_id, service.snapshot_at_cursor()),
        Operation::ChangesAfter {
            after_cursor,
            limit,
        } => conversation_change_response(
            request_id,
            service.conversation_changes_after(after_cursor, limit),
        ),
        Operation::PromptPage {
            conversation_id,
            before,
            limit,
        } => query_response(
            request_id,
            service.conversation_prompt_page(&conversation_id, before.as_ref(), limit),
        ),
        Operation::BootstrapPage { cursor, limit } => conversation_query_response(
            request_id,
            service.conversation_bootstrap_page(cursor.as_ref(), limit),
        ),
        _ => error_response(request_id, "query_failed"),
    }
}

fn snapshot_response(
    request_id: &str,
    result: Result<crate::runtime_domain::HubSnapshotAtCursor, HubError>,
) -> Vec<u8> {
    result.map_or_else(
        |_| error_response(request_id, "query_failed"),
        |value| {
            if value.conversation_timestamps_json_safe() {
                success_response(request_id, snapshot_projection(value))
            } else {
                error_response(request_id, "query_failed")
            }
        },
    )
}

fn conversation_query_response<T>(request_id: &str, result: Result<T, HubError>) -> Vec<u8>
where
    T: Serialize + ConversationTimestampsJsonSafe,
{
    result.map_or_else(
        |_| error_response(request_id, "query_failed"),
        |value| {
            if value.conversation_timestamps_json_safe() {
                success_response(request_id, value)
            } else {
                error_response(request_id, "query_failed")
            }
        },
    )
}

fn query_response<T: Serialize>(request_id: &str, result: Result<T, HubError>) -> Vec<u8> {
    result.map_or_else(
        |_| error_response(request_id, "query_failed"),
        |value| success_response(request_id, value),
    )
}

fn conversation_change_response<T: Serialize + ConversationChangesJsonSafe>(
    request_id: &str,
    result: Result<T, HubError>,
) -> Vec<u8> {
    result.map_or_else(
        |_| error_response(request_id, "query_failed"),
        |value| {
            if value.conversation_changes_json_safe() {
                success_response(request_id, value)
            } else {
                error_response(request_id, "query_failed")
            }
        },
    )
}

#[cfg(test)]
#[path = "tests/runtime_rpc.rs"]
mod tests;
