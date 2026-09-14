use std::{
    fs,
    io::{BufRead, BufReader, Cursor, Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};

use reqwest::{Client, Url, redirect::Policy};
use serde_json::{Value, json};

use super::super::credentials::{CredentialStore, StoredCredential};
use super::{
    RemoteClient, run_with_io,
    state::{TuiState, render, scope_filter_matches},
};

#[path = "remote_tui/runs_tests.rs"]
mod runs;

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
        selected_entry: Some(super::OwnedConversationEntry {
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

#[tokio::test]
async fn remote_tui_filtered_out_selected_session_remains_openable_without_permission_error() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(
            &listener,
            &json!({
                "conversations": [{
                    "conversation": conversation_projection("c-1", "Shared"),
                    "aggregate_version": 1
                }],
                "next_after_id": null,
                "has_more": false
            }),
        );
        let (mut history, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
        respond(
            &mut history,
            "200 OK",
            &json!({
                "conversation_id": "c-1",
                "prompts": [],
                "has_more": false
            }),
        );
    });

    let client = test_client(address);
    let mut reader = Cursor::new("filter group:grp_1\nopen c-1\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Opened session \"c-1\" and refreshed Prompt history."));
    assert!(!output.contains("That session is not in the loaded pages."));
    assert!(!output.to_lowercase().contains("permission"));
}

#[tokio::test]
async fn ambiguous_prompt_retry_reuses_the_same_key_version_and_body() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || serve_ambiguous_prompt_retry(&listener));

    let client = test_client(address);
    let mut reader = Cursor::new("prompt finish the shared task\nquit\nretry\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("outcome is not confirmed"));
    assert!(output.contains("A write may have been accepted"));
    assert!(output.contains("Prompt stored. No Run was started."));
    assert!(output.contains("Prompt history refreshed."));
    assert!(output.contains("finish the shared task"));
    assert!(output.contains("version 8"));
}

#[tokio::test]
async fn ambiguous_create_retry_reuses_the_same_key_and_title() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || serve_ambiguous_create_retry(&listener));

    let client = test_client(address);
    let mut reader = Cursor::new(
        "create --scope project:prj_1 Shared session\nretry\nprompt continue newly created session\nquit\n",
    );
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Session creation outcome is not confirmed"));
    assert!(output.contains("Created session \"c-2\"."));
    assert!(output.contains("project:\"prj_1\""));
    assert!(output.contains("outside loaded pages"));
    assert!(output.contains("version 2"));
    assert!(output.contains("continue newly created session"));
}

#[tokio::test]
async fn discarding_pending_prompt_prints_recovery_metadata_without_prompt_body() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut list_stream, _) = listener.accept().unwrap();
        let _ = read_request(&mut list_stream);
        respond(
            &mut list_stream,
            "200 OK",
            &json!({
                "conversations": [{
                    "conversation": conversation_projection("c-1", "Shared"),
                    "aggregate_version": 7
                }],
                "next_after_id": null,
                "has_more": false
            }),
        );
        drop(list_stream);

        let (mut prompt_stream, _) = listener.accept().unwrap();
        let (request, _, _) = read_request(&mut prompt_stream);
        assert!(request.starts_with("POST /api/v1/conversations/c-1/prompts "));
        respond(
            &mut prompt_stream,
            "503 Service Unavailable",
            &json!({"code": "temporarily_unavailable"}),
        );
    });

    let client = test_client(address);
    let mut reader = Cursor::new("prompt private prompt body\nquit --discard-pending\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("\"operation\":\"append_prompt\""));
    assert!(output.contains("\"conversation_id\":\"c-1\""));
    assert!(output.contains("\"expected_version\":7"));
    assert!(output.contains("\"idempotency_key\":\"forge-tui-"));
    assert!(!output.contains("private prompt body"));
}

#[tokio::test]
async fn oversized_command_preserves_pending_write_recovery() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut list_stream, _) = listener.accept().unwrap();
        let _ = read_request(&mut list_stream);
        respond(
            &mut list_stream,
            "200 OK",
            &json!({
                "conversations": [{
                    "conversation": conversation_projection("c-1", "Shared"),
                    "aggregate_version": 7
                }],
                "next_after_id": null,
                "has_more": false
            }),
        );
        drop(list_stream);

        let (mut prompt_stream, _) = listener.accept().unwrap();
        let (request, _, _) = read_request(&mut prompt_stream);
        assert!(request.starts_with("POST /api/v1/conversations/c-1/prompts "));
        respond(
            &mut prompt_stream,
            "503 Service Unavailable",
            &json!({"code": "temporarily_unavailable"}),
        );
    });

    let mut input = String::from("prompt private prompt body\n");
    input.push_str(&"x".repeat(256 * 1024 + 1));
    input.push('\n');
    let client = test_client(address);
    let mut reader = Cursor::new(input);
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Input exceeds the 256 KiB limit."));
    assert!(output.contains("Pending write recovery:"));
    assert!(output.contains("\"expected_version\":7"));
    assert!(!output.contains("private prompt body"));
}

#[tokio::test]
async fn failed_refresh_keeps_the_loaded_session_available() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || serve_failed_refresh_keeps_session(&listener));

    let client = test_client(address);
    let mut reader = Cursor::new("list\nopen c-1\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Refresh failed: Forge API returned HTTP 503"));
    assert!(output.contains("Prompt history for \"c-1\":"));
    assert!(output.contains("No prompts."));
}

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

#[tokio::test]
async fn successful_prompt_write_survives_history_refresh_failure() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut history, _, _, _) = accept_request(&listener);
        respond(
            &mut history,
            "200 OK",
            &json!({
                "conversation_id": "c-1",
                "prompts": [{"id": "p-1", "conversation_id": "c-1", "role": "user", "content": "previous confirmed prompt", "created_at_ms": 10}],
                "has_more": false
            }),
        );
        let (mut post, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("POST /api/v1/conversations/c-1/prompts "));
        let prompt: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(prompt["content"], "new confirmed prompt");
        respond(&mut post, "201 Created", &json!({"aggregate_version": 2}));
        let (mut refresh, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
        respond(
            &mut refresh,
            "503 Service Unavailable",
            &json!({"code": "temporarily_unavailable"}),
        );
    });

    let client = test_client(address);
    let mut reader = Cursor::new("open c-1\nprompt new confirmed prompt\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Prompt stored. No Run was started."));
    assert!(output.contains("Prompt was stored, but history refresh failed"));
    assert!(output.contains("previous confirmed prompt"));
    assert!(!output.contains("outcome is not confirmed"));
}

fn serve_failed_refresh_keeps_session(listener: &TcpListener) {
    let (mut initial, _) = listener.accept().unwrap();
    let _ = read_request(&mut initial);
    respond(
        &mut initial,
        "200 OK",
        &json!({
            "conversations": [{
                "conversation": conversation_projection("c-1", "Keep me"),
                "aggregate_version": 3
            }],
            "next_after_id": null,
            "has_more": false
        }),
    );
    drop(initial);

    serve_failed_refresh_response(listener);
    serve_selected_history(listener);
}

fn serve_failed_refresh_response(listener: &TcpListener) {
    let (mut failed_refresh, _) = listener.accept().unwrap();
    let (request, _, _) = read_request(&mut failed_refresh);
    assert!(request.starts_with("GET /api/v1/conversations?"));
    respond(
        &mut failed_refresh,
        "503 Service Unavailable",
        &json!({"code": "temporarily_unavailable"}),
    );
}

fn serve_selected_history(listener: &TcpListener) {
    let (mut history, _) = listener.accept().unwrap();
    let (request, _, _) = read_request(&mut history);
    assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
    respond(
        &mut history,
        "200 OK",
        &json!({"conversation_id": "c-1", "prompts": [], "has_more": false}),
    );
}

#[tokio::test]
async fn sync_reads_owner_changes_and_refreshes_the_selected_history() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (_config_root, cursor_store) = tui_checkpoint_store(address);
    let server = thread::spawn(move || serve_owner_sync(&listener));

    let mut client = test_client(address);
    client.change_cursor = Some(cursor_store.clone());
    let mut reader = Cursor::new("sync\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Synced 1 owner-visible changes through cursor 1"));
    assert!(output.contains("version 2"));
    assert!(output.contains("Prompt history refreshed for the selected session."));
    assert!(output.contains("from another client"));
    assert_eq!(cursor_store.load().unwrap(), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn sync_keeps_the_cursor_when_selected_history_refresh_fails() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (_config_root, cursor_store) = tui_checkpoint_store(address);
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut initial_history, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
        respond(
            &mut initial_history,
            "200 OK",
            &json!({
                "conversation_id": "c-1",
                "prompts": [{"id": "p-1", "conversation_id": "c-1", "role": "user", "content": "keep this confirmed history", "created_at_ms": 10}],
                "has_more": false
            }),
        );
        let (mut changes, _, _, _) = accept_request(&listener);
        respond(&mut changes, "200 OK", &owner_change_page());
        serve_conversation_page(&listener, &conversation_page(2));
        let (mut history, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
        respond(
            &mut history,
            "503 Service Unavailable",
            &json!({
                "api_version": "forgeos.app-server/v1",
                "code": "temporarily_unavailable",
                "message": "retry"
            }),
        );
    });
    let mut client = test_client(address);
    client.change_cursor = Some(cursor_store.clone());
    let mut reader = Cursor::new("open c-1\nsync\nquit\n");
    let mut writer = Vec::new();

    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    assert_eq!(cursor_store.load().unwrap(), 0);
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Sync Prompt history refresh failed: Forge API returned HTTP 503"));
    assert!(output.contains("keep this confirmed history"));
    server.join().unwrap();
}

#[cfg(unix)]
fn tui_checkpoint_store(
    coordinator: std::net::SocketAddr,
) -> (
    tempfile::TempDir,
    super::super::credentials::ChangeCursorStore,
) {
    use std::os::unix::fs::PermissionsExt;

    let config_root = tempfile::tempdir_in(std::env::var("HOME").unwrap()).unwrap();
    fs::set_permissions(config_root.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let store = CredentialStore::for_test(config_root.path().to_path_buf());
    let credential = StoredCredential {
        issuer: "https://id.example".into(),
        client_id: "forge-cli".into(),
        subject: "user-a".into(),
        tenant_id: "tenant-a".into(),
        access_token: "saved-test-token".into(),
        expires_at_unix: 4_000_000_000,
    };
    store.save(&credential).unwrap();
    let cursor = store.change_cursor_store(&format!("http://{coordinator}/"), &credential);
    (config_root, cursor)
}

fn serve_owner_sync(listener: &TcpListener) {
    serve_conversation_page(listener, &conversation_page(1));
    let (mut changes, request, _, _) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/conversation-changes?after_cursor=0&limit=128 "));
    respond(&mut changes, "200 OK", &owner_change_page());
    drop(changes);
    serve_conversation_page(listener, &conversation_page(2));
    let (mut history, request, _, _) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
    respond(
        &mut history,
        "200 OK",
        &json!({
            "conversation_id": "c-1",
            "prompts": [{"id": "p-2", "conversation_id": "c-1", "role": "user", "content": "from another client", "created_at_ms": 20}],
            "has_more": false
        }),
    );
}

fn conversation_page(version: u64) -> Value {
    json!({
        "conversations": [{
            "conversation": conversation_projection("c-1", "Shared"),
            "aggregate_version": version
        }],
        "next_after_id": null,
        "has_more": false
    })
}

fn conversation_projection(id: &str, title: &str) -> Value {
    json!({
        "id": id,
        "scope": {"kind": "global"},
        "title": title,
        "created_at_ms": 1,
        "updated_at_ms": 1
    })
}

fn owner_change_page() -> Value {
    json!({
        "after_cursor": 0,
        "scanned_through_cursor": 1,
        "has_more": false,
        "changes": [{
            "cursor": 1,
            "schema_version": 1,
            "conversation_id": "c-1",
            "entity_id": "p-2",
            "aggregate_version": 2,
            "kind": "prompt_appended",
            "created_at_ms": 20
        }]
    })
}

fn serve_ambiguous_prompt_retry(listener: &TcpListener) {
    serve_conversation_page(
        listener,
        &json!({
            "conversations": [{
                "conversation": conversation_projection("c-1", "Shared"),
                "aggregate_version": 7
            }],
            "next_after_id": null,
            "has_more": false
        }),
    );
    let (mut first_stream, first_request, first_headers, first_body) =
        receive_prompt_request(listener);
    respond(
        &mut first_stream,
        "503 Service Unavailable",
        &json!({"api_version": "forgeos.app-server/v1", "code": "temporarily_unavailable", "message": "retry"}),
    );
    drop(first_stream);
    let (mut retry_stream, retry_request, retry_headers, retry_body) = accept_request(listener);
    assert_eq!(retry_request, first_request);
    assert_eq!(retry_headers, first_headers);
    assert_eq!(retry_body, first_body);
    respond(
        &mut retry_stream,
        "201 Created",
        &json!({"aggregate_version": 8, "replayed": true}),
    );
    let (mut history, request, _, _) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
    respond(
        &mut history,
        "200 OK",
        &json!({
            "conversation_id": "c-1",
            "prompts": [{"id": "p-1", "conversation_id": "c-1", "role": "user", "content": "finish the shared task", "created_at_ms": 10}],
            "has_more": false
        }),
    );
}

fn receive_prompt_request(
    listener: &TcpListener,
) -> (std::net::TcpStream, String, String, Vec<u8>) {
    let (stream, request, headers, body) = accept_request(listener);
    assert!(request.starts_with("POST /api/v1/conversations/c-1/prompts "));
    assert!(headers.contains("authorization: bearer test-token"));
    assert!(headers.contains("idempotency-key: forge-tui-"));
    let prompt: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(prompt["content"], "finish the shared task");
    assert_eq!(prompt["expected_version"], 7);
    (stream, request, headers, body)
}

fn serve_ambiguous_create_retry(listener: &TcpListener) {
    serve_conversation_page(
        listener,
        &json!({"conversations": [], "next_after_id": null, "has_more": false}),
    );
    serve_idempotent_create(listener);
    serve_conversation_page(
        listener,
        &json!({"conversations": [], "next_after_id": null, "has_more": false}),
    );
    serve_prompt_for_created_session(listener);
}

fn serve_idempotent_create(listener: &TcpListener) {
    let (mut first_stream, first_request, first_headers, first_body) = accept_request(listener);
    assert!(first_request.starts_with("POST /api/v1/conversations "));
    assert!(first_headers.contains("idempotency-key: forge-tui-"));
    let first_json: Value = serde_json::from_slice(&first_body).unwrap();
    assert_eq!(first_json["title"], "Shared session");
    assert_eq!(first_json["scope"]["kind"], "project");
    assert_eq!(first_json["scope"]["id"], "prj_1");
    respond(
        &mut first_stream,
        "503 Service Unavailable",
        &json!({"api_version": "forgeos.app-server/v1", "code": "temporarily_unavailable", "message": "retry"}),
    );
    drop(first_stream);
    let (mut retry_stream, retry_request, retry_headers, retry_body) = accept_request(listener);
    assert_eq!(retry_request, first_request);
    assert_eq!(retry_headers, first_headers);
    assert_eq!(retry_body, first_body);
    respond(
        &mut retry_stream,
        "201 Created",
        &json!({"id": "c-2", "scope": {"kind": "project", "id": "prj_1"}, "title": "Shared session"}),
    );
}

fn serve_prompt_for_created_session(listener: &TcpListener) {
    let (mut stream, request, _, body) = accept_request(listener);
    assert!(request.starts_with("POST /api/v1/conversations/c-2/prompts "));
    let prompt: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(prompt["content"], "continue newly created session");
    assert_eq!(prompt["expected_version"], 1);
    respond(
        &mut stream,
        "201 Created",
        &json!({"aggregate_version": 2, "replayed": false}),
    );
    let (mut history, request, _, _) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/conversations/c-2/prompts?"));
    respond(
        &mut history,
        "200 OK",
        &json!({
            "conversation_id": "c-2",
            "prompts": [{"id": "p-1", "conversation_id": "c-2", "role": "user", "content": "continue newly created session", "created_at_ms": 10}],
            "has_more": false
        }),
    );
}

fn test_client(address: std::net::SocketAddr) -> RemoteClient {
    RemoteClient {
        http: Client::builder()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap(),
        base_url: Url::parse(&format!("http://{address}")).unwrap(),
        access_token: "test-token".into(),
        change_cursor: None,
        token_refresh: None,
    }
}

fn serve_conversation_page(listener: &TcpListener, payload: &Value) {
    let (mut stream, request, _, _) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/conversations?"));
    respond(&mut stream, "200 OK", payload);
}

fn accept_request(listener: &TcpListener) -> (std::net::TcpStream, String, String, Vec<u8>) {
    let (mut stream, _) = listener.accept().unwrap();
    let (request, headers, body) = read_request(&mut stream);
    (stream, request, headers, body)
}

fn read_request(stream: &mut std::net::TcpStream) -> (String, String, Vec<u8>) {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    reader.read_line(&mut request_line).unwrap();
    let mut headers = String::new();
    let mut content_length = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" || line.is_empty() {
            break;
        }
        let normalized = line.to_ascii_lowercase();
        if let Some(value) = normalized.strip_prefix("content-length:") {
            content_length = value.trim().parse::<usize>().unwrap();
        }
        headers.push_str(&normalized);
    }
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body).unwrap();
    (request_line.trim().into(), headers, body)
}

fn respond(stream: &mut std::net::TcpStream, status: &str, payload: &Value) {
    let bytes = serde_json::to_vec(payload).unwrap();
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        bytes.len()
    )
    .unwrap();
    stream.write_all(&bytes).unwrap();
}
