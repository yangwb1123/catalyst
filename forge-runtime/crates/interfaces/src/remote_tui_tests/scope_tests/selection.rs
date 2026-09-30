use super::*;

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
        selected_entry: Some(super::super::super::OwnedConversationEntry {
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
            super::super::super::OwnedConversationEntry {
                conversation: json!({
                    "id": "conversation-001",
                    "scope": {"kind": "global"},
                    "title": "Shared",
                    "created_at_ms": 1,
                    "updated_at_ms": 1
                }),
                aggregate_version: 1,
            },
            super::super::super::OwnedConversationEntry {
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
    record_hidden_prompt_history(&mut state);

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
            super::super::super::OwnedConversationEntry {
                conversation: json!({
                    "id": "conversation-001",
                    "scope": {"kind": "global"},
                    "title": "Shared",
                    "created_at_ms": 1,
                    "updated_at_ms": 1
                }),
                aggregate_version: 1,
            },
            super::super::super::OwnedConversationEntry {
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
    let client = super::super::test_client("127.0.0.1:1".parse().unwrap());
    let mut output = Vec::new();
    let exited = super::super::super::commands::dispatch_command(
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

fn record_hidden_prompt_history(state: &mut TuiState) {
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
}
