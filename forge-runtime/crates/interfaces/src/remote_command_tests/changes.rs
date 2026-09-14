use super::*;

#[tokio::test]
async fn conversation_changes_sends_owner_feed_cursor_and_validates_dense_page() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversation-changes?after_cursor=4&limit=128 ",
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response: json!({
            "after_cursor": 4,
            "scanned_through_cursor": 5,
            "has_more": false,
            "changes": [{
                "cursor": 5,
                "schema_version": 1,
                "conversation_id": "c-1",
                "entity_id": "p-2",
                "aggregate_version": 2,
                "kind": "prompt_appended",
                "created_at_ms": 20
            }]
        }),
    }]);
    let page = client.conversation_changes_after(4).await.unwrap();
    assert_eq!(page.scanned_through_cursor, 5);
    assert_eq!(page.changes.len(), 1);
    assert_eq!(page.changes[0].conversation_id, "c-1");
    server.join().unwrap();
}
#[cfg(unix)]
#[tokio::test]
async fn default_change_list_resumes_from_and_commits_the_saved_cursor() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (_config_root, cursor_store) = checkpoint_store(address);
    cursor_store.save(4).unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request = capture_request(&mut stream);
        assert!(
            request
                .line
                .starts_with("GET /api/v1/conversation-changes?after_cursor=4&limit=128 ")
        );
        write_json_response(
            &mut stream,
            "200 OK",
            &json!({
                "after_cursor": 4,
                "scanned_through_cursor": 5,
                "has_more": false,
                "changes": [{
                    "cursor": 5,
                    "schema_version": 1,
                    "conversation_id": "c-1",
                    "entity_id": "p-1",
                    "aggregate_version": 2,
                    "kind": "prompt_appended",
                    "created_at_ms": 10
                }]
            }),
        );
    });
    let client = test_remote_client(address);
    let client = RemoteClient {
        change_cursor: Some(cursor_store.clone()),
        ..client
    };

    let page = client.resumed_conversation_changes(None).await.unwrap();
    assert_eq!(page.scanned_through_cursor, 5);
    assert_eq!(cursor_store.load().unwrap(), 5);
    server.join().unwrap();
}
#[cfg(unix)]
#[tokio::test]
async fn explicit_change_cursor_does_not_replace_the_saved_checkpoint() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (_config_root, cursor_store) = checkpoint_store(address);
    cursor_store.save(4).unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request = capture_request(&mut stream);
        assert!(
            request
                .line
                .starts_with("GET /api/v1/conversation-changes?after_cursor=1&limit=128 ")
        );
        write_json_response(
            &mut stream,
            "200 OK",
            &json!({"after_cursor": 1, "scanned_through_cursor": 1, "has_more": false, "changes": []}),
        );
    });
    let client = RemoteClient {
        change_cursor: Some(cursor_store.clone()),
        ..test_remote_client(address)
    };

    client.resumed_conversation_changes(Some(1)).await.unwrap();
    assert_eq!(cursor_store.load().unwrap(), 4);
    server.join().unwrap();
}
