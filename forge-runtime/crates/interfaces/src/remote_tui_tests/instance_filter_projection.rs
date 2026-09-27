use std::{net::TcpListener, thread};

use serde_json::{json, Value};

use super::super::state::TuiState;
use super::super::{
    state::{PendingCreate, PendingPrompt, PendingRunIntent, PendingRunIntentPageObservation},
    OwnedConversationEntry,
};
use super::helpers::{accept_request, respond, test_client};

fn shared_state_with_private_projection() -> TuiState {
    let session_view = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap();
    let resource_view = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json"
    ))
    .unwrap();
    let conversation = json!({
        "id": "conversation-001",
        "scope": {"kind": "global"},
        "title": "Shared",
        "created_at_ms": 1,
        "updated_at_ms": 1
    });
    TuiState {
        conversations: vec![OwnedConversationEntry {
            conversation: conversation.clone(),
            aggregate_version: 7,
        }],
        selected_id: Some("conversation-001".into()),
        selected_entry: Some(OwnedConversationEntry {
            conversation,
            aggregate_version: 7,
        }),
        client_instance_filter: Some("client-web-001".into()),
        history_loaded_for: Some("conversation-001".into()),
        prompt_history: vec![json!({
            "id": "prompt-001",
            "conversation_id": "conversation-001",
            "role": "user",
            "content": "private prompt",
            "created_at_ms": 1
        })],
        history_before: Some(crate::args::PromptPageCursor {
            created_at_ms: 1,
            prompt_id: "prompt-001".into(),
        }),
        pending_prompt: Some(PendingPrompt {
            conversation_id: "conversation-001".into(),
            expected_version: 7,
            content: "pending prompt".into(),
            idempotency_key: "pending-prompt".into(),
        }),
        pending_run_intent: Some(PendingRunIntent {
            conversation_id: "conversation-001".into(),
            expected_version: 7,
            content: "pending run".into(),
            idempotency_key: "pending-run".into(),
        }),
        pending_create: Some(PendingCreate {
            title: "pending create".into(),
            scope: crate::args::RemoteConversationScope::Global,
            idempotency_key: "pending-create".into(),
        }),
        selected_run_id: Some("run-001".into()),
        run_timeline_sequence: 9,
        selected_run_observed: Some(crate::runtime_domain::run_observed::RunObserved {
            api_version: "forge.run.observed.v1",
            owner_ref: "owner-ref".into(),
            conversation_id: "conversation-001".into(),
            run_id: "run-001".into(),
            prompt_id: "prompt-001".into(),
            created_at_ms: 1,
            latest_sequence: 9,
            status: "completed",
            metadata_observed: true,
            content_included: false,
            authority: Default::default(),
        }),
        device_inventory_observed: Some(
            json!({"schema_version": "forge.device-inventory-observation/v1"}),
        ),
        device_inventory_v2_observed: Some(
            json!({"schema_version": "forge.device-inventory-observation/v2"}),
        ),
        pending_run_intent_page: Some(PendingRunIntentPageObservation {
            conversation_id: "conversation-001".into(),
            before_submitted_at_ms: Some(10),
            before_intent_id: Some("intent-001".into()),
        }),
        selected_pending_run_intent_conversation_id: Some("conversation-001".into()),
        selected_pending_run_intent_id: Some("intent-001".into()),
        pending_run_intent_timeline_sequence: 4,
        client_instance_session_view_observed: Some(session_view),
        client_instance_resource_view_observed: Some(resource_view),
        ..TuiState::default()
    }
}

fn assert_private_projection_cleared(state: &TuiState) {
    assert!(state.history_loaded_for.is_none());
    assert!(state.prompt_history.is_empty());
    assert!(state.history_before.is_none());
    assert!(state.selected_run_id.is_none());
    assert_eq!(state.run_timeline_sequence, 0);
    assert!(state.selected_run_observed.is_none());
    assert!(state.pending_run_intent_page.is_none());
    assert!(state.selected_pending_run_intent_conversation_id.is_none());
    assert!(state.selected_pending_run_intent_id.is_none());
    assert_eq!(state.pending_run_intent_timeline_sequence, 0);
}

fn assert_owner_state_and_pending_writes_preserved(state: &TuiState) {
    assert_eq!(state.selected_id.as_deref(), Some("conversation-001"));
    assert_eq!(state.conversations.len(), 1);
    assert_eq!(
        state
            .selected_entry
            .as_ref()
            .and_then(|entry| entry.conversation.get("id"))
            .and_then(serde_json::Value::as_str),
        Some("conversation-001")
    );
    assert!(state.device_inventory_observed.is_some());
    assert!(state.device_inventory_v2_observed.is_some());
    assert!(state.client_instance_session_view_observed.is_some());
    assert!(state.client_instance_resource_view_observed.is_some());
    assert!(state.pending_prompt.is_some());
    assert!(state.pending_run_intent.is_some());
    assert!(state.pending_create.is_some());
}

#[tokio::test]
async fn remote_tui_shared_instance_filter_change_clears_private_projection_without_http() {
    let client = test_client("127.0.0.1:1".parse().unwrap());
    let mut state = shared_state_with_private_projection();
    let mut output = Vec::new();

    let exited = super::super::commands::dispatch_command(
        &client,
        &mut state,
        None,
        "instance client-cli-001",
        &mut output,
    )
    .await
    .unwrap();

    assert!(!exited);
    assert_eq!(
        state.client_instance_filter.as_deref(),
        Some("client-cli-001")
    );
    assert_private_projection_cleared(&state);
    assert_owner_state_and_pending_writes_preserved(&state);
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "Client-instance filter set to \"client-cli-001\"; this is a local display projection over unverified session_ids, not authorization or device identity.\n"
    );
}

#[tokio::test]
async fn remote_tui_instance_filter_clear_and_scope_change_clear_private_projection() {
    let client = test_client("127.0.0.1:1".parse().unwrap());

    let mut clear_state = shared_state_with_private_projection();
    let mut clear_output = Vec::new();
    super::super::commands::dispatch_command(
        &client,
        &mut clear_state,
        None,
        "instance clear",
        &mut clear_output,
    )
    .await
    .unwrap();
    assert!(clear_state.client_instance_filter.is_none());
    assert!(clear_state.scope_filter.is_none());
    assert_private_projection_cleared(&clear_state);
    assert_owner_state_and_pending_writes_preserved(&clear_state);
    assert_eq!(
        String::from_utf8(clear_output).unwrap(),
        "Scope filter cleared.\n"
    );

    let mut scope_state = shared_state_with_private_projection();
    let mut scope_output = Vec::new();
    super::super::commands::dispatch_command(
        &client,
        &mut scope_state,
        None,
        "filter global",
        &mut scope_output,
    )
    .await
    .unwrap();
    assert!(scope_state.client_instance_filter.is_none());
    assert_eq!(
        scope_state.scope_filter,
        Some(crate::args::RemoteConversationScope::Global)
    );
    assert_private_projection_cleared(&scope_state);
    assert_owner_state_and_pending_writes_preserved(&scope_state);
    assert_eq!(
        String::from_utf8(scope_output).unwrap(),
        "Scope filter set to global; this only organizes the displayed session list, not authorization or device identity.\n"
    );
}

#[tokio::test]
async fn remote_tui_invalid_or_unknown_instance_filter_preserves_state() {
    for command in ["instance invalid/id", "instance client-app-999"] {
        let client = test_client("127.0.0.1:1".parse().unwrap());
        let mut state = shared_state_with_private_projection();
        let mut output = Vec::new();

        super::super::commands::dispatch_command(&client, &mut state, None, command, &mut output)
            .await
            .unwrap();

        assert_eq!(
            state.client_instance_filter.as_deref(),
            Some("client-web-001")
        );
        assert!(state.history_loaded_for.is_some());
        assert!(!state.prompt_history.is_empty());
        assert_eq!(state.selected_run_id.as_deref(), Some("run-001"));
        assert!(state.pending_prompt.is_some());
        assert!(state.pending_run_intent.is_some());
        assert!(state.pending_create.is_some());
        assert!(state.client_instance_session_view_observed.is_some());
        assert!(state.client_instance_resource_view_observed.is_some());
        assert!(!String::from_utf8(output).unwrap().contains("filter set"));
    }
}

#[tokio::test]
async fn remote_tui_instance_filter_blocks_hidden_execution_consent_preview_before_http() {
    let client = test_client("127.0.0.1:1".parse().unwrap());
    let mut state = shared_state_with_private_projection();
    // The paired fixture declares only conversation-002 for the mobile
    // instance. Keep conversation-001 selected to verify the candidate never
    // reaches its first HTTP request through the filtered projection.
    state.client_instance_filter = Some("client-mobile-001".into());
    let mut output = Vec::new();

    let exited = super::super::commands::dispatch_command(
        &client,
        &mut state,
        None,
        "execution-consent-preview",
        &mut output,
    )
    .await
    .unwrap();

    assert!(!exited);
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains(
        "Conversation read blocked by client-instance display filter: conversation \"conversation-001\" is not declared for instance \"client-mobile-001\". No request was sent."
    ));
    assert!(!output.contains("Execution-consent preview request failed"));
}

#[test]
fn remote_tui_create_projection_keeps_unobserved_session_hidden() {
    let state = shared_state_with_private_projection();
    let hidden = json!({"id": "conversation-created-after-observation"});
    let visible = json!({"id": "conversation-001"});

    assert!(
        !super::super::state::conversation_visible_to_selected_client_instance(&state, &hidden)
            .unwrap()
    );
    assert!(
        super::super::state::conversation_visible_to_selected_client_instance(&state, &visible)
            .unwrap()
    );
}

#[tokio::test]
async fn remote_tui_create_stops_before_http_when_instance_projection_is_invalid() {
    let client = test_client("127.0.0.1:1".parse().unwrap());
    let mut state = shared_state_with_private_projection();
    // This case exercises the projection boundary itself. Pending writes are
    // intentionally cleared so the ordinary single-writer recovery guard
    // does not mask the invalid client-instance observation.
    state.pending_prompt = None;
    state.pending_run_intent = None;
    state.pending_create = None;
    state.client_instance_observation_status =
        Some(super::super::state::ClientInstanceObservationStatus {
            session_view_present: true,
            resource_view_present: true,
        });
    let mut output = Vec::new();

    super::super::writes::create_session(&client, &mut state, "new session", &mut output)
        .await
        .unwrap();

    assert!(state.pending_create.is_none());
    assert!(String::from_utf8(output)
        .unwrap()
        .contains("Create blocked by client-instance display filter"));
}

#[tokio::test]
async fn remote_tui_instance_create_requires_a_converged_resource_pair() {
    let client = test_client("127.0.0.1:1".parse().unwrap());
    let mut state = shared_state_with_private_projection();
    state.client_instance_resource_view_observed = None;
    state.pending_prompt = None;
    state.pending_run_intent = None;
    state.pending_create = None;
    let mut output = Vec::new();

    super::super::writes::create_session(&client, &mut state, "new session", &mut output)
        .await
        .unwrap();

    assert!(state.pending_create.is_none());
    assert!(String::from_utf8(output).unwrap().contains(
        "selected client-instance create requires converged session/resource observations"
    ));
}

#[tokio::test]
async fn remote_tui_create_does_not_select_session_hidden_by_instance_projection() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let session_view: Value = serde_json::from_str(include_str!(
            "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
        ))
        .unwrap();
        let resource_view: Value = serde_json::from_str(include_str!(
            "../../../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json"
        ))
        .unwrap();
        let (mut session, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
        assert!(body.is_empty());
        respond(&mut session, "200 OK", &session_view);

        let (mut resource, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        assert!(body.is_empty());
        respond(&mut resource, "200 OK", &resource_view);

        let (mut create, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("POST /api/v1/conversations "));
        let created = json!({
            "id": "conversation-created-after-observation",
            "scope": {"kind": "global"},
            "title": "Created",
            "created_at_ms": 2,
            "updated_at_ms": 2
        });
        respond(&mut create, "201 Created", &created);

        let (mut list, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations?"));
        respond(
            &mut list,
            "200 OK",
            &json!({
                "conversations": [{"conversation": created, "aggregate_version": 1}],
                "next_after_id": null,
                "has_more": false
            }),
        );
    });

    let client = test_client(address);
    let mut state = shared_state_with_private_projection();
    state.selected_id = None;
    state.selected_entry = None;
    state.pending_prompt = None;
    state.pending_run_intent = None;
    state.pending_create = None;
    let mut output = Vec::new();

    super::super::writes::create_session(&client, &mut state, "Created", &mut output)
        .await
        .unwrap();
    server.join().unwrap();

    assert!(state.selected_id.is_none());
    assert!(state.selected_entry.is_none());
    assert!(state.conversations.is_empty());
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("Created session \"conversation-created-after-observation\"."));
    assert!(output.contains("outside the selected client-instance display projection"));
}

#[tokio::test]
async fn remote_tui_create_blocks_owner_post_when_pair_drifted() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let session_view: Value = serde_json::from_str(include_str!(
            "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
        ))
        .unwrap();
        let mut resource_view: Value = serde_json::from_str(include_str!(
            "../../../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json"
        ))
        .unwrap();
        resource_view["instances"][0]["observed_at_ms"] = json!(200501);

        let (mut session, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
        assert!(body.is_empty());
        respond(&mut session, "200 OK", &session_view);

        let (mut resource, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        assert!(body.is_empty());
        respond(&mut resource, "200 OK", &resource_view);
    });

    let client = test_client(address);
    let mut state = shared_state_with_private_projection();
    state.pending_prompt = None;
    state.pending_run_intent = None;
    state.pending_create = None;
    let mut output = Vec::new();

    super::super::writes::create_session(&client, &mut state, "Must stay blocked", &mut output)
        .await
        .unwrap();
    server.join().unwrap();

    assert!(state.pending_create.is_some());
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("Create client-instance projection refresh failed"));
    assert!(output.contains("No request was sent"));
}

#[tokio::test]
async fn remote_tui_create_keeps_pending_write_after_projection_authorization_failure() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let session_view: Value = serde_json::from_str(include_str!(
            "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
        ))
        .unwrap();
        let (mut session, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
        assert!(body.is_empty());
        respond(&mut session, "200 OK", &session_view);

        let (mut resource, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        assert!(body.is_empty());
        respond(
            &mut resource,
            "403 Forbidden",
            &json!({"error": "forbidden"}),
        );
    });

    let client = test_client(address);
    let mut state = shared_state_with_private_projection();
    state.pending_prompt = None;
    state.pending_run_intent = None;
    state.pending_create = None;
    let mut output = Vec::new();

    super::super::writes::create_session(&client, &mut state, "Retry after auth", &mut output)
        .await
        .unwrap();
    server.join().unwrap();

    assert!(state.pending_create.is_some());
    assert_eq!(
        state.client_instance_filter.as_deref(),
        Some("client-web-001")
    );
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("Create client-instance projection refresh failed"));
    assert!(output.contains("Local session view cleared after authorization failure."));
    assert!(output.contains("No request was sent"));
}
