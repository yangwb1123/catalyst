use serde_json::json;

use super::super::{
    OwnedConversationEntry,
    commands::dispatch_command,
    state::{PendingRunIntent, TuiState},
};
use super::helpers::{accept_request, conversation_projection, respond, test_client};
use serde_json::Value;
use std::time::{Duration, Instant};
use std::{net::TcpListener, thread};

fn inventory_resource_pair() -> (serde_json::Value, serde_json::Value) {
    let pair: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json"
    ))
    .unwrap();
    (pair["inventory"].clone(), pair["resource_view"].clone())
}

fn session_view_for_resource(resource: &serde_json::Value) -> serde_json::Value {
    let mut session_view: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap();
    session_view["instances"] = resource["instances"].clone();
    session_view
}

fn pending_run_intent_state(
    inventory: serde_json::Value,
    resource: serde_json::Value,
    session_view: serde_json::Value,
) -> TuiState {
    let entry = OwnedConversationEntry {
        conversation: conversation_projection("conversation-001", "Shared"),
        aggregate_version: 2,
    };
    TuiState {
        conversations: vec![entry.clone()],
        selected_id: Some("conversation-001".into()),
        selected_entry: Some(entry),
        client_instance_filter: Some("client-web-001".into()),
        device_inventory_v2_observed: Some(inventory),
        client_instance_session_view_observed: Some(session_view),
        client_instance_resource_view_observed: Some(resource),
        pending_run_intent: Some(PendingRunIntent {
            conversation_id: "conversation-001".into(),
            expected_version: 2,
            content: "compute this".into(),
            idempotency_key: "pending-run-intent-key".into(),
        }),
        ..TuiState::default()
    }
}

#[path = "pending_run_intents/instance_retry.rs"]
mod instance_retry;
#[path = "pending_run_intents/metadata.rs"]
mod metadata;
#[path = "pending_run_intents/submission.rs"]
mod submission;
#[path = "pending_run_intents/timeline.rs"]
mod timeline;
