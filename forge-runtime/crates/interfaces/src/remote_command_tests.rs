use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    thread,
    time::Duration,
};

use reqwest::{Client, Url, redirect::Policy};
use serde_json::{Value, json};

use super::RemoteError;
use super::client_auth::validated_issuer;
use super::credentials::{CredentialStore, StoredCredential};
use super::{
    OwnedConversationEntry, OwnedConversationPage, OwnedRunCursorResponse, OwnedRunPageResponse,
    OwnedRunSummaryResponse, OwnedRunTimelinePageResponse, RemoteClient, parse_api_url,
    resolve_access_token, validate_conversation_page, validate_owned_conversation_entry,
    validate_run_page, validate_run_page_request, validate_run_timeline, validate_timeline_request,
};
use crate::args::RemoteConversationScope;
use crate::runtime_domain::ConversationImportPrompt;

#[path = "remote_command_tests/changes.rs"]
mod changes;
#[path = "remote_command_tests/client_instance_views.rs"]
mod client_instance_views;
#[path = "remote_command_tests/config.rs"]
mod config;
#[path = "remote_command_tests/credential_candidate.rs"]
mod credential_candidate;
#[path = "remote_command_tests/execution_consent.rs"]
mod execution_consent;
#[path = "remote_command_tests/execution_reconciliation.rs"]
mod execution_reconciliation;
#[path = "remote_command_tests/imports.rs"]
mod imports;
#[path = "remote_command_tests/inventory_resource_convergence.rs"]
mod inventory_resource_convergence;
#[path = "remote_command_tests/lifecycle_registry.rs"]
mod lifecycle_registry;
#[path = "remote_command_tests/local_runner_preview.rs"]
mod local_runner_preview;
#[path = "remote_command_tests/pending_run_intent.rs"]
mod pending_run_intent;
#[path = "remote_command_tests/placement.rs"]
mod placement;
#[path = "remote_command_tests/prompt_writes.rs"]
mod prompt_writes;
#[path = "remote_command_tests/requests.rs"]
mod requests;
#[path = "remote_command_tests/run_attempt_lease_dispatch_preflight.rs"]
mod run_attempt_lease_dispatch_preflight;
#[path = "remote_command_tests/run_execution_evidence.rs"]
mod run_execution_evidence;
#[path = "remote_command_tests/runner_attempt_boundary.rs"]
mod runner_attempt_boundary;
#[path = "remote_command_tests/runner_dispatch_admission.rs"]
mod runner_dispatch_admission;
#[path = "remote_command_tests/runner_dispatch_plan_preview.rs"]
mod runner_dispatch_plan_preview;
#[path = "remote_command_tests/runner_execution_boundary.rs"]
mod runner_execution_boundary;
#[path = "remote_command_tests/runner_execution_intent.rs"]
mod runner_execution_intent;
#[path = "remote_command_tests/runner_transport_admission.rs"]
mod runner_transport_admission;
#[path = "remote_command_tests/runs.rs"]
mod runs;
#[path = "remote_command_tests/session_runner_receipt.rs"]
mod session_runner_receipt;
#[path = "remote_command_tests/session_runner_receipt_history.rs"]
mod session_runner_receipt_history;
#[path = "remote_command_tests/session_runner_reconciliation.rs"]
mod session_runner_reconciliation;
#[path = "remote_command_tests/validation.rs"]
mod validation;

#[cfg(unix)]
fn checkpoint_store(
    coordinator: std::net::SocketAddr,
) -> (tempfile::TempDir, super::credentials::ChangeCursorStore) {
    use std::os::unix::fs::PermissionsExt;

    let config_root = tempfile::tempdir_in(std::env::var("HOME").unwrap()).unwrap();
    fs::set_permissions(config_root.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let store = CredentialStore::for_test(config_root.path().to_path_buf());
    let credential = StoredCredential {
        issuer: "https://id.example".into(),
        client_id: "forge-cli".into(),
        subject: "user-a".into(),
        tenant_id: "tenant-a".into(),
        access_token: "saved-test-token".into(),
        expires_at_unix: 4_000_000_000,
    };
    store.save(&credential).unwrap();
    let cursor = store.change_cursor_store(&format!("http://{coordinator}/"), &credential);
    (config_root, cursor)
}

struct ExpectedRequest {
    request_prefix: &'static str,
    required_headers: &'static [&'static str],
    body_fields: Value,
    response_status: &'static str,
    response: Value,
}

struct CapturedRequest {
    line: String,
    headers: String,
    body: Vec<u8>,
}

fn spawn_mock_server(responses: Vec<ExpectedRequest>) -> (RemoteClient, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        for expected in responses {
            let (mut stream, _) = listener.accept().unwrap();
            let request = capture_request(&mut stream);
            assert!(request.line.starts_with(expected.request_prefix));
            assert!(request.headers.contains("authorization: bearer test-token"));
            for header in expected.required_headers {
                assert!(request.headers.contains(header));
            }
            assert_expected_body_fields(&request.body, &expected.body_fields);
            write_json_response(&mut stream, expected.response_status, &expected.response);
        }
    });

    (test_remote_client(address), server)
}

fn test_remote_client(address: std::net::SocketAddr) -> RemoteClient {
    RemoteClient {
        http: Client::builder()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap(),
        base_url: Url::parse(&format!("http://{address}")).unwrap(),
        access_token: "test-token".into(),
        change_cursor: None,
        token_refresh: None,
    }
}

fn capture_request(stream: &mut TcpStream) -> CapturedRequest {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let mut headers = String::new();
    let mut content_length = 0;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).unwrap();
        if header == "\r\n" || header.is_empty() {
            break;
        }
        let normalized = header.to_ascii_lowercase();
        if let Some(value) = normalized.strip_prefix("content-length:") {
            content_length = value.trim().parse::<usize>().unwrap();
        }
        headers.push_str(&normalized);
    }
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body).unwrap();
    CapturedRequest {
        line: line.trim().to_owned(),
        headers,
        body,
    }
}

fn assert_expected_body_fields(body: &[u8], expected: &Value) {
    if expected.is_null() {
        return;
    }
    let actual: Value = serde_json::from_slice(body).unwrap();
    for (field, value) in expected.as_object().unwrap() {
        assert_eq!(&actual[field], value, "unexpected request field {field}");
    }
}

fn write_json_response(stream: &mut TcpStream, status: &str, payload: &Value) {
    let bytes = serde_json::to_vec(payload).unwrap();
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        bytes.len()
    )
    .unwrap();
    stream.write_all(&bytes).unwrap();
}
