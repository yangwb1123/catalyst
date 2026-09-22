use std::{io::Cursor, net::TcpListener, thread};

use serde_json::json;

use super::{
    helpers::{
        accept_request, conversation_projection, respond, serve_conversation_page, test_client,
    },
    run_with_io,
};

#[tokio::test]
async fn remote_tui_clears_owner_view_after_run_authorization_failure() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(
            &listener,
            &json!({
                "conversations": [{
                    "conversation": conversation_projection("c-1", "Private session"),
                    "aggregate_version": 3
                }],
                "next_after_id": null,
                "has_more": false
            }),
        );

        let (mut history, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
        respond(
            &mut history,
            "200 OK",
            &json!({
                "conversation_id": "c-1",
                "prompts": [{
                    "id": "p-1",
                    "conversation_id": "c-1",
                    "role": "user",
                    "content": "private prompt",
                    "created_at_ms": 1
                }],
                "has_more": false
            }),
        );

        let (mut runs, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/runs?"));
        respond(&mut runs, "403 Forbidden", &json!({"code": "forbidden"}));
    });

    let client = test_client(address);
    let mut reader = Cursor::new("open c-1\nruns\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("Run list failed: Forge API returned HTTP 403"),
        "{output}"
    );
    assert!(
        output.contains("Local session view cleared after authorization failure."),
        "{output}"
    );
    let last_render = output
        .rsplit("Forge shared sessions")
        .next()
        .unwrap_or_default();
    assert!(!last_render.contains("c-1"), "{last_render}");
    assert!(!last_render.contains("private prompt"), "{last_render}");
}
