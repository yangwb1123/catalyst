use super::*;
use rusqlite::Connection;

#[test]
fn conversation_timestamps_above_json_safe_integer_fail_rpc_transport() {
    let directory = tempdir().expect("temp directory");
    let (database, _, _, _) = create_prompted_fixture(directory.path());
    Connection::open(&database)
        .expect("open fixture database")
        .execute(
            "UPDATE conversations SET created_at_ms = ?1, updated_at_ms = ?1",
            [9_007_199_254_740_992_i64],
        )
        .expect("set transport-unsafe timestamp");

    let snapshot = process_request(&database, &request("snapshot_at_cursor", &json!({})));
    let snapshot: serde_json::Value =
        serde_json::from_slice(without_lf(&snapshot)).expect("snapshot response JSON");
    assert_eq!(snapshot["ok"], false);
    assert_eq!(snapshot["error"]["code"], "query_failed");
}

#[test]
fn global_change_timestamp_above_json_safe_integer_fails_rpc_transport() {
    let directory = tempdir().expect("temp directory");
    let (database, _, _, _) = create_prompted_fixture(directory.path());
    Connection::open(&database)
        .expect("open fixture database")
        .execute(
            "UPDATE conversation_changes SET created_at_ms = ?1",
            [9_007_199_254_740_992_i64],
        )
        .expect("set transport-unsafe change timestamp");

    let changes = process_request(
        &database,
        &request(
            "conversation_changes_after",
            &json!({"after_cursor": 0, "limit": 8}),
        ),
    );
    let changes: serde_json::Value =
        serde_json::from_slice(without_lf(&changes)).expect("change response JSON");
    assert_eq!(changes["ok"], false);
    assert_eq!(changes["error"]["code"], "query_failed");
}

#[test]
fn global_change_cursor_above_json_safe_integer_is_rejected_before_open() {
    let directory = tempdir().expect("temp directory");
    let database = directory.path().join("missing.sqlite3");
    let response = process_request(
        &database,
        &request(
            "conversation_changes_after",
            &json!({"after_cursor": 9_007_199_254_740_992_u64, "limit": 1}),
        ),
    );
    let response: serde_json::Value =
        serde_json::from_slice(without_lf(&response)).expect("unsafe cursor response");
    assert_eq!(response["error"]["code"], "invalid_cursor");
    assert!(!database.exists());
}

#[test]
fn owned_change_timestamp_above_json_safe_integer_fails_rpc_transport() {
    let directory = tempdir().expect("temp directory");
    let (database, _, _, _) = create_prompted_fixture(directory.path());
    let owner = json!({
        "issuer": "https://identity.example",
        "subject": "account-42",
        "tenant_id": "tenant-slate"
    });
    let created = process_owned_request(
        &database,
        "create_owned_conversation",
        &json!({
            "owner": owner,
            "scope": {"kind": "global"},
            "title": "Shared",
            "idempotency_key": "unsafe-change-1"
        }),
    );
    assert_eq!(created["ok"], true, "{created}");
    let conversation_id = created["result"]["id"].as_str().expect("conversation ID");
    Connection::open(&database)
        .expect("open fixture database")
        .execute(
            "UPDATE conversation_changes SET created_at_ms = ?1 WHERE conversation_id = ?2",
            rusqlite::params![9_007_199_254_740_992_i64, conversation_id],
        )
        .expect("set owner change timestamp");

    let changes = process_owned_request(
        &database,
        "owned_conversation_changes_after",
        &json!({
            "owner": {
                "issuer": "https://identity.example",
                "subject": "account-42",
                "tenant_id": "tenant-slate"
            },
            "after_cursor": 0,
            "limit": 8
        }),
    );
    assert_eq!(changes["ok"], false);
    assert_eq!(changes["error"]["code"], "storage_corrupt");
}

#[test]
fn owned_aggregate_version_above_json_safe_integer_fails_rpc_transport() {
    let directory = tempdir().expect("temp directory");
    let (database, _, _, _) = create_prompted_fixture(directory.path());
    let owner = json!({
        "issuer": "https://identity.example",
        "subject": "account-42",
        "tenant_id": "tenant-slate"
    });
    let created = process_owned_request(
        &database,
        "create_owned_conversation",
        &json!({
            "owner": owner,
            "scope": {"kind": "global"},
            "title": "Shared",
            "idempotency_key": "unsafe-aggregate-1"
        }),
    );
    assert_eq!(created["ok"], true, "{created}");
    let conversation_id = created["result"]["id"].as_str().expect("conversation ID");
    Connection::open(&database)
        .expect("open fixture database")
        .execute(
            "UPDATE conversation_change_heads SET last_version = ?1 WHERE conversation_id = ?2",
            rusqlite::params![9_007_199_254_740_992_i64, conversation_id],
        )
        .expect("set transport-unsafe aggregate version");

    let listed = process_owned_request(
        &database,
        "list_owned_conversations",
        &json!({
            "owner": {
                "issuer": "https://identity.example",
                "subject": "account-42",
                "tenant_id": "tenant-slate"
            },
            "limit": 8
        }),
    );
    assert_eq!(listed["ok"], false, "{listed}");
    assert_eq!(listed["error"]["code"], "storage_corrupt");

    let detail = process_owned_request(
        &database,
        "get_owned_conversation",
        &json!({
            "owner": {
                "issuer": "https://identity.example",
                "subject": "account-42",
                "tenant_id": "tenant-slate"
            },
            "conversation_id": conversation_id
        }),
    );
    assert_eq!(detail["ok"], false, "{detail}");
    assert_eq!(detail["error"]["code"], "storage_corrupt");
}

#[test]
fn owned_prompt_expected_version_above_json_safe_integer_is_rejected_before_open() {
    let directory = tempdir().expect("temp directory");
    let database = directory.path().join("missing.sqlite3");
    let response = process_owned_request(
        &database,
        "append_owned_prompt",
        &json!({
            "owner": {
                "issuer": "https://identity.example",
                "subject": "account-42",
                "tenant_id": "tenant-slate"
            },
            "conversation_id": "conversation-1",
            "content": "ship it",
            "idempotency_key": "unsafe-version-1",
            "expected_version": 9_007_199_254_740_992_u64
        }),
    );
    assert_eq!(response["error"]["code"], "invalid_owned_prompt_request");
    assert!(!database.exists());
}
