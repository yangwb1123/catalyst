use super::*;

use std::time::{SystemTime, UNIX_EPOCH};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::{Client, Url, redirect::Policy};
use serde_json::Value;

#[tokio::test]
async fn transient_prompt_write_is_not_automatically_replayed() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/c-1/prompts ",
        required_headers: &["idempotency-key: prompt-key"],
        body_fields: json!({"content": "write once", "expected_version": 7}),
        response_status: "503 Service Unavailable",
        response: json!({"code": "temporarily_unavailable"}),
    }]);
    let error = client
        .append_prompt("c-1", 7, "write once", "prompt-key")
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Forge API returned HTTP 503 (temporarily_unavailable)"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn prompt_receipt_binds_backend_prompt_before_projection() {
    let token = prompt_receipt_test_token();
    let (client, server) = spawn_prompt_receipt_server(&token, prompt_receipt_response());
    let receipt = client
        .append_prompt_receipt("conversation-1", 2, "send this", "prompt-key")
        .await
        .unwrap();
    assert_eq!(receipt["owner"]["subject"], "user-1");
    assert_eq!(receipt["request"]["conversation_id"], "conversation-1");
    assert_eq!(receipt["receipt"]["prompt_id"], "prompt-1");
    assert_eq!(receipt["receipt"]["aggregate_version"], 3);
    assert_eq!(receipt["receipt"]["content_included"], false);
    assert!(
        !serde_json::to_string(&receipt)
            .unwrap()
            .contains("send this")
    );
    server.join().unwrap();

    let mut cases = Vec::new();
    let mut foreign_conversation = prompt_receipt_response();
    foreign_conversation["prompt"]["conversation_id"] =
        Value::String("conversation-foreign".into());
    cases.push(foreign_conversation);
    let mut role_drift = prompt_receipt_response();
    role_drift["prompt"]["role"] = Value::String("assistant".into());
    cases.push(role_drift);
    let mut content_drift = prompt_receipt_response();
    content_drift["prompt"]["content"] = Value::String("foreign content".into());
    cases.push(content_drift);
    let mut identity_drift = prompt_receipt_response();
    identity_drift["prompt"]["id"] = Value::String(String::new());
    cases.push(identity_drift);
    let mut timestamp_drift = prompt_receipt_response();
    timestamp_drift["prompt"]["created_at_ms"] = Value::from(9_007_199_254_740_992_u64);
    cases.push(timestamp_drift);
    let mut cas_drift = prompt_receipt_response();
    cas_drift["aggregate_version"] = Value::from(4_u64);
    cases.push(cas_drift);
    let mut replay_marker_drift = prompt_receipt_response();
    replay_marker_drift["replayed"] = Value::String("false".into());
    cases.push(replay_marker_drift);

    for response in cases {
        let (client, server) = spawn_prompt_receipt_server(&token, response);
        let error = client
            .append_prompt_receipt("conversation-1", 2, "send this", "prompt-key")
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Forge API returned an invalid Prompt")
        );
        server.join().unwrap();
    }
}

fn prompt_receipt_response() -> Value {
    json!({
        "prompt": {
            "id": "prompt-1",
            "conversation_id": "conversation-1",
            "role": "user",
            "content": "send this",
            "created_at_ms": 300,
        },
        "aggregate_version": 3,
        "replayed": false,
    })
}

fn prompt_receipt_test_token() -> String {
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
    let payload = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&json!({
            "iss": "https://id.example",
            "client_id": "forge-cli",
            "sub": "user-1",
            "tenant_id": "tenant-1",
            "exp": SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() + 3600,
            "aud": ["forge-api"],
            "scopes": ["forge:conversations:read", "forge:conversations:write"],
        }))
        .unwrap(),
    );
    format!("{header}.{payload}.signature")
}

fn spawn_prompt_receipt_server(
    token: &str,
    response: Value,
) -> (RemoteClient, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let expected_token = token.to_owned();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request = capture_request(&mut stream);
        assert!(
            request
                .line
                .starts_with("POST /api/v1/conversations/conversation-1/prompts ")
        );
        assert!(request.headers.contains(&format!(
            "authorization: bearer {}",
            expected_token.to_ascii_lowercase()
        )));
        assert_expected_body_fields(
            &request.body,
            &json!({"content": "send this", "expected_version": 2}),
        );
        write_json_response(&mut stream, "200 OK", &response);
    });
    let client = RemoteClient {
        http: Client::builder()
            .redirect(Policy::none())
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap(),
        base_url: Url::parse(&format!("http://{address}")).unwrap(),
        access_token: token.to_owned(),
        change_cursor: None,
        token_refresh: None,
    };
    (client, server)
}
