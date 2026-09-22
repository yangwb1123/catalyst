pub(super) fn serve_failed_refresh_keeps_session(listener: &TcpListener) {
    let (mut initial, _) = accept_stream(listener);
    let _ = read_request(&mut initial);
    respond(
        &mut initial,
        "200 OK",
        &json!({
            "conversations": [{
                "conversation": conversation_projection("c-1", "Keep me"),
                "aggregate_version": 3
            }],
            "next_after_id": null,
            "has_more": false
        }),
    );
    drop(initial);

    serve_failed_refresh_response(listener);
    serve_selected_history(listener);
}

pub(super) fn serve_failed_refresh_response(listener: &TcpListener) {
    for _ in 0..3 {
        let (mut failed_refresh, _) = accept_stream(listener);
        let (request, _, _) = read_request(&mut failed_refresh);
        assert!(request.starts_with("GET /api/v1/conversations?"));
        respond(
            &mut failed_refresh,
            "503 Service Unavailable",
            &json!({"code": "temporarily_unavailable"}),
        );
    }
}

pub(super) fn serve_transient_history_failure(listener: &TcpListener) {
    for _ in 0..3 {
        let (mut refresh, request, _, _) = accept_request(listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
        respond(
            &mut refresh,
            "503 Service Unavailable",
            &json!({"code": "temporarily_unavailable"}),
        );
    }
}

pub(super) fn serve_selected_history(listener: &TcpListener) {
    let (mut history, _) = accept_stream(listener);
    let (request, _, _) = read_request(&mut history);
    assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
    respond(
        &mut history,
        "200 OK",
        &json!({"conversation_id": "c-1", "prompts": [], "has_more": false}),
    );
}

#[cfg(unix)]
#[tokio::test]
async fn sync_reads_owner_changes_and_refreshes_the_selected_history() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (_config_root, cursor_store) = tui_checkpoint_store(address);
    let server = thread::spawn(move || serve_owner_sync(&listener));

    let mut client = test_client(address);
    client.change_cursor = Some(cursor_store.clone());
    let mut reader = Cursor::new("sync\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Synced 1 owner-visible changes through cursor 1"));
    assert!(output.contains("version 2"));
    assert!(output.contains("Prompt history refreshed for the selected session."));
    assert!(output.contains("from another client"));
    assert_eq!(cursor_store.load().unwrap(), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn sync_restores_owner_change_cursor_for_a_second_tui_process() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (_config_root, cursor_store) = tui_checkpoint_store(address);
    let server = thread::spawn(move || {
        serve_owner_sync(&listener);

        // A second TUI process must load the checkpoint before its first sync.
        // Keep the same owner/coordinator fixture and return an empty page at
        // the committed head, proving that the first change is not replayed.
        serve_conversation_page(&listener, &conversation_page(2));
        let (mut changes, request, _, _) = accept_request(&listener);
        assert!(request.starts_with(
            "GET /api/v1/conversation-changes?after_cursor=1&limit=128 "
        ));
        respond(
            &mut changes,
            "200 OK",
            &json!({
                "after_cursor": 1,
                "scanned_through_cursor": 1,
                "has_more": false,
                "changes": []
            }),
        );
        serve_conversation_page(&listener, &conversation_page(2));
        let (mut history, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
        respond(
            &mut history,
            "200 OK",
            &json!({
                "conversation_id": "c-1",
                "prompts": [],
                "has_more": false
            }),
        );
    });

    let mut first_client = test_client(address);
    first_client.change_cursor = Some(cursor_store.clone());
    let mut first_reader = Cursor::new("sync\nquit\n");
    let mut first_writer = Vec::new();
    run_with_io(&first_client, &mut first_reader, &mut first_writer)
        .await
        .unwrap();
    assert_eq!(cursor_store.load().unwrap(), 1);

    // Construct a fresh client and TUI state, as the second OS process would.
    let mut second_client = test_client(address);
    second_client.change_cursor = Some(cursor_store.clone());
    let mut second_reader = Cursor::new("sync\nquit\n");
    let mut second_writer = Vec::new();
    run_with_io(&second_client, &mut second_reader, &mut second_writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(second_writer).unwrap();
    assert!(
        output.contains("Synced 0 owner-visible changes through cursor 1"),
        "{output}"
    );
    assert!(!output.contains("from another client"), "{output}");
    assert_eq!(cursor_store.load().unwrap(), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn sync_drains_bounded_owner_change_pages_before_committing_cursor() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (_config_root, cursor_store) = tui_checkpoint_store(address);
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));

        let (mut first, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversation-changes?after_cursor=0&limit=128 "));
        respond(&mut first, "200 OK", &dense_change_page(0, 128, true));

        let (mut second, request, _, _) = accept_request(&listener);
        assert!(
            request.starts_with("GET /api/v1/conversation-changes?after_cursor=128&limit=128 ")
        );
        respond(&mut second, "200 OK", &dense_change_page(128, 1, false));

        serve_conversation_page(&listener, &conversation_page(2));
        let (mut history, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
        respond(
            &mut history,
            "200 OK",
            &json!({
                "conversation_id": "c-1",
                "prompts": [{
                    "id": "p-129",
                    "conversation_id": "c-1",
                    "role": "user",
                    "content": "from another client",
                    "created_at_ms": 129
                }],
                "has_more": false
            }),
        );
    });

    let mut client = test_client(address);
    client.change_cursor = Some(cursor_store.clone());
    let mut reader = Cursor::new("sync\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("Synced 129 owner-visible changes through cursor 129"),
        "{output}"
    );
    assert!(output.contains("version 2"), "{output}");
    assert!(output.contains("from another client"), "{output}");
    assert_eq!(cursor_store.load().unwrap(), 129);
}

#[cfg(unix)]
#[tokio::test]
async fn sync_keeps_the_cursor_when_a_later_change_page_fails() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (_config_root, cursor_store) = tui_checkpoint_store(address);
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));

        let (mut first, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversation-changes?after_cursor=0&limit=128 "));
        respond(&mut first, "200 OK", &dense_change_page(0, 128, true));

        for _ in 0..3 {
            let (mut second, request, _, _) = accept_request(&listener);
            assert!(
                request.starts_with("GET /api/v1/conversation-changes?after_cursor=128&limit=128 ")
            );
            respond(
                &mut second,
                "503 Service Unavailable",
                &json!({"code": "temporarily_unavailable"}),
            );
        }
    });

    let mut client = test_client(address);
    client.change_cursor = Some(cursor_store.clone());
    let mut reader = Cursor::new("sync\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("Sync change feed request failed: Forge API returned HTTP 503")
            && output.contains("The change cursor was not advanced."),
        "{output}"
    );
    assert_eq!(cursor_store.load().unwrap(), 0);
}

fn dense_change_page(after_cursor: u64, count: u64, has_more: bool) -> Value {
    let changes = (after_cursor + 1..=after_cursor + count)
        .map(|cursor| {
            json!({
                "cursor": cursor,
                "schema_version": 1,
                "conversation_id": "c-1",
                "entity_id": format!("p-{cursor}"),
                "aggregate_version": 2,
                "kind": "prompt_appended",
                "created_at_ms": cursor
            })
        })
        .collect::<Vec<_>>();
    json!({
        "after_cursor": after_cursor,
        "scanned_through_cursor": after_cursor + count,
        "has_more": has_more,
        "changes": changes
    })
}

#[cfg(unix)]
#[tokio::test]
async fn sync_keeps_the_cursor_when_selected_history_refresh_fails() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (_config_root, cursor_store) = tui_checkpoint_store(address);
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut initial_history, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
        respond(
            &mut initial_history,
            "200 OK",
            &json!({
                "conversation_id": "c-1",
                "prompts": [{"id": "p-1", "conversation_id": "c-1", "role": "user", "content": "keep this confirmed history", "created_at_ms": 10}],
                "has_more": false
            }),
        );
        let (mut changes, _, _, _) = accept_request(&listener);
        respond(&mut changes, "200 OK", &owner_change_page());
        serve_conversation_page(&listener, &conversation_page(2));
        for _ in 0..3 {
            let (mut history, request, _, _) = accept_request(&listener);
            assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
            respond(
                &mut history,
                "503 Service Unavailable",
                &json!({
                    "api_version": "forgeos.app-server/v1",
                    "code": "temporarily_unavailable",
                    "message": "retry"
                }),
            );
        }
    });
    let mut client = test_client(address);
    client.change_cursor = Some(cursor_store.clone());
    let mut reader = Cursor::new("open c-1\nsync\nquit\n");
    let mut writer = Vec::new();

    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    assert_eq!(cursor_store.load().unwrap(), 0);
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Sync Prompt history refresh failed: Forge API returned HTTP 503"));
    assert!(output.contains("keep this confirmed history"));
    server.join().unwrap();
}

#[cfg(unix)]
pub(super) fn tui_checkpoint_store(
    coordinator: std::net::SocketAddr,
) -> (
    tempfile::TempDir,
    super::super::super::credentials::ChangeCursorStore,
) {
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

pub(super) fn serve_owner_sync(listener: &TcpListener) {
    serve_conversation_page(listener, &conversation_page(1));
    let (mut changes, request, _, _) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/conversation-changes?after_cursor=0&limit=128 "));
    respond(&mut changes, "200 OK", &owner_change_page());
    drop(changes);
    serve_conversation_page(listener, &conversation_page(2));
    let (mut history, request, _, _) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
    respond(
        &mut history,
        "200 OK",
        &json!({
            "conversation_id": "c-1",
            "prompts": [{"id": "p-2", "conversation_id": "c-1", "role": "user", "content": "from another client", "created_at_ms": 20}],
            "has_more": false
        }),
    );
}

pub(super) fn conversation_page(version: u64) -> Value {
    json!({
        "conversations": [{
            "conversation": conversation_projection("c-1", "Shared"),
            "aggregate_version": version
        }],
        "next_after_id": null,
        "has_more": false
    })
}

pub(super) fn conversation_projection(id: &str, title: &str) -> Value {
    json!({
        "id": id,
        "scope": {"kind": "global"},
        "title": title,
        "created_at_ms": 1,
        "updated_at_ms": 1
    })
}

pub(super) fn owner_change_page() -> Value {
    json!({
        "after_cursor": 0,
        "scanned_through_cursor": 1,
        "has_more": false,
        "changes": [{
            "cursor": 1,
            "schema_version": 1,
            "conversation_id": "c-1",
            "entity_id": "p-2",
            "aggregate_version": 2,
            "kind": "prompt_appended",
            "created_at_ms": 20
        }]
    })
}
