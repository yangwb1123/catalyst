use std::{io::Cursor, net::TcpListener, thread};

use serde_json::{Value, json};

use super::{
    RemoteClient, run_with_io,
    state::{TuiState, render, scope_filter_matches},
};

#[path = "remote_tui/runs_tests.rs"]
mod runs;

#[path = "remote_tui_tests/attempt_request.rs"]
mod attempt_request;
#[path = "remote_tui_tests/authorization.rs"]
mod authorization;
#[path = "remote_tui_tests/changes.rs"]
mod changes;
#[path = "remote_tui_tests/client_instance_views.rs"]
mod client_instance_views;
#[path = "remote_tui_tests/credential_candidate.rs"]
mod credential_candidate;
#[path = "remote_tui_tests/execution_consent.rs"]
mod execution_consent;
#[path = "remote_tui_tests/execution_reconciliation.rs"]
mod execution_reconciliation;
#[path = "remote_tui_tests/inventory.rs"]
mod inventory;
#[path = "remote_tui_tests/lifecycle_registry.rs"]
mod lifecycle_registry;
#[path = "remote_tui_tests/local_runner_preview.rs"]
mod local_runner_preview;
#[path = "remote_tui_tests/pending_run_intent.rs"]
mod pending_run_intent;
#[path = "remote_tui_tests/pending_run_intents.rs"]
mod pending_run_intents;
#[path = "remote_tui_tests/placement.rs"]
mod placement;
#[path = "remote_tui_tests/resilience.rs"]
mod resilience;
#[path = "remote_tui_tests/run_attempt_lease_dispatch_preflight.rs"]
mod run_attempt_lease_dispatch_preflight;
#[path = "remote_tui_tests/runner_dispatch_plan_preview.rs"]
mod runner_dispatch_plan_preview;
#[path = "remote_tui_tests/scope_tests.rs"]
mod scope_tests;
#[path = "remote_tui_tests/session_selection.rs"]
mod session_selection;
#[path = "remote_tui_tests/state.rs"]
mod state;

#[tokio::test]
async fn remote_tui_scope_filter_preserves_server_cursor_across_empty_page_and_can_be_cleared() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut first, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations?limit=128 "));
        respond(&mut first, "200 OK", &full_global_conversation_page());
        drop(first);

        let (mut second, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations?limit=128&after_id=c-127 "));
        respond(
            &mut second,
            "200 OK",
            &json!({
                "conversations": [{
                    "conversation": {
                        "id": "c-128",
                        "scope": {"kind": "project", "id": "prj_1"},
                        "title": "Project hit",
                        "created_at_ms": 2,
                        "updated_at_ms": 2
                    },
                    "aggregate_version": 2
                }],
                "next_after_id": null,
                "has_more": false
            }),
        );
    });

    let client = test_client(address);
    let mut reader = Cursor::new("filter project:prj_1\nnext\nfilter clear\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("No sessions match this scope filter in the loaded pages."));
    assert!(
        output.contains("\"c-128\"  \"Project hit\"  [project:\"prj_1\"]"),
        "{output}"
    );
    assert!(output.contains("Scope filter cleared."));
    assert!(output.contains("\"c-000\"  \"Global 0\"  [global]"));
    assert!(output.contains("organization-only display filter"));
}

#[tokio::test]
async fn remote_tui_import_rejects_noncanonical_confirmation_before_loading_local_hub() {
    let client = test_client("127.0.0.1:1".parse().unwrap());
    let mut state = TuiState::default();
    let mut writer = Vec::new();

    let exited = super::commands::dispatch_command(
        &client,
        &mut state,
        None,
        "import local-conversation --confirm ABC",
        &mut writer,
    )
    .await
    .unwrap();

    assert!(!exited);
    assert_eq!(
        String::from_utf8(writer).unwrap(),
        "Import confirmation must be a lowercase SHA-256 digest.\n"
    );
}

#[tokio::test]
async fn remote_tui_can_preview_heartbeat_persistence_without_a_device_request() {
    let client = test_client("127.0.0.1:1".parse().unwrap());
    let mut state = TuiState::default();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../../docs/contracts/fixtures/forge-device-heartbeat-persistence-contract-v1.json",
    );
    let mut writer = Vec::new();

    let exited = super::commands::dispatch_command(
        &client,
        &mut state,
        None,
        &format!(
            "heartbeat-persistence-preview --input {}",
            fixture.display()
        ),
        &mut writer,
    )
    .await
    .unwrap();

    assert!(!exited);
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "offline heartbeat persistence preview [forge.device-heartbeat-persistence-contract/v1] device=device-a tenant=tenant-1 approval=approved"
        ),
        "{output}"
    );
    assert!(output.contains("initial_insert: accepted=true revision=1"));
    assert!(output.contains("stale_heartbeat_rejected_after_matching_revision: accepted=false error=sequence_not_increasing"));
    assert!(output.contains("authority: identity_verified=false"));
}

#[tokio::test]
async fn remote_tui_can_preview_identity_proof_without_a_device_request() {
    let client = test_client("127.0.0.1:1".parse().unwrap());
    let mut state = TuiState::default();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-device-identity-proof-contract-v1.json");
    let mut writer = Vec::new();

    let exited = super::commands::dispatch_command(
        &client,
        &mut state,
        None,
        &format!("identity-proof-preview --input {}", fixture.display()),
        &mut writer,
    )
    .await
    .unwrap();

    assert!(!exited);
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "offline device identity proof preview [forge.device-identity-proof-contract/v1] device=device-a"
        ),
        "{output}"
    );
    assert!(output.contains("approved_exact_binding: accepted=true reason=bound_approved"));
    assert!(output.contains("wrong_owner_subject: accepted=false reason=owner_mismatch"));
    assert!(output.contains("authority: identity_verified=false"));
}

fn full_global_conversation_page() -> Value {
    let conversations = (0..128)
        .map(|index| {
            let id = format!("c-{index:03}");
            json!({
                "conversation": conversation_projection(&id, &format!("Global {index}")),
                "aggregate_version": 1
            })
        })
        .collect::<Vec<_>>();
    json!({
        "conversations": conversations,
        "next_after_id": "c-127",
        "has_more": true
    })
}

#[path = "remote_tui_tests/helpers.rs"]
mod helpers;
use helpers::{
    accept_request, conversation_projection, respond, serve_conversation_page, test_client,
};
