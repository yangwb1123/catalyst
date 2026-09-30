use super::*;

#[tokio::test]
async fn remote_tui_instance_filter_blocks_hidden_pending_run_intent_submit_before_http() {
    let mut state = TuiState {
        conversations: vec![super::super::super::OwnedConversationEntry {
            conversation: json!({
                "id": "conversation-002",
                "scope": {"kind": "global"},
                "title": "CLI only"
            }),
            aggregate_version: 3,
        }],
        selected_id: Some("conversation-002".into()),
        client_instance_filter: Some("client-web-001".into()),
        client_instance_session_view_observed: Some(client_instance_session_view()),
        ..TuiState::default()
    };
    let client = super::super::test_client("127.0.0.1:1".parse().unwrap());
    let mut output = Vec::new();

    let exited = super::super::super::commands::dispatch_command(
        &client,
        &mut state,
        None,
        "run-intents submit must not cross the instance projection",
        &mut output,
    )
    .await
    .unwrap();

    assert!(!exited);
    assert!(state.pending_run_intent.is_none());
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains(
        "Pending Run-intent blocked by client-instance display filter: conversation \"conversation-002\" is not declared for instance \"client-web-001\". No request was sent."
    ));
}

#[tokio::test]
async fn remote_tui_instance_filter_blocks_hidden_pending_run_intent_list_before_http() {
    let mut state = TuiState {
        conversations: vec![super::super::super::OwnedConversationEntry {
            conversation: json!({
                "id": "conversation-002",
                "scope": {"kind": "global"},
                "title": "CLI only"
            }),
            aggregate_version: 3,
        }],
        selected_id: Some("conversation-002".into()),
        client_instance_filter: Some("client-web-001".into()),
        client_instance_session_view_observed: Some(client_instance_session_view()),
        ..TuiState::default()
    };
    let client = super::super::test_client("127.0.0.1:1".parse().unwrap());
    let mut output = Vec::new();

    let exited = super::super::super::commands::dispatch_command(
        &client,
        &mut state,
        None,
        "run-intents",
        &mut output,
    )
    .await
    .unwrap();

    assert!(!exited);
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains(
        "Pending Run-intent blocked by client-instance display filter: conversation \"conversation-002\" is not declared for instance \"client-web-001\". No request was sent."
    ));
    assert!(!output.contains("Pending Run-intent list failed:"));
}

#[tokio::test]
async fn remote_tui_instance_filter_blocks_hidden_pending_run_intent_timeline_before_http() {
    let mut state = TuiState {
        conversations: vec![super::super::super::OwnedConversationEntry {
            conversation: json!({
                "id": "conversation-002",
                "scope": {"kind": "global"},
                "title": "CLI only"
            }),
            aggregate_version: 3,
        }],
        selected_id: Some("conversation-002".into()),
        client_instance_filter: Some("client-web-001".into()),
        client_instance_session_view_observed: Some(client_instance_session_view()),
        ..TuiState::default()
    };
    let client = super::super::test_client("127.0.0.1:1".parse().unwrap());
    let mut output = Vec::new();

    let exited = super::super::super::commands::dispatch_command(
        &client,
        &mut state,
        None,
        "run-intents timeline intent-1",
        &mut output,
    )
    .await
    .unwrap();

    assert!(!exited);
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains(
        "Pending Run-intent blocked by client-instance display filter: conversation \"conversation-002\" is not declared for instance \"client-web-001\". No request was sent."
    ));
    assert_eq!(state.pending_run_intent_timeline_sequence, 0);
}

#[tokio::test]
async fn remote_tui_instance_filter_blocks_hidden_pending_run_intent_retry_before_http() {
    let mut state = TuiState {
        client_instance_filter: Some("client-web-001".into()),
        client_instance_session_view_observed: Some(client_instance_session_view()),
        pending_run_intent: Some(super::super::super::state::PendingRunIntent {
            conversation_id: "conversation-002".into(),
            expected_version: 3,
            content: "retry must not cross the instance projection".into(),
            idempotency_key: "pending-run-intent".into(),
        }),
        ..TuiState::default()
    };
    let client = super::super::test_client("127.0.0.1:1".parse().unwrap());
    let mut output = Vec::new();

    let exited = super::super::super::commands::dispatch_command(
        &client,
        &mut state,
        None,
        "retry",
        &mut output,
    )
    .await
    .unwrap();

    assert!(!exited);
    assert!(state.pending_run_intent.is_some());
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains(
        "Pending Run-intent blocked by client-instance display filter: conversation \"conversation-002\" is not declared for instance \"client-web-001\". No request was sent."
    ));
}
