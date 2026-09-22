use super::*;

#[tokio::test]
async fn malformed_error_body_preserves_http_status_for_retry_decisions() {
    for status in ["401 Unauthorized", "503 Service Unavailable"] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let attempts = if status.starts_with("503") { 3 } else { 1 };
        let server = thread::spawn(move || {
            for _ in 0..attempts {
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
            }
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
fn conversation_timestamps_use_json_safe_integer_boundary() {
    const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
    let entry = |timestamp| OwnedConversationEntry {
        conversation: json!({
            "id": "c-001",
            "scope": {"kind": "global"},
            "title": "Session",
            "created_at_ms": timestamp,
            "updated_at_ms": timestamp,
        }),
        aggregate_version: 1,
    };
    let page = |timestamp| OwnedConversationPage {
        conversations: vec![entry(timestamp)],
        next_after_id: None,
        has_more: false,
    };

    assert!(validate_conversation_page(&page(MAX_SAFE_INTEGER), None).is_ok());
    assert!(validate_conversation_page(&page(MAX_SAFE_INTEGER + 1), None).is_err());

    let mut chronologically_invalid = page(10);
    chronologically_invalid.conversations[0].conversation["created_at_ms"] = json!(20);
    chronologically_invalid.conversations[0].conversation["updated_at_ms"] = json!(10);
    assert!(validate_conversation_page(&chronologically_invalid, None).is_err());
}

#[test]
fn conversation_aggregate_versions_use_json_safe_integer_boundary() {
    const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
    let conversation = json!({
        "id": "c-001",
        "scope": {"kind": "global"},
        "title": "Session",
        "created_at_ms": 1,
        "updated_at_ms": 1,
    });
    let mut page = OwnedConversationPage {
        conversations: vec![OwnedConversationEntry {
            conversation: conversation.clone(),
            aggregate_version: MAX_SAFE_INTEGER,
        }],
        next_after_id: None,
        has_more: false,
    };
    assert!(validate_conversation_page(&page, None).is_ok());
    page.conversations[0].aggregate_version = MAX_SAFE_INTEGER + 1;
    assert!(validate_conversation_page(&page, None).is_err());

    let detail = json!({
        "conversation": conversation,
        "aggregate_version": MAX_SAFE_INTEGER,
    });
    let entry = serde_json::from_value::<OwnedConversationEntry>(detail.clone()).unwrap();
    assert!(validate_owned_conversation_entry(&detail, &entry).is_ok());
    let mut unsafe_detail = detail;
    unsafe_detail["aggregate_version"] = json!(MAX_SAFE_INTEGER + 1);
    let unsafe_entry =
        serde_json::from_value::<OwnedConversationEntry>(unsafe_detail.clone()).unwrap();
    assert!(validate_owned_conversation_entry(&unsafe_detail, &unsafe_entry).is_err());
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

#[test]
fn shared_session_contract_fixture_is_accepted() {
    let Ok(fixture_path) = std::env::var("FORGE_SESSION_CONTRACT_FIXTURE") else {
        return;
    };
    let fixture: Value = serde_json::from_slice(&std::fs::read(fixture_path).unwrap()).unwrap();
    let envelope = fixture.as_object().unwrap();
    assert_eq!(envelope.len(), 5);
    assert!(envelope.keys().all(|key| {
        matches!(
            key.as_str(),
            "conversation_page"
                | "conversation_detail"
                | "prompt_page"
                | "change_page"
                | "append_prompt"
        )
    }));

    let page =
        serde_json::from_value::<OwnedConversationPage>(fixture["conversation_page"].clone())
            .unwrap();
    assert!(validate_conversation_page(&page, None).is_ok());
    assert_eq!(page.conversations.len(), 2);

    let detail =
        serde_json::from_value::<OwnedConversationEntry>(fixture["conversation_detail"].clone())
            .unwrap();
    assert!(validate_owned_conversation_entry(&fixture["conversation_detail"], &detail).is_ok());
    assert_eq!(detail.aggregate_version, 2);

    let prompt_page = &fixture["prompt_page"];
    assert!(
        super::super::prompt_page::validate_prompt_page(prompt_page, "conversation-001").is_ok()
    );

    let change_page = serde_json::from_value::<super::super::changes::OwnedConversationChangePage>(
        fixture["change_page"].clone(),
    )
    .unwrap();
    assert!(change_page.validate(0, 2).is_ok());

    let append = fixture["append_prompt"].as_object().unwrap();
    assert_eq!(append.len(), 3);
    assert_eq!(append["prompt"]["conversation_id"], "conversation-001");
    assert_eq!(append["prompt"]["role"], "user");
    assert_eq!(append["aggregate_version"], 3);
    assert_eq!(append["replayed"], false);
}

#[test]
fn run_observer_resume_contract_fixture_is_accepted() {
    let Ok(fixture_path) = std::env::var("FORGE_RUN_RESUME_CONTRACT_FIXTURE") else {
        return;
    };
    let fixture: Value = serde_json::from_slice(&std::fs::read(fixture_path).unwrap()).unwrap();
    let envelope = fixture.as_object().unwrap();
    assert_eq!(envelope.len(), 6);
    assert_eq!(
        fixture["api_version"],
        "forgeos.run-observer-resume-contract/v1"
    );
    let conversation_id = fixture["conversation_id"].as_str().unwrap();
    let run_id = fixture["run_id"].as_str().unwrap();
    let limit = usize::try_from(fixture["limit"].as_u64().unwrap()).unwrap();
    let pages = fixture["pages"].as_array().unwrap();
    assert_eq!(conversation_id, "conversation-001");
    assert_eq!(run_id, "run-001");
    assert_eq!(limit, 2);
    assert_eq!(pages.len(), 3);

    let mut after_sequence = 0;
    let mut sequences = Vec::new();
    for page in pages {
        let decoded: OwnedRunTimelinePageResponse = serde_json::from_value(page.clone()).unwrap();
        validate_run_timeline(&decoded, conversation_id, run_id, after_sequence, limit).unwrap();
        assert_eq!(decoded.after_sequence, after_sequence);
        sequences.extend(decoded.events.into_iter().map(|event| event.seq));
        after_sequence = decoded.scanned_through_sequence;
    }
    let expected = fixture["expected_sequences"].as_array().unwrap();
    assert_eq!(sequences.len(), expected.len());
    for (sequence, expected) in sequences.iter().zip(expected) {
        assert_eq!(Some(*sequence), expected.as_u64());
    }
    assert_eq!(after_sequence, 5);
}

#[test]
fn run_request_and_response_numbers_are_limited_to_json_safe_integer() {
    use crate::runtime_domain::OwnedRunStatus;
    const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

    assert!(validate_run_page_request(1, Some(MAX_SAFE_INTEGER), Some("run-1")).is_ok());
    assert!(validate_run_page_request(1, Some(MAX_SAFE_INTEGER + 1), Some("run-1")).is_err());
    assert!(validate_timeline_request(MAX_SAFE_INTEGER, 1).is_ok());
    assert!(validate_timeline_request(MAX_SAFE_INTEGER + 1, 1).is_err());

    let safe_run = OwnedRunSummaryResponse {
        run_id: "run-1".into(),
        prompt_id: "prompt-1".into(),
        created_at_ms: MAX_SAFE_INTEGER,
        latest_sequence: MAX_SAFE_INTEGER,
        status: OwnedRunStatus::Completed,
    };
    let safe_page = OwnedRunPageResponse {
        conversation_id: "conversation-1".into(),
        runs: vec![safe_run.clone()],
        next_cursor: Some(OwnedRunCursorResponse {
            created_at_ms: MAX_SAFE_INTEGER,
            run_id: "run-1".into(),
        }),
        has_more: true,
    };
    assert!(validate_run_page(&safe_page, "conversation-1", 1, None, None).is_ok());

    for oversized in [
        OwnedRunSummaryResponse {
            created_at_ms: MAX_SAFE_INTEGER + 1,
            ..safe_run.clone()
        },
        OwnedRunSummaryResponse {
            latest_sequence: MAX_SAFE_INTEGER + 1,
            ..safe_run.clone()
        },
    ] {
        let mut page = safe_page.clone();
        page.runs = vec![oversized];
        page.next_cursor = None;
        page.has_more = false;
        assert!(validate_run_page(&page, "conversation-1", 1, None, None).is_err());
    }

    let mut duplicate_run_ids = safe_page.clone();
    duplicate_run_ids.has_more = false;
    duplicate_run_ids.next_cursor = None;
    duplicate_run_ids.runs.push(OwnedRunSummaryResponse {
        created_at_ms: MAX_SAFE_INTEGER - 1,
        ..safe_run.clone()
    });
    assert!(validate_run_page(&duplicate_run_ids, "conversation-1", 2, None, None).is_err());

    let mut zero_sequence = safe_page.clone();
    zero_sequence.runs[0].latest_sequence = 0;
    zero_sequence.next_cursor = None;
    zero_sequence.has_more = false;
    assert!(validate_run_page(&zero_sequence, "conversation-1", 1, None, None).is_err());

    let mut oversized_cursor = safe_page.clone();
    oversized_cursor.next_cursor.as_mut().unwrap().created_at_ms = MAX_SAFE_INTEGER + 1;
    assert!(validate_run_page(&oversized_cursor, "conversation-1", 1, None, None).is_err());

    let safe_timeline = OwnedRunTimelinePageResponse {
        conversation_id: "conversation-1".into(),
        run_id: "run-1".into(),
        after_sequence: MAX_SAFE_INTEGER,
        scanned_through_sequence: MAX_SAFE_INTEGER,
        has_more: false,
        events: vec![],
    };
    assert!(
        validate_run_timeline(
            &safe_timeline,
            "conversation-1",
            "run-1",
            MAX_SAFE_INTEGER,
            1,
        )
        .is_ok()
    );
    let mut oversized_timeline = safe_timeline;
    oversized_timeline.scanned_through_sequence = MAX_SAFE_INTEGER + 1;
    assert!(
        validate_run_timeline(
            &oversized_timeline,
            "conversation-1",
            "run-1",
            MAX_SAFE_INTEGER,
            1,
        )
        .is_err()
    );
}
