use super::*;

fn client_instance_session_view() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap()
}

#[test]
fn remote_tui_scope_filter_matches_global_project_and_group_ids_exactly() {
    use crate::args::RemoteConversationScope;

    let global = json!({"scope": {"kind": "global"}});
    let project = json!({"scope": {"kind": "project", "id": "prj_1"}});
    let group = json!({"scope": {"kind": "group", "id": "grp_1"}});

    assert!(scope_filter_matches(
        &global,
        Some(&RemoteConversationScope::Global)
    ));
    assert!(scope_filter_matches(
        &project,
        Some(&RemoteConversationScope::Project("prj_1".into()))
    ));
    assert!(!scope_filter_matches(
        &project,
        Some(&RemoteConversationScope::Project("prj_2".into()))
    ));
    assert!(scope_filter_matches(
        &group,
        Some(&RemoteConversationScope::Group("grp_1".into()))
    ));
    assert!(!scope_filter_matches(
        &group,
        Some(&RemoteConversationScope::Group("grp_2".into()))
    ));
    assert!(scope_filter_matches(&global, None));
}

#[test]
fn remote_tui_filtered_selected_entry_is_labeled_as_usable_outside_the_list_filter() {
    let state = TuiState {
        selected_id: Some("c-1".into()),
        selected_entry: Some(super::super::OwnedConversationEntry {
            conversation: json!({
                "id": "c-1",
                "scope": {"kind": "global"},
                "title": "Selected"
            }),
            aggregate_version: 1,
        }),
        scope_filter: Some(crate::args::RemoteConversationScope::Project(
            "prj_1".into(),
        )),
        ..TuiState::default()
    };
    let mut writer = Vec::new();
    render(&state, &mut writer).unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("outside current list filter, still selected and openable"));
}

#[test]
fn remote_tui_instance_filter_reselects_visible_owner_session_and_clears_prompt_panel() {
    let view = client_instance_session_view();
    let mut state = TuiState {
        conversations: vec![
            super::super::OwnedConversationEntry {
                conversation: json!({
                    "id": "conversation-001",
                    "scope": {"kind": "global"},
                    "title": "Shared",
                    "created_at_ms": 1,
                    "updated_at_ms": 1
                }),
                aggregate_version: 1,
            },
            super::super::OwnedConversationEntry {
                conversation: json!({
                    "id": "conversation-002",
                    "scope": {"kind": "global"},
                    "title": "CLI only",
                    "created_at_ms": 2,
                    "updated_at_ms": 2
                }),
                aggregate_version: 1,
            },
        ],
        selected_id: Some("conversation-002".into()),
        client_instance_filter: Some("client-web-001".into()),
        client_instance_session_view_observed: Some(view),
        ..TuiState::default()
    };
    state.record_prompt_history(
        "conversation-002",
        &json!({
            "conversation_id": "conversation-002",
            "prompts": [{
                "id": "prompt-002",
                "conversation_id": "conversation-002",
                "role": "user",
                "content": "hidden from selected instance",
                "created_at_ms": 2
            }],
            "has_more": false
        }),
    );

    state.reconcile_client_instance_selection();

    assert_eq!(state.selected_id.as_deref(), Some("conversation-001"));
    assert!(state.prompt_history.is_empty());
    let mut output = Vec::new();
    render(&state, &mut output).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("Client-instance filter: \"client-web-001\""));
    assert!(output.contains("\"conversation-001\"  \"Shared\""));
    assert!(!output.contains("CLI only"));
    assert!(!output.contains("hidden from selected instance"));
}

#[tokio::test]
async fn remote_tui_instance_command_is_reachable_without_a_second_api_request() {
    let view = client_instance_session_view();
    let mut state = TuiState {
        conversations: vec![
            super::super::OwnedConversationEntry {
                conversation: json!({
                    "id": "conversation-001",
                    "scope": {"kind": "global"},
                    "title": "Shared",
                    "created_at_ms": 1,
                    "updated_at_ms": 1
                }),
                aggregate_version: 1,
            },
            super::super::OwnedConversationEntry {
                conversation: json!({
                    "id": "conversation-002",
                    "scope": {"kind": "global"},
                    "title": "CLI only",
                    "created_at_ms": 2,
                    "updated_at_ms": 2
                }),
                aggregate_version: 1,
            },
        ],
        selected_id: Some("conversation-002".into()),
        client_instance_session_view_observed: Some(view),
        ..TuiState::default()
    };
    let client = super::test_client("127.0.0.1:1".parse().unwrap());
    let mut output = Vec::new();
    let exited = super::super::commands::dispatch_command(
        &client,
        &mut state,
        None,
        "instance client-web-001",
        &mut output,
    )
    .await
    .unwrap();

    assert!(!exited);
    assert_eq!(state.selected_id.as_deref(), Some("conversation-001"));
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("Client-instance filter set to \"client-web-001\""));
}

#[tokio::test]
async fn remote_tui_instance_filter_blocks_hidden_prompt_before_http() {
    let mut state = TuiState {
        conversations: vec![super::super::OwnedConversationEntry {
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
    let client = super::test_client("127.0.0.1:1".parse().unwrap());
    let mut output = Vec::new();

    let exited = super::super::commands::dispatch_command(
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
        pending_prompt: Some(super::super::state::PendingPrompt {
            conversation_id: "conversation-002".into(),
            expected_version: 3,
            content: "retry must not cross the instance projection".into(),
            idempotency_key: "pending-prompt".into(),
        }),
        ..TuiState::default()
    };
    let client = super::test_client("127.0.0.1:1".parse().unwrap());
    let mut output = Vec::new();

    let exited =
        super::super::commands::dispatch_command(&client, &mut state, None, "retry", &mut output)
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
        conversations: vec![super::super::OwnedConversationEntry {
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
    let client = super::test_client("127.0.0.1:1".parse().unwrap());
    let mut output = Vec::new();

    let exited = super::super::commands::dispatch_command(
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
        conversations: vec![super::super::OwnedConversationEntry {
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
    let client = super::test_client("127.0.0.1:1".parse().unwrap());
    let mut output = Vec::new();

    let exited = super::super::commands::dispatch_command(
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
    let client = super::test_client("127.0.0.1:1".parse().unwrap());
    let mut output = Vec::new();

    let exited =
        super::super::commands::dispatch_command(&client, &mut state, None, "older", &mut output)
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

#[tokio::test]
async fn remote_tui_instance_filter_blocks_hidden_pending_run_intent_submit_before_http() {
    let mut state = TuiState {
        conversations: vec![super::super::OwnedConversationEntry {
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
    let client = super::test_client("127.0.0.1:1".parse().unwrap());
    let mut output = Vec::new();

    let exited = super::super::commands::dispatch_command(
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
        conversations: vec![super::super::OwnedConversationEntry {
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
    let client = super::test_client("127.0.0.1:1".parse().unwrap());
    let mut output = Vec::new();

    let exited = super::super::commands::dispatch_command(
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
        conversations: vec![super::super::OwnedConversationEntry {
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
    let client = super::test_client("127.0.0.1:1".parse().unwrap());
    let mut output = Vec::new();

    let exited = super::super::commands::dispatch_command(
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
        pending_run_intent: Some(super::super::state::PendingRunIntent {
            conversation_id: "conversation-002".into(),
            expected_version: 3,
            content: "retry must not cross the instance projection".into(),
            idempotency_key: "pending-run-intent".into(),
        }),
        ..TuiState::default()
    };
    let client = super::test_client("127.0.0.1:1".parse().unwrap());
    let mut output = Vec::new();

    let exited =
        super::super::commands::dispatch_command(&client, &mut state, None, "retry", &mut output)
            .await
            .unwrap();

    assert!(!exited);
    assert!(state.pending_run_intent.is_some());
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains(
        "Pending Run-intent blocked by client-instance display filter: conversation \"conversation-002\" is not declared for instance \"client-web-001\". No request was sent."
    ));
}

#[tokio::test]
async fn remote_tui_instance_filter_blocks_hidden_run_reads_before_http() {
    for command in ["runs", "timeline run-1", "run-observed run-1"] {
        let mut state = TuiState {
            conversations: vec![super::super::OwnedConversationEntry {
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
        let client = super::test_client("127.0.0.1:1".parse().unwrap());
        let mut output = Vec::new();

        let exited = super::super::commands::dispatch_command(
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
        conversations: vec![super::super::OwnedConversationEntry {
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
    let client = super::test_client("127.0.0.1:1".parse().unwrap());
    let mut output = Vec::new();

    let refreshed =
        super::super::sync::sync_selected_run_timeline(&client, &mut state, &mut output)
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
