use super::*;

#[tokio::test]
async fn remote_tui_clears_owner_view_after_placement_authorization_failure() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || serve_authorization_failure(listener));

    let input = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(input.path(), PLACEMENT_INPUT).unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "placement-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("Placement preview request failed: Forge API returned HTTP 403"),
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
    assert!(!last_render.contains("Private session"), "{last_render}");
}

fn serve_authorization_failure(listener: TcpListener) {
    serve_conversation_page(
        &listener,
        &json!({
            "conversations": [{
                "conversation": {
                    "id": "c-1",
                    "scope": {"kind": "global"},
                    "title": "Private session",
                    "created_at_ms": 1,
                    "updated_at_ms": 1
                },
                "aggregate_version": 1
            }],
            "next_after_id": null,
            "has_more": false
        }),
    );
    let (mut stream, request, _, _) = accept_request(&listener);
    assert!(request.starts_with("POST /api/v1/device-placement/preview "));
    respond(&mut stream, "403 Forbidden", &json!({"code": "forbidden"}));
}
