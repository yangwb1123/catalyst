use std::{io::Cursor, net::TcpListener, thread};

use serde_json::{Value, json};
use tempfile::NamedTempFile;

use super::{
    helpers::{accept_request, respond, serve_conversation_page, test_client},
    run_with_io,
};

#[tokio::test]
async fn remote_tui_posts_execution_reconciliation_for_selected_session() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-execution-reconciliation-observation-v1.json"
    ))
    .unwrap();
    let mut request = fixture["cases"][0]["input"].clone();
    request["conversation_id"] = json!("conversation-001");
    request["run_id"] = json!("run-001");
    let response = {
        let input: forge_runtime_domain::execution::reconciliation::ReconciliationInput =
            serde_json::from_value(request.clone()).unwrap();
        serde_json::to_value(
            forge_runtime_domain::execution::reconciliation::observe(input).unwrap(),
        )
        .unwrap()
    };
    let expected_request = request.clone();
    let expected_response = response.clone();
    let server = thread::spawn(move || {
        serve_conversation_page(
            &listener,
            &json!({
                "conversations": [{
                    "conversation": {
                        "id": "conversation-001",
                        "scope": {"kind": "global"},
                        "title": "Shared",
                        "created_at_ms": 1,
                        "updated_at_ms": 1
                    },
                    "aggregate_version": 1
                }],
                "next_after_id": null,
                "has_more": false
            }),
        );
        let (mut stream, request_line, headers, body) = accept_request(&listener);
        assert!(request_line.starts_with(
            "POST /api/v1/conversations/conversation-001/runs/run-001/execution-reconciliation/preview "
        ));
        assert!(headers.contains("authorization: bearer test-token"));
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            expected_request
        );
        respond(&mut stream, "200 OK", &expected_response);
    });
    let input = NamedTempFile::new().unwrap();
    std::fs::write(input.path(), serde_json::to_vec(&request).unwrap()).unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "execution-reconciliation-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "execution reconciliation preview [forge.execution-reconciliation-observation/v1]"
        ),
        "{output}"
    );
    assert!(
        output.contains("next_observation=await_terminal"),
        "{output}"
    );
    assert!(output.contains("automatic_retry=false"), "{output}");
    assert!(!output.contains("fence-1"));
    assert!(!output.contains("/api/v1/devices"));
}
