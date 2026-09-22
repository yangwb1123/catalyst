use super::*;
use crate::runtime_domain::ConversationScope;
use serde_json::json;
use tempfile::tempdir;

#[path = "runtime_rpc/conversation_transport.rs"]
mod conversation_transport;
#[path = "runtime_rpc/owned.rs"]
mod owned;

#[test]
fn snapshot_and_change_queries_are_bounded_and_omit_project_paths() {
    let directory = tempdir().expect("temp directory");
    let (database, project_id, conversation_id, before_cursor) =
        create_prompted_fixture(directory.path());

    let snapshot = process_request(&database, &request("snapshot_at_cursor", &json!({})));
    let snapshot: serde_json::Value =
        serde_json::from_slice(without_lf(&snapshot)).expect("snapshot response JSON");
    assert_sanitized_snapshot(&snapshot, before_cursor + 1, &project_id, directory.path());

    let page = process_request(
        &database,
        &request(
            "conversation_changes_after",
            &json!({"after_cursor": before_cursor, "limit": 1}),
        ),
    );
    let page: serde_json::Value =
        serde_json::from_slice(without_lf(&page)).expect("page response JSON");
    assert_prompt_change(&page, &conversation_id);
}

#[test]
fn owner_claims_are_validated_before_opening_the_database() {
    let directory = tempdir().expect("temp directory");
    let database = directory.path().join("missing.sqlite3");
    let response = process_owned_request(
        &database,
        "list_owned_conversations",
        &json!({"owner":{"issuer":" ","subject":"account-42","tenant_id":"tenant-slate"},"limit":8}),
    );
    assert_eq!(
        response["error"]["code"],
        "invalid_owned_conversation_request"
    );
    assert!(!database.exists());
}

#[test]
fn prompt_pages_are_scoped_keyset_ordered_and_hide_idempotency_keys() {
    let directory = tempdir().expect("temp directory");
    let (database, _, conversation_id, _) = create_prompted_fixture(directory.path());
    let store = Arc::new(SqliteHubStore::open(&database).expect("reopen writable Hub"));
    let service = HubService::new(store);
    service
        .append_prompt(&conversation_id, "assistant", "second", "secret-key-two")
        .expect("append second Prompt");
    let other = service
        .create_session(&ConversationScope::Global, "other", "other-create")
        .expect("create other Conversation");
    service
        .append_prompt(&other.id, "user", "other body", "secret-other-key")
        .expect("append other Conversation Prompt");
    let cursor_before = service
        .snapshot_at_cursor()
        .expect("cursor before reads")
        .cursor;
    let first = assert_first_prompt_page(&database, &conversation_id);
    assert_prompt_page_continuation(&database, &conversation_id, &first);
    assert_eq!(
        service
            .snapshot_at_cursor()
            .expect("cursor after reads")
            .cursor,
        cursor_before
    );
}

#[test]
fn bootstrap_pages_use_a_frozen_head_and_advance_over_prompt_only_events() {
    let directory = tempdir().expect("temp directory");
    let (database, _, conversation_id, _) = create_prompted_fixture(directory.path());

    let first = process_request(&database, &bootstrap_page_request(None, 1));
    let first: serde_json::Value =
        serde_json::from_slice(without_lf(&first)).expect("first bootstrap response");
    assert_eq!(first["ok"], true);
    assert_eq!(first["result"]["snapshot_cursor"], 2);
    assert_eq!(first["result"]["scanned_through_cursor"], 0);
    assert!(
        first["result"]["conversations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(first["result"]["has_more"], true);

    let second = process_request(
        &database,
        &bootstrap_page_request(Some(first["result"]["next_cursor"].clone()), 1),
    );
    let second: serde_json::Value =
        serde_json::from_slice(without_lf(&second)).expect("second bootstrap response");
    assert_eq!(second["ok"], true);
    assert_eq!(second["result"]["conversations"][0]["creation_cursor"], 1);
    assert_eq!(second["result"]["conversations"][0]["aggregate_version"], 2);
    assert_eq!(
        second["result"]["conversations"][0]["conversation"]["id"],
        conversation_id
    );
    assert_eq!(second["result"]["has_more"], true);

    let third = process_request(
        &database,
        &bootstrap_page_request(Some(second["result"]["next_cursor"].clone()), 1),
    );
    let third: serde_json::Value =
        serde_json::from_slice(without_lf(&third)).expect("third bootstrap response");
    assert_eq!(third["ok"], true);
    assert_eq!(third["result"]["scanned_through_cursor"], 2);
    assert!(
        third["result"]["conversations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(third["result"]["has_more"], false);
    assert!(third["result"].get("next_cursor").is_none());
}

#[test]
fn invalid_bootstrap_cursors_fail_before_database_open() {
    let directory = tempdir().expect("temp directory");
    let database = directory.path().join("missing.sqlite3");
    for (cursor, limit, expected_code) in [
        (None, 0, "invalid_limit"),
        (
            Some(json!({
                "snapshot_cursor": 1,
                "phase": "change_log",
                "after_conversation_id": "unexpected",
                "after_change_cursor": 0
            })),
            1,
            "invalid_cursor",
        ),
        (
            Some(json!({
                "snapshot_cursor": 1,
                "phase": "legacy_baseline",
                "after_conversation_id": "c1",
                "after_change_cursor": 0
            })),
            1,
            "invalid_cursor",
        ),
        (
            Some(json!({
                "snapshot_cursor": 1,
                "phase": "change_log",
                "after_change_cursor": 2
            })),
            1,
            "invalid_cursor",
        ),
    ] {
        let fields = json!({"limit": limit, "cursor": cursor});
        let response = process_request(&database, &request("conversation_bootstrap_page", &fields));
        let response: serde_json::Value =
            serde_json::from_slice(without_lf(&response)).expect("invalid bootstrap response");
        assert_eq!(response["error"]["code"], expected_code);
        assert!(!database.exists());
    }
}

fn assert_first_prompt_page(database: &Path, conversation_id: &str) -> serde_json::Value {
    let bytes = process_request(database, &prompt_page_request(conversation_id, None, 1));
    let response: serde_json::Value =
        serde_json::from_slice(without_lf(&bytes)).expect("first Prompt page JSON");
    let result = &response["result"];
    assert_eq!(response["ok"], true);
    assert_eq!(result["conversation_id"], conversation_id);
    assert_eq!(result["prompts"].as_array().expect("Prompts").len(), 1);
    assert_eq!(result["has_more"], true);
    let prompt = result["prompts"][0].as_object().expect("Prompt projection");
    assert!(!prompt.contains_key("idempotency_key"));
    assert_eq!(prompt["conversation_id"], conversation_id);
    assert!(!String::from_utf8_lossy(&bytes).contains("secret-key"));
    assert!(!String::from_utf8_lossy(&bytes).contains("secret-other-key"));
    response
}

fn assert_prompt_page_continuation(
    database: &Path,
    conversation_id: &str,
    first: &serde_json::Value,
) {
    let cursor = first["result"]["next_cursor"].clone();
    let bytes = process_request(
        database,
        &prompt_page_request(conversation_id, Some(cursor), 1),
    );
    let second: serde_json::Value =
        serde_json::from_slice(without_lf(&bytes)).expect("second Prompt page JSON");
    assert_eq!(second["ok"], true);
    assert_eq!(second["result"]["has_more"], false);
    assert_eq!(
        second["result"]["prompts"]
            .as_array()
            .expect("Prompts")
            .len(),
        1
    );
    assert_ne!(
        first["result"]["prompts"][0]["id"],
        second["result"]["prompts"][0]["id"]
    );
}

#[test]
fn maximum_control_character_prompt_fits_the_bounded_response() {
    let directory = tempdir().expect("temp directory");
    let database = directory.path().join("hub.sqlite3");
    let store = Arc::new(SqliteHubStore::open(&database).expect("create Hub"));
    let service = HubService::new(store);
    let conversation = service
        .create_session(&ConversationScope::Global, "large", "create-large")
        .expect("create Conversation");
    service
        .append_prompt(
            &conversation.id,
            "user",
            &"\0".repeat(crate::runtime_application::MAX_PROMPT_BYTES),
            "secret-large-key",
        )
        .expect("append maximum Prompt");

    let response = process_request(&database, &prompt_page_request(&conversation.id, None, 1));
    assert!(response.len() < MAX_RESPONSE_BYTES);
    let response: serde_json::Value =
        serde_json::from_slice(without_lf(&response)).expect("bounded Prompt response");
    assert_eq!(response["ok"], true);
    assert_eq!(
        response["result"]["prompts"][0]["content"]
            .as_str()
            .expect("Prompt body")
            .len(),
        crate::runtime_application::MAX_PROMPT_BYTES
    );
    assert!(
        response["result"]["prompts"][0]
            .as_object()
            .expect("Prompt projection")
            .get("idempotency_key")
            .is_none()
    );
}

#[test]
fn invalid_prompt_pages_fail_before_database_open_and_missing_conversations_are_generic() {
    let directory = tempdir().expect("temp directory");
    let database = directory.path().join("missing.sqlite3");
    for (fields, expected_code) in [
        (json!({"conversation_id":"c1","limit":0}), "invalid_limit"),
        (
            json!({"conversation_id":"c1","limit":MAX_CONVERSATION_PROMPT_PAGE_LIMIT + 1}),
            "invalid_limit",
        ),
        (
            json!({"conversation_id":" ","limit":1}),
            "invalid_prompt_request",
        ),
        (
            json!({"conversation_id":"c1","limit":1,"before":{"created_at_ms":1,"prompt_id":"p1","extra":true}}),
            "invalid_request",
        ),
        (
            json!({"conversation_id":"c1","limit":1,"before":{"created_at_ms":9007199254740992u64,"prompt_id":"p1"}}),
            "invalid_prompt_request",
        ),
    ] {
        let response = process_request(&database, &request("conversation_prompt_page", &fields));
        let response: serde_json::Value =
            serde_json::from_slice(without_lf(&response)).expect("invalid request JSON");
        assert_eq!(response["error"]["code"], expected_code);
        assert!(!database.exists());
    }

    let unknown_operation = request("prompt_history", &json!({}));
    let unknown = process_request(&database, &unknown_operation);
    let unknown: serde_json::Value =
        serde_json::from_slice(without_lf(&unknown)).expect("unknown operation response");
    assert_eq!(unknown["error"]["code"], "invalid_request");
    assert!(!database.exists());

    let (existing_database, _, _, _) = create_prompted_fixture(directory.path());
    let missing = process_request(
        &existing_database,
        &prompt_page_request("missing-conversation", None, 1),
    );
    let missing: serde_json::Value =
        serde_json::from_slice(without_lf(&missing)).expect("missing Conversation response");
    assert_eq!(missing["ok"], false);
    assert_eq!(missing["error"]["code"], "query_failed");
}

fn create_prompted_fixture(directory: &Path) -> (PathBuf, String, String, u64) {
    let database = directory.join("hub.sqlite3");
    let store = Arc::new(SqliteHubStore::open(&database).expect("create Hub"));
    let service = HubService::new(store);
    let project = service.open_project(directory).expect("open project");
    let conversation = service
        .create_session(&ConversationScope::Global, "shared", "create-key")
        .expect("create conversation");
    let before_cursor = service.snapshot_at_cursor().expect("snapshot").cursor;
    service
        .append_prompt(&conversation.id, "user", "hello", "prompt-key")
        .expect("append prompt");
    (database, project.id, conversation.id, before_cursor)
}

fn assert_sanitized_snapshot(
    snapshot: &serde_json::Value,
    expected_cursor: u64,
    project_id: &str,
    directory: &Path,
) {
    assert_eq!(snapshot["ok"], true);
    assert_eq!(snapshot["result"]["cursor"], expected_cursor);
    assert_eq!(
        snapshot["result"]["snapshot"]["projects"][0]["id"],
        project_id
    );
    assert!(
        snapshot["result"]["snapshot"]["projects"][0]
            .get("path")
            .is_none()
    );
    assert!(
        !String::from_utf8_lossy(&snapshot.to_string().into_bytes())
            .contains(&directory.to_string_lossy().to_string())
    );
}

fn assert_prompt_change(page: &serde_json::Value, conversation_id: &str) {
    assert_eq!(page["ok"], true);
    assert_eq!(page["result"]["changes"][0]["kind"], "prompt_appended");
    assert_eq!(
        page["result"]["changes"][0]["conversation_id"],
        conversation_id
    );
}

#[test]
fn malformed_frames_and_unknown_fields_fail_closed() {
    assert_eq!(read_framed_request(&b"{}\n\n"[..]), Err("invalid_framing"));
    assert_eq!(read_framed_request(&b"{}"[..]), Err("invalid_framing"));
    let oversized = vec![b'a'; MAX_REQUEST_BYTES + 1];
    assert_eq!(
        read_framed_request(oversized.as_slice()),
        Err("request_too_large")
    );
    let unknown = br#"{"api_version":"forgeos.runtime-bridge/v1","request_id":"q1","operation":"snapshot_at_cursor","extra":true}"#;
    assert_eq!(
        serde_json::from_slice::<RpcRequest>(unknown)
            .unwrap_err()
            .classify(),
        serde_json::error::Category::Data
    );
}

#[test]
fn invalid_limits_and_versions_are_rejected_before_database_open() {
    let directory = tempdir().expect("temp directory");
    let database = directory.path().join("missing.sqlite3");
    let invalid_limit = request(
        "conversation_changes_after",
        &json!({"after_cursor": 0, "limit": MAX_CHANGE_LIMIT + 1}),
    );
    let response = process_request(&database, &invalid_limit);
    let response: serde_json::Value =
        serde_json::from_slice(without_lf(&response)).expect("invalid limit response");
    assert_eq!(response["error"]["code"], "invalid_limit");
    assert!(!database.exists());

    let unsupported = json!({
        "api_version": "forgeos.runtime-bridge/v2",
        "request_id": "test-1",
        "operation": "snapshot_at_cursor",
    });
    let response = process_request(
        &database,
        &serde_json::to_vec(&unsupported).expect("encode unsupported request"),
    );
    let response: serde_json::Value =
        serde_json::from_slice(without_lf(&response)).expect("unsupported version response");
    assert_eq!(response["error"]["code"], "unsupported_version");
    assert!(!database.exists());
}

#[test]
fn response_limit_falls_back_to_one_bounded_error_line() {
    let response = bound_response(vec![b'x'; MAX_RESPONSE_BYTES], "test-1");
    assert!(response.len() < MAX_RESPONSE_BYTES);
    assert_eq!(response.last(), Some(&b'\n'));
    let response: serde_json::Value =
        serde_json::from_slice(without_lf(&response)).expect("bounded response JSON");
    assert_eq!(response["error"]["code"], "response_too_large");

    let encoded = success_response("test-1", json!({"payload": "x".repeat(MAX_RESPONSE_BYTES)}));
    let encoded: serde_json::Value =
        serde_json::from_slice(&encoded).expect("bounded serialization fallback JSON");
    assert_eq!(encoded["error"]["code"], "response_too_large");
}

#[test]
fn missing_database_query_does_not_create_or_migrate_state() {
    let directory = tempdir().expect("temp directory");
    let database = directory.path().join("missing.sqlite3");
    let response = process_request(&database, &request("snapshot_at_cursor", &json!({})));
    let response: serde_json::Value =
        serde_json::from_slice(without_lf(&response)).expect("error response JSON");
    assert_eq!(response["ok"], false);
    assert_eq!(response["error"]["code"], "query_failed");
    assert!(!database.exists());
}

fn request(operation: &str, fields: &serde_json::Value) -> Vec<u8> {
    let mut value = json!({
        "api_version": API_VERSION,
        "request_id": "test-1",
        "operation": operation,
    });
    value.as_object_mut().expect("request object").extend(
        fields
            .as_object()
            .expect("request fields")
            .iter()
            .map(|(key, value)| (key.clone(), value.clone())),
    );
    serde_json::to_vec(&value).expect("encode request")
}

fn process_owned_request(
    database: &Path,
    operation: &str,
    fields: &serde_json::Value,
) -> serde_json::Value {
    let mut value = json!({
        "api_version": WRITE_API_VERSION,
        "request_id": "owned-test-1",
        "operation": operation,
    });
    value.as_object_mut().expect("request object").extend(
        fields
            .as_object()
            .expect("request fields")
            .iter()
            .map(|(key, value)| (key.clone(), value.clone())),
    );
    let request = serde_json::to_vec(&value).expect("encode owner-scoped request");
    let response = process_request(database, &request);
    let response: serde_json::Value =
        serde_json::from_slice(without_lf(&response)).expect("owner-scoped response JSON");
    assert_eq!(response["api_version"], WRITE_API_VERSION);
    response
}

fn prompt_page_request(
    conversation_id: &str,
    before: Option<serde_json::Value>,
    limit: usize,
) -> Vec<u8> {
    let mut fields = json!({"conversation_id": conversation_id, "limit": limit});
    if let Some(before) = before {
        fields["before"] = before;
    }
    request("conversation_prompt_page", &fields)
}

fn bootstrap_page_request(cursor: Option<serde_json::Value>, limit: usize) -> Vec<u8> {
    let mut fields = json!({"limit": limit});
    if let Some(cursor) = cursor {
        fields["cursor"] = cursor;
    }
    request("conversation_bootstrap_page", &fields)
}

fn without_lf(bytes: &[u8]) -> &[u8] {
    bytes.strip_suffix(b"\n").expect("one LF response")
}
