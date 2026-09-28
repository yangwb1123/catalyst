use serde_json::json;

use super::super::super::{
    OwnedConversationEntry,
    state::{PendingCreate, PendingPrompt, PendingRunIntent, PendingRunIntentPageObservation},
};
use super::TuiState;

pub(super) fn shared_state_with_private_projection() -> TuiState {
    let state = run_projection_state(owner_state());
    TuiState {
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
        ..state
    }
}

fn run_projection_state(state: TuiState) -> TuiState {
    TuiState {
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
        ..state
    }
}

fn owner_state() -> TuiState {
    let session_view = serde_json::from_str(include_str!(
        "../../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap();
    let resource_view = serde_json::from_str(include_str!(
        "../../../../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json"
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
        client_instance_session_view_observed: Some(session_view),
        client_instance_resource_view_observed: Some(resource_view),
        ..TuiState::default()
    }
}
