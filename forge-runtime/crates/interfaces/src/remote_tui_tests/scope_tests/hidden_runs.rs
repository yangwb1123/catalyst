use super::*;

#[tokio::test]
async fn remote_tui_instance_filter_blocks_hidden_run_reads_before_http() {
    for command in ["runs", "timeline run-1", "run-observed run-1"] {
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
            command,
            &mut output,
        )
        .await
        .unwrap();

        assert!(!exited);
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains(
            "Conversation read blocked by client-instance display filter: conversation \"conversation-002\" is not declared for instance \"client-web-001\". No request was sent."
        ));
        assert!(!output.contains("Run list failed:"));
        assert!(!output.contains("Run timeline failed:"));
        assert!(!output.contains("Run observed request failed:"));
    }
}

#[tokio::test]
async fn remote_tui_instance_filter_blocks_hidden_run_sync_before_private_reads() {
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
        selected_run_id: Some("run-1".into()),
        run_timeline_sequence: 7,
        ..TuiState::default()
    };
    let client = super::super::test_client("127.0.0.1:1".parse().unwrap());
    let mut output = Vec::new();

    let refreshed =
        super::super::super::sync::sync_selected_run_timeline(&client, &mut state, &mut output)
            .await
            .unwrap();

    assert!(refreshed);
    assert!(state.selected_run_id.is_none());
    assert_eq!(state.run_timeline_sequence, 0);
    assert!(state.selected_run_observed.is_none());
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains(
        "Conversation read blocked by client-instance display filter: conversation \"conversation-002\" is not declared for instance \"client-web-001\". No request was sent."
    ));
}
