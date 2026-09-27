use super::super::RemoteClient;
use reqwest::{Client, Url, redirect::Policy};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    thread,
    time::Duration,
};

pub(super) struct ExpectedRequest {
    pub request_prefix: &'static str,
    pub response_status: &'static str,
    pub response: Value,
    pub body: Option<Value>,
}

pub(super) fn fixture(name: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures")
        .join(format!("{name}.json"));
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

pub(super) fn request() -> Value {
    json!({
        "owner":{"issuer":"https://id.example","subject":"user-1","tenant_id":"tenant-1"},
        "conversation_id":"conversation-001","run_id":"run-001","attempt_id":"attempt-1","attempt_state":"accepted",
        "command":{"v":1,"command_id":"command-1","lease_proof":{"attempt_id":"attempt-1","target_id":"runner-a","epoch":3,"fencing_token":"token-a"},"idempotency_key":"run-1:attempt-1:command-1","workspace_ref":"workspace-1","argv":["forge-task","--prompt-ref","prompt-1"],"timeout_ms":5000,"max_output_bytes":65536},
        "transport":{"schema_version":"forge.runner-transport-admission/v1","evaluation_mode":"pure_runner_transport_admission","method":"POST","path":"/api/v1/runners/runner-a/dispatch","timestamp":300,"nonce":"nonce-a","payload_sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","payload_bytes":256,"replay_checked":true,"preview_only":true,"authority":{"identity_verified":false,"heartbeat_accepted":false,"lease_issued":false,"reservation_created":false,"execution_authorized":false,"dispatch_performed":false,"audit_published":false}},
        "expected_payload_sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "controls":{"effect_state":"not_started","cancellation_requested":false},"transition":"begin_starting"
    })
}

pub(super) fn response(request: &Value) -> Value {
    json!({
        "schema_version":"forge.runner-attempt-boundary/v1","evaluation_mode":"attempt_lifecycle_dispatch_boundary_preview",
        "owner":request["owner"].clone(),"conversation_id":"conversation-001","run_id":"run-001","attempt_id":"attempt-1","command_id":"command-1","target_id":request["command"]["lease_proof"]["target_id"].clone(),"lease_epoch":3,
        "current_attempt_state":"accepted","next_attempt_state":"starting","transition":"begin_starting","execution_boundary_ready":true,"attempt_transition_valid":true,"attempt_transition_dispatchable":true,"attempt_boundary_ready":true,"rejection_reasons":[],"preview_only":true,
        "authority":{"attempt_persisted":false,"reservation_created":false,"execution_authorized":false,"dispatch_performed":false,"audit_published":false}
    })
}

pub(super) fn spawn_mock_server(
    responses: Vec<ExpectedRequest>,
) -> (RemoteClient, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let address = listener.local_addr().expect("mock server address");
    let server = thread::spawn(move || {
        for expected in responses {
            let (mut stream, _) = listener.accept().expect("accept mock request");
            let (request, body) = capture_request(&mut stream);
            if let Some(expected_body) = expected.body {
                assert_eq!(
                    serde_json::from_slice::<Value>(&body).unwrap(),
                    expected_body
                );
            }
            assert!(
                request.starts_with(expected.request_prefix),
                "request={request:?} expected prefix={:?}",
                expected.request_prefix
            );
            write_json_response(&mut stream, expected.response_status, &expected.response);
        }
    });
    let http = Client::builder()
        .redirect(Policy::none())
        .timeout(Duration::from_secs(2))
        .build()
        .expect("mock HTTP client");
    let base_url = Url::parse(&format!("http://{address}")).expect("mock server URL");
    let client = RemoteClient {
        http,
        base_url,
        access_token: "test-token".into(),
        change_cursor: None,
        token_refresh: None,
    };
    (client, server)
}

fn capture_request(stream: &mut TcpStream) -> (String, Vec<u8>) {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("set mock read timeout");
    let mut reader = BufReader::new(stream.try_clone().expect("clone mock stream"));
    let mut line = String::new();
    reader.read_line(&mut line).expect("read request line");
    let mut content_length = 0usize;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).expect("read request header");
        if header == "\r\n" || header.is_empty() {
            break;
        }
        if let Some(value) = header.to_ascii_lowercase().strip_prefix("content-length:") {
            content_length = value.trim().parse().expect("content length");
        }
    }
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body).expect("read request body");
    (line.trim().to_owned(), body)
}

fn write_json_response(stream: &mut TcpStream, status: &str, response: &Value) {
    let body = serde_json::to_vec(response).expect("encode mock response");
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .expect("write mock response headers");
    stream.write_all(&body).expect("write mock response body");
}
