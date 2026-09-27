use std::{io::Cursor, net::TcpListener, thread};

use serde_json::{Value, json};

use super::super::state::{TuiState, render};
use super::{
    helpers::{accept_request, conversation_page, respond, serve_conversation_page, test_client},
    run_with_io,
};

#[tokio::test]
async fn tui_loads_older_prompt_pages_without_repeating_the_cursor_row() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut newest, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?limit=128 "));
        respond(
            &mut newest,
            "200 OK",
            &json!({
                "conversation_id": "c-1",
                "prompts": [{"id": "p-new", "conversation_id": "c-1", "role": "user",
                    "content": "newer prompt", "created_at_ms": 200}],
                "next_cursor": {"created_at_ms": 200, "prompt_id": "p-new"},
                "has_more": true
            }),
        );

        let (mut older, request, _, _) = accept_request(&listener);
        assert!(request.starts_with(
            "GET /api/v1/conversations/c-1/prompts?limit=128&before_created_at_ms=200&before_prompt_id=p-new "
        ));
        respond(
            &mut older,
            "200 OK",
            &json!({
                "conversation_id": "c-1",
                "prompts": [{"id": "p-old", "conversation_id": "c-1", "role": "assistant",
                    "content": "older prompt", "created_at_ms": 100}],
                "has_more": false
            }),
        );
    });

    let client = test_client(address);
    let mut reader = Cursor::new("open c-1\nolder\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("newer prompt"));
    assert!(output.contains("Older Prompt history loaded."));
    assert!(output.contains("older prompt"));
    assert!(output.rfind("\"older prompt\"").unwrap() < output.rfind("\"newer prompt\"").unwrap());
}

#[test]
fn prompt_history_merges_pages_without_duplicates_and_renders_chronologically() {
    let mut state = TuiState {
        selected_id: Some("c-1".into()),
        ..TuiState::default()
    };
    state.record_prompt_history(
        "c-1",
        &json!({
            "conversation_id": "c-1",
            "prompts": [
                {"id": "p-3", "conversation_id": "c-1", "role": "user", "content": "last", "created_at_ms": 30},
                {"id": "p-2", "conversation_id": "c-1", "role": "assistant", "content": "middle", "created_at_ms": 20}
            ],
            "next_cursor": {"created_at_ms": 20, "prompt_id": "p-2"},
            "has_more": true
        }),
    );
    state.record_prompt_history(
        "c-1",
        &json!({
            "conversation_id": "c-1",
            "prompts": [
                {"id": "p-2", "conversation_id": "c-1", "role": "assistant", "content": "middle", "created_at_ms": 20},
                {"id": "p-1", "conversation_id": "c-1", "role": "user", "content": "first", "created_at_ms": 20}
            ],
            "has_more": false
        }),
    );

    let mut output = Vec::new();
    render(&state, &mut output).unwrap();
    let output = String::from_utf8(output).unwrap();
    let first = output.find("first").unwrap();
    let middle = output.find("middle").unwrap();
    let last = output.find("last").unwrap();
    assert!(first < middle && middle < last);
    assert_eq!(output.matches("middle").count(), 1);
    assert_eq!(state.prompt_history.len(), 3);
    assert!(state.history_before.is_none());
}

#[test]
fn prompt_history_is_cleared_when_the_selected_conversation_changes() {
    let mut state = TuiState {
        selected_id: Some("c-1".into()),
        ..TuiState::default()
    };
    state.record_prompt_history(
        "c-1",
        &json!({
            "conversation_id": "c-1",
            "prompts": [{"id": "p-1", "conversation_id": "c-1", "role": "user", "content": "private to c-1", "created_at_ms": 10}],
            "has_more": false
        }),
    );
    state.clear_prompt_history();
    state.selected_id = Some("c-2".into());

    let mut output = Vec::new();
    render(&state, &mut output).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("Prompt history for \"c-2\":"));
    assert!(!output.contains("private to c-1"));
}

#[test]
fn missing_client_instance_view_renders_an_empty_projection() {
    let mut state = TuiState {
        conversations: vec![
            super::super::OwnedConversationEntry {
                conversation: json!({
                    "id": "c-1",
                    "title": "Web session",
                    "scope": {"kind": "global"}
                }),
                aggregate_version: 1,
            },
            super::super::OwnedConversationEntry {
                conversation: json!({
                    "id": "c-2",
                    "title": "CLI session",
                    "scope": {"kind": "global"}
                }),
                aggregate_version: 1,
            },
        ],
        client_instance_filter: Some("client-web-001".into()),
        selected_id: Some("c-1".into()),
        ..TuiState::default()
    };

    state.reconcile_client_instance_selection();
    let mut output = Vec::new();
    render(&state, &mut output).unwrap();
    let output = String::from_utf8(output).unwrap();

    assert!(state.selected_id.is_none());
    assert!(output.contains("No sessions match this client-instance filter"));
    assert!(!output.contains("Web session"));
    assert!(!output.contains("CLI session"));
}

#[test]
fn mixed_client_instance_session_and_resource_views_fail_closed() {
    let session: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap();
    let mut resource: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json"
    ))
    .unwrap();
    resource["instances"][0]["observed_at_ms"] = json!(200501_u64);
    let mut state = TuiState {
        conversations: vec![super::super::OwnedConversationEntry {
            conversation: json!({
                "id": "conversation-001",
                "title": "Shared",
                "scope": {"kind": "global"}
            }),
            aggregate_version: 1,
        }],
        selected_id: Some("conversation-001".into()),
        client_instance_filter: Some("client-web-001".into()),
        client_instance_session_view_observed: Some(session),
        client_instance_resource_view_observed: Some(resource),
        history_loaded_for: Some("conversation-001".into()),
        prompt_history: vec![json!({
            "id": "prompt-001",
            "conversation_id": "conversation-001",
            "role": "user",
            "content": "must not survive mixed refresh",
            "created_at_ms": 1
        })],
        ..TuiState::default()
    };

    state.reconcile_client_instance_selection();

    assert!(state.active_client_instance_view().is_none());
    assert!(state.selected_id.is_none());
    assert!(state.prompt_history.is_empty());
}

#[test]
fn nonconverged_client_instance_observations_render_a_blocking_status_and_retain_metadata() {
    let session: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap();
    let resource: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json"
    ))
    .unwrap();
    let mut state = TuiState {
        client_instance_filter: Some("client-web-001".into()),
        client_instance_session_view_observed: Some(session),
        client_instance_resource_view_observed: Some(resource),
        ..TuiState::default()
    };

    state.mark_client_instance_observations_not_converged();

    assert!(state.client_instance_session_view_observed.is_some());
    assert!(state.client_instance_resource_view_observed.is_some());
    assert!(state.active_client_instance_view().is_none());
    let mut output = Vec::new();
    render(&state, &mut output).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains(
        "Client-instance observations are not converged; instance filtering and private reads through this client-instance projection are blocked"
    ));
    assert!(output.contains("Existing observation metadata is retained for display only"));
}

#[test]
fn revoking_one_client_instance_view_preserves_the_filter_and_private_clear() {
    let view: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap();
    let mut state = TuiState {
        conversations: vec![super::super::OwnedConversationEntry {
            conversation: json!({
                "id": "conversation-001",
                "title": "Web session",
                "scope": {"kind": "global"}
            }),
            aggregate_version: 1,
        }],
        client_instance_filter: Some("client-web-001".into()),
        client_instance_session_view_observed: Some(view),
        selected_id: Some("conversation-001".into()),
        history_loaded_for: Some("conversation-001".into()),
        prompt_history: vec![json!({
            "id": "prompt-001",
            "conversation_id": "conversation-001",
            "role": "user",
            "content": "private prompt",
            "created_at_ms": 1
        })],
        selected_run_id: Some("run-001".into()),
        selected_run_observed: Some(crate::runtime_domain::run_observed::RunObserved {
            api_version: "forge.run.observed.v1",
            owner_ref: "owner-ref".into(),
            conversation_id: "conversation-001".into(),
            run_id: "run-001".into(),
            prompt_id: "prompt-001".into(),
            created_at_ms: 1,
            latest_sequence: 1,
            status: "completed",
            metadata_observed: true,
            content_included: false,
            authority: Default::default(),
        }),
        ..TuiState::default()
    };

    assert!(state.clear_client_instance_view("session-view"));
    assert_eq!(
        state.client_instance_filter.as_deref(),
        Some("client-web-001")
    );
    assert!(state.client_instance_session_view_observed.is_none());
    assert!(state.selected_id.is_none());
    assert!(state.prompt_history.is_empty());
    assert!(state.selected_run_id.is_none());
    assert!(state.selected_run_observed.is_none());

    let mut output = Vec::new();
    render(&state, &mut output).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("No sessions match this client-instance filter"));
    assert!(!output.contains("Web session"));
    assert!(!output.contains("private prompt"));
}

#[test]
fn owner_view_clear_drops_explicit_observations() {
    let mut state = TuiState {
        device_inventory_observed: Some(
            json!({"schema_version": "forge.device-inventory-observation/v1"}),
        ),
        device_inventory_v2_observed: Some(
            json!({"schema_version": "forge.device-inventory-observation/v2"}),
        ),
        client_instance_session_view_observed: Some(
            json!({"schema_version": "forge.client-instance-session-view/v1"}),
        ),
        client_instance_resource_view_observed: Some(
            json!({"schema_version": "forge.client-instance-resource-view/v1"}),
        ),
        ..TuiState::default()
    };

    state.clear_remote_session_view();

    assert!(state.device_inventory_observed.is_none());
    assert!(state.device_inventory_v2_observed.is_none());
    assert!(state.client_instance_session_view_observed.is_none());
    assert!(state.client_instance_resource_view_observed.is_none());
}

#[test]
fn newest_prompt_refresh_preserves_the_oldest_loaded_page_cursor() {
    let mut state = TuiState {
        selected_id: Some("c-1".into()),
        ..TuiState::default()
    };
    state.record_prompt_history(
        "c-1",
        &json!({
            "conversation_id": "c-1",
            "prompts": [{"id": "p-old", "conversation_id": "c-1", "role": "user", "content": "old", "created_at_ms": 10}],
            "next_cursor": {"created_at_ms": 10, "prompt_id": "p-old"},
            "has_more": true
        }),
    );
    state.record_prompt_history(
        "c-1",
        &json!({
            "conversation_id": "c-1",
            "prompts": [{"id": "p-new", "conversation_id": "c-1", "role": "user", "content": "new", "created_at_ms": 20}],
            "next_cursor": {"created_at_ms": 20, "prompt_id": "p-new"},
            "has_more": true
        }),
    );

    let cursor = state.history_before.unwrap();
    assert_eq!(cursor.created_at_ms, 10);
    assert_eq!(cursor.prompt_id, "p-old");
    assert_eq!(state.prompt_history.len(), 2);
}
