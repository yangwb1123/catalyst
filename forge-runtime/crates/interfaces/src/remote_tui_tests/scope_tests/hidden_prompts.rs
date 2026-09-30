use super::*;

#[tokio::test]
async fn remote_tui_instance_filter_blocks_hidden_prompt_before_http() {
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
        "prompt must not cross the instance projection",
        &mut output,
    )
    .await
    .unwrap();

    assert!(!exited);
    assert!(state.pending_prompt.is_none());
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains(
        "Prompt blocked by client-instance display filter: conversation \"conversation-002\" is not declared for instance \"client-web-001\". No request was sent."
    ));
    assert!(!output.contains("Prompt stored."));
}

#[tokio::test]
async fn remote_tui_instance_filter_blocks_hidden_pending_prompt_retry_before_http() {
    let mut state = TuiState {
        client_instance_filter: Some("client-web-001".into()),
        client_instance_session_view_observed: Some(client_instance_session_view()),
        pending_prompt: Some(super::super::super::state::PendingPrompt {
            conversation_id: "conversation-002".into(),
            expected_version: 3,
            content: "retry must not cross the instance projection".into(),
            idempotency_key: "pending-prompt".into(),
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
    let pending = state
        .pending_prompt
        .as_ref()
        .expect("pending Prompt retained");
    assert_eq!(pending.conversation_id, "conversation-002");
    assert_eq!(pending.idempotency_key, "pending-prompt");
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains(
        "Prompt blocked by client-instance display filter: conversation \"conversation-002\" is not declared for instance \"client-web-001\". No request was sent."
    ));
}

#[tokio::test]
async fn remote_tui_instance_filter_blocks_hidden_open_before_owner_or_prompt_http() {
    let mut state = TuiState {
        conversations: vec![super::super::super::OwnedConversationEntry {
            conversation: json!({
                "id": "conversation-002",
                "scope": {"kind": "global"},
                "title": "CLI only"
            }),
            aggregate_version: 3,
        }],
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
        "open conversation-002",
        &mut output,
    )
    .await
    .unwrap();

    assert!(!exited);
    assert!(state.selected_id.is_none());
    assert!(state.selected_entry.is_none());
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains(
        "Conversation read blocked by client-instance display filter: conversation \"conversation-002\" is not declared for instance \"client-web-001\". No request was sent."
    ));
    assert!(!output.contains("Opened session"));
}

#[tokio::test]
async fn remote_tui_instance_filter_blocks_hidden_detail_before_owner_http() {
    let mut state = TuiState {
        conversations: vec![super::super::super::OwnedConversationEntry {
            conversation: json!({
                "id": "conversation-002",
                "scope": {"kind": "global"},
                "title": "CLI only"
            }),
            aggregate_version: 3,
        }],
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
        "detail conversation-002",
        &mut output,
    )
    .await
    .unwrap();

    assert!(!exited);
    assert!(state.selected_id.is_none());
    assert!(state.selected_entry.is_none());
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains(
        "Conversation read blocked by client-instance display filter: conversation \"conversation-002\" is not declared for instance \"client-web-001\". No request was sent."
    ));
    assert!(!output.contains("Conversation detail:"));
}

#[tokio::test]
async fn remote_tui_instance_filter_blocks_hidden_older_history_before_prompt_http() {
    let mut state = TuiState {
        selected_id: Some("conversation-002".into()),
        history_loaded_for: Some("conversation-002".into()),
        history_before: Some(crate::args::PromptPageCursor {
            created_at_ms: 100,
            prompt_id: "prompt-002".into(),
        }),
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
        "older",
        &mut output,
    )
    .await
    .unwrap();

    assert!(!exited);
    assert_eq!(
        state.history_loaded_for.as_deref(),
        Some("conversation-002")
    );
    assert!(state.history_before.is_some());
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains(
        "Conversation read blocked by client-instance display filter: conversation \"conversation-002\" is not declared for instance \"client-web-001\". No request was sent."
    ));
    assert!(!output.contains("Older Prompt history loaded."));
}
