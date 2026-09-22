use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    thread,
    time::Duration,
};

use reqwest::{Client, Url, redirect::Policy};
use serde_json::{Value, json};

use super::super::RemoteClient;
use super::{
    RemoteCommand, execute_pending_run_intent_command, execute_prompt_command, execute_run_command,
    execute_session_command,
};

struct ExpectedRequest {
    request_prefix: &'static str,
    response_status: &'static str,
    response: Value,
}

fn session_view() -> Value {
    serde_json::from_str(include_str!(
        "../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .expect("client-instance session view fixture")
}

fn session_view_path() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json")
        .display()
        .to_string()
}

fn spawn_mock_server(responses: Vec<ExpectedRequest>) -> (RemoteClient, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let address = listener.local_addr().expect("mock server address");
    let server = thread::spawn(move || {
        for expected in responses {
            let (mut stream, _) = listener.accept().expect("accept mock request");
            let request = capture_request(&mut stream);
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

fn capture_request(stream: &mut TcpStream) -> String {
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
    line.trim().to_owned()
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

#[tokio::test]
async fn hidden_instance_prompt_list_is_rejected_before_prompt_request() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/client-instances/session-view ",
        response_status: "200 OK",
        response: session_view(),
    }]);
    let command = RemoteCommand::PromptsList {
        conversation_id: "conversation-002".into(),
        before_created_at_ms: None,
        before_prompt_id: None,
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let error = execute_prompt_command(&client, &command, None, None)
        .await
        .expect_err("hidden Conversation must be rejected locally");
    assert!(error.to_string().contains("no Prompt request was sent"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn hidden_instance_session_detail_is_rejected_before_conversation_request() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/client-instances/session-view ",
        response_status: "200 OK",
        response: session_view(),
    }]);
    let command = RemoteCommand::SessionsShow {
        conversation_id: "conversation-002".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let error = execute_session_command(&client, &command, None)
        .await
        .expect_err("hidden Conversation must be rejected locally");
    assert!(
        error
            .to_string()
            .contains("no Conversation request was sent")
    );
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_session_detail_reads_after_projection_check() {
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/session-view ",
            response_status: "200 OK",
            response: session_view(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/conversations/conversation-001 ",
            response_status: "200 OK",
            response: json!({
                "conversation": {
                    "id": "conversation-001",
                    "title": "Visible",
                    "scope": {"kind": "global"},
                    "created_at_ms": 1,
                    "updated_at_ms": 1
                },
                "aggregate_version": 1
            }),
        },
    ]);
    let command = RemoteCommand::SessionsShow {
        conversation_id: "conversation-001".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let detail = execute_session_command(&client, &command, None)
        .await
        .expect("visible Conversation detail");
    assert_eq!(detail["conversation"]["id"], "conversation-001");
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_instance_session_detail_uses_supplied_view_without_candidate_request() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/conversation-001 ",
        response_status: "200 OK",
        response: json!({
            "conversation": {
                "id": "conversation-001",
                "title": "Visible",
                    "scope": {"kind": "global"},
                "created_at_ms": 1,
                "updated_at_ms": 1
            },
        "aggregate_version": 1
        }),
    }]);
    let command = RemoteCommand::SessionsShow {
        conversation_id: "conversation-001".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: Some(session_view_path()),
    };
    let detail = execute_session_command(&client, &command, None)
        .await
        .expect("local view should permit visible Conversation");
    assert_eq!(detail["conversation"]["id"], "conversation-001");
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_prompt_list_reads_after_projection_check() {
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/session-view ",
            response_status: "200 OK",
            response: session_view(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/conversations/conversation-001/prompts?",
            response_status: "200 OK",
            response: json!({
                "conversation_id": "conversation-001",
                "prompts": [],
                "has_more": false
            }),
        },
    ]);
    let command = RemoteCommand::PromptsList {
        conversation_id: "conversation-001".into(),
        before_created_at_ms: None,
        before_prompt_id: None,
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let page = execute_prompt_command(&client, &command, None, None)
        .await
        .expect("visible Conversation Prompt list");
    assert_eq!(page["conversation_id"], "conversation-001");
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_instance_prompt_list_uses_the_supplied_view_without_candidate_request() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/conversation-001/prompts?",
        response_status: "200 OK",
        response: json!({
            "conversation_id": "conversation-001",
            "prompts": [],
            "has_more": false
        }),
    }]);
    let command = RemoteCommand::PromptsList {
        conversation_id: "conversation-001".into(),
        before_created_at_ms: None,
        before_prompt_id: None,
        instance_id: Some("client-web-001".into()),
        instance_view: Some(session_view_path()),
    };
    let page = execute_prompt_command(&client, &command, None, None)
        .await
        .expect("local view should permit visible Conversation");
    assert_eq!(page["conversation_id"], "conversation-001");
    server.join().expect("mock server");
}

#[tokio::test]
async fn hidden_instance_prompt_add_is_rejected_before_prompt_request() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/client-instances/session-view ",
        response_status: "200 OK",
        response: session_view(),
    }]);
    let command = RemoteCommand::PromptsAdd {
        conversation_id: "conversation-002".into(),
        expected_version: 3,
        content: "must not cross instance projection".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let error = execute_prompt_command(&client, &command, Some("prompt-key"), None)
        .await
        .expect_err("hidden Conversation must be rejected locally");
    assert!(error.to_string().contains("no Prompt request was sent"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_prompt_add_posts_after_projection_check() {
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/session-view ",
            response_status: "200 OK",
            response: session_view(),
        },
        ExpectedRequest {
            request_prefix: "POST /api/v1/conversations/conversation-001/prompts ",
            response_status: "201 Created",
            response: json!({
                "aggregate_version": 4,
                "replayed": false
            }),
        },
    ]);
    let command = RemoteCommand::PromptsAdd {
        conversation_id: "conversation-001".into(),
        expected_version: 3,
        content: "visible prompt".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let receipt = execute_prompt_command(&client, &command, Some("prompt-key"), None)
        .await
        .expect("visible Conversation Prompt append");
    assert_eq!(receipt["aggregate_version"], 4);
    server.join().expect("mock server");
}

#[tokio::test]
async fn hidden_instance_pending_run_intent_list_is_rejected_before_private_read() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/client-instances/session-view ",
        response_status: "200 OK",
        response: session_view(),
    }]);
    let command = RemoteCommand::PendingRunIntentsList {
        conversation_id: "conversation-002".into(),
        limit: 25,
        before_submitted_at_ms: None,
        before_intent_id: None,
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let error = execute_pending_run_intent_command(&client, &command, None, None)
        .await
        .expect_err("hidden Conversation must be rejected before the Run-intent read");
    assert!(
        error
            .to_string()
            .contains("no pending Run-intent request was sent")
    );
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_pending_run_intent_list_reads_after_projection_check() {
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/session-view ",
            response_status: "200 OK",
            response: session_view(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/conversations/conversation-001/run-intents?limit=25 ",
            response_status: "200 OK",
            response: json!({
                "conversation_id": "conversation-001",
                "intents": [],
                "has_more": false
            }),
        },
    ]);
    let command = RemoteCommand::PendingRunIntentsList {
        conversation_id: "conversation-001".into(),
        limit: 25,
        before_submitted_at_ms: None,
        before_intent_id: None,
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let page = execute_pending_run_intent_command(&client, &command, None, None)
        .await
        .expect("visible Conversation pending Run-intent list");
    assert_eq!(page["conversation_id"], "conversation-001");
    server.join().expect("mock server");
}

#[tokio::test]
async fn hidden_instance_pending_run_intent_submit_is_rejected_before_post() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/client-instances/session-view ",
        response_status: "200 OK",
        response: session_view(),
    }]);
    let command = RemoteCommand::PendingRunIntentSubmit {
        conversation_id: "conversation-002".into(),
        expected_version: 3,
        content: "must not cross instance projection".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let error = execute_pending_run_intent_command(&client, &command, Some("intent-key"), None)
        .await
        .expect_err("hidden Conversation must be rejected before submit");
    assert!(
        error
            .to_string()
            .contains("no pending Run-intent request was sent")
    );
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_pending_run_intent_timeline_reads_after_projection_check() {
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/session-view ",
            response_status: "200 OK",
            response: session_view(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/conversations/conversation-001/run-intents/intent-1/timeline?after_sequence=0&limit=1 ",
            response_status: "200 OK",
            response: json!({
                "conversation_id": "conversation-001",
                "intent_id": "intent-1",
                "after_sequence": 0,
                "scanned_through_sequence": 1,
                "has_more": false,
                "events": [{
                    "event_id": "event-1",
                    "seq": 1,
                    "emitted_at_ms": 20,
                    "type": "submitted"
                }]
            }),
        },
    ]);
    let command = RemoteCommand::PendingRunIntentTimeline {
        conversation_id: "conversation-001".into(),
        intent_id: "intent-1".into(),
        after_sequence: 0,
        limit: 1,
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let page = execute_pending_run_intent_command(&client, &command, None, None)
        .await
        .expect("visible Conversation pending Run-intent timeline");
    assert_eq!(page["intent_id"], "intent-1");
    server.join().expect("mock server");
}

#[tokio::test]
async fn hidden_instance_run_reads_are_rejected_before_private_requests() {
    let commands = [
        RemoteCommand::RunsList {
            conversation_id: "conversation-002".into(),
            limit: 25,
            before_created_at_ms: None,
            before_run_id: None,
            instance_id: Some("client-web-001".into()),
            instance_view: None,
        },
        RemoteCommand::RunObserved {
            conversation_id: "conversation-002".into(),
            run_id: "run-1".into(),
            instance_id: Some("client-web-001".into()),
            instance_view: None,
        },
        RemoteCommand::RunTimeline {
            conversation_id: "conversation-002".into(),
            run_id: "run-1".into(),
            after_sequence: 0,
            limit: 128,
            resume: false,
            instance_id: Some("client-web-001".into()),
            instance_view: None,
        },
    ];

    for command in commands {
        let (client, server) = spawn_mock_server(vec![ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/session-view ",
            response_status: "200 OK",
            response: session_view(),
        }]);
        let error = execute_run_command(&client, &command)
            .await
            .expect_err("hidden Conversation must be rejected before the Run request");
        assert!(error.to_string().contains("no Run request was sent"));
        server.join().expect("mock server");
    }
}

#[tokio::test]
async fn local_instance_run_list_uses_the_supplied_view_without_candidate_request() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/conversation-001/runs?limit=25 ",
        response_status: "200 OK",
        response: json!({
            "conversation_id": "conversation-001",
            "runs": [],
            "has_more": false
        }),
    }]);
    let command = RemoteCommand::RunsList {
        conversation_id: "conversation-001".into(),
        limit: 25,
        before_created_at_ms: None,
        before_run_id: None,
        instance_id: Some("client-web-001".into()),
        instance_view: Some(session_view_path()),
    };
    let page = execute_run_command(&client, &command)
        .await
        .expect("local view should permit visible Run list");
    assert_eq!(page["conversation_id"], "conversation-001");
    server.join().expect("mock server");
}
