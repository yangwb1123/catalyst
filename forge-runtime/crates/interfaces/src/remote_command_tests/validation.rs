use super::*;

#[tokio::test]
async fn malformed_error_body_preserves_http_status_for_retry_decisions() {
    for status in ["401 Unauthorized", "503 Service Unavailable"] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            assert!(request_line.starts_with("GET "));
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header == "\r\n" || header.is_empty() {
                    break;
                }
            }
            let body = b"not json";
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(body).unwrap();
        });
        let client = RemoteClient {
            http: Client::builder()
                .timeout(Duration::from_secs(2))
                .build()
                .unwrap(),
            base_url: Url::parse(&format!("http://{address}")).unwrap(),
            access_token: "test-token".into(),
            change_cursor: None,
            token_refresh: None,
        };
        let error = client.list_prompts("c-1", None).await.unwrap_err();
        let status_code = status[..3].parse::<u16>().unwrap();
        assert_eq!(
            error.to_string(),
            format!("Forge API returned HTTP {status_code} (request_failed)")
        );
        server.join().unwrap();
    }
}
#[test]
fn conversation_page_cursor_and_order_are_validated() {
    let conversations = (0..128)
        .map(|index| OwnedConversationEntry {
            conversation: json!({
                "id": format!("c-{index:03}"),
                "scope": {"kind": "global"},
                "title": "Session",
                "created_at_ms": index,
                "updated_at_ms": index,
            }),
            aggregate_version: 1,
        })
        .collect::<Vec<_>>();
    let mut page = OwnedConversationPage {
        conversations,
        next_after_id: Some("c-127".into()),
        has_more: true,
    };
    assert!(validate_conversation_page(&page, None).is_ok());
    page.next_after_id = Some("c-126".into());
    assert!(validate_conversation_page(&page, None).is_err());
    page.next_after_id = Some("c-127".into());
    assert!(validate_conversation_page(&page, Some("c-000")).is_err());
}

#[test]
fn conversation_page_rejects_unknown_response_fields() {
    let response = json!({
        "conversations": [{
            "conversation": {
                "id": "c-001",
                "scope": {"kind": "global"},
                "title": "Session",
                "created_at_ms": 1,
                "updated_at_ms": 1,
                "unexpected": true,
            },
            "aggregate_version": 1,
        }],
        "has_more": false,
    });
    let page = serde_json::from_value::<OwnedConversationPage>(response.clone()).unwrap();
    assert!(validate_conversation_page(&page, None).is_err());

    let mut unexpected_entry_field = response.clone();
    unexpected_entry_field["conversations"][0]["unexpected"] = json!(true);
    assert!(serde_json::from_value::<OwnedConversationPage>(unexpected_entry_field).is_err());

    let mut unexpected_page_field = response;
    unexpected_page_field["unexpected"] = json!(true);
    assert!(serde_json::from_value::<OwnedConversationPage>(unexpected_page_field).is_err());
}

#[test]
fn conversation_contract_fixture_is_accepted() {
    let Ok(fixture_path) = std::env::var("FORGE_CONTRACT_FIXTURE") else {
        return;
    };
    let fixture = std::fs::read(fixture_path).unwrap();
    let page = serde_json::from_slice::<OwnedConversationPage>(&fixture).unwrap();
    assert!(validate_conversation_page(&page, None).is_ok());
    assert_eq!(page.conversations.len(), 2);
    assert_eq!(
        page.conversations[0]
            .conversation
            .get("scope")
            .and_then(|scope| scope.get("kind"))
            .and_then(Value::as_str),
        Some("project")
    );
}
