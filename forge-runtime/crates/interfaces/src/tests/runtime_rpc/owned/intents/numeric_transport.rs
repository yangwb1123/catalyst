use super::*;

#[test]
fn pending_run_intent_rpc_uses_json_safe_numeric_boundaries_before_open() {
    const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;
    let directory = tempdir().expect("temp directory");
    let database = directory.path().join("missing.sqlite3");
    let owner = test_owner("tenant-slate");
    let safe_cursor = process_owned_request_unchecked(
        &database,
        "owned_pending_run_intent_page",
        &json!({
            "owner": owner,
            "conversation_id": "conversation-1",
            "before": {"submitted_at_ms": MAX_SAFE_JSON_INTEGER, "intent_id": "intent-1"},
            "limit": 1
        }),
    );
    assert_eq!(
        safe_cursor["error"]["code"], "query_failed",
        "{safe_cursor}"
    );
    let unsafe_cursor = process_owned_request_unchecked(
        &database,
        "owned_pending_run_intent_page",
        &json!({
            "owner": test_owner("tenant-slate"),
            "conversation_id": "conversation-1",
            "before": {"submitted_at_ms": MAX_SAFE_JSON_INTEGER + 1, "intent_id": "intent-1"},
            "limit": 1
        }),
    );
    assert_eq!(
        unsafe_cursor["error"]["code"], "invalid_pending_run_intent_request",
        "{unsafe_cursor}"
    );

    let safe_timeline = process_owned_request_unchecked(
        &database,
        "owned_pending_run_intent_timeline_page",
        &json!({
            "owner": test_owner("tenant-slate"),
            "conversation_id": "conversation-1",
            "intent_id": "intent-1",
            "after_sequence": MAX_SAFE_JSON_INTEGER,
            "limit": 1
        }),
    );
    assert_eq!(
        safe_timeline["error"]["code"], "query_failed",
        "{safe_timeline}"
    );
    let unsafe_timeline = process_owned_request_unchecked(
        &database,
        "owned_pending_run_intent_timeline_page",
        &json!({
            "owner": test_owner("tenant-slate"),
            "conversation_id": "conversation-1",
            "intent_id": "intent-1",
            "after_sequence": MAX_SAFE_JSON_INTEGER + 1,
            "limit": 1
        }),
    );
    assert_eq!(
        unsafe_timeline["error"]["code"], "invalid_pending_run_intent_request",
        "{unsafe_timeline}"
    );

    let zero_submit = process_owned_request_unchecked(
        &database,
        "submit_owned_prompt_run_intent",
        &json!({
            "owner": test_owner("tenant-slate"),
            "conversation_id": "conversation-1",
            "content": "prompt",
            "idempotency_key": "intent-key-zero",
            "expected_version": 0,
            "profile_id": "server-profile-v1",
            "profile_sha256": vec![0x71; 32]
        }),
    );
    assert_eq!(
        zero_submit["error"]["code"], "invalid_owned_prompt_request",
        "{zero_submit}"
    );

    let safe_submit = process_owned_request_unchecked(
        &database,
        "submit_owned_prompt_run_intent",
        &json!({
            "owner": test_owner("tenant-slate"),
            "conversation_id": "conversation-1",
            "content": "prompt",
            "idempotency_key": "intent-key",
            "expected_version": MAX_SAFE_JSON_INTEGER,
            "profile_id": "server-profile-v1",
            "profile_sha256": vec![0x71; 32]
        }),
    );
    assert_eq!(
        safe_submit["error"]["code"], "query_failed",
        "{safe_submit}"
    );
    let unsafe_submit = process_owned_request_unchecked(
        &database,
        "submit_owned_prompt_run_intent",
        &json!({
            "owner": test_owner("tenant-slate"),
            "conversation_id": "conversation-1",
            "content": "prompt",
            "idempotency_key": "intent-key",
            "expected_version": MAX_SAFE_JSON_INTEGER + 1,
            "profile_id": "server-profile-v1",
            "profile_sha256": vec![0x71; 32]
        }),
    );
    assert_eq!(
        unsafe_submit["error"]["code"], "invalid_owned_prompt_request",
        "{unsafe_submit}"
    );
    assert!(!database.exists());
}

#[test]
fn pending_run_intent_rpc_fail_closes_unsafe_stored_projection_numbers() {
    const MAX_SAFE_JSON_INTEGER: i64 = 9_007_199_254_740_991;
    let case = pending_intent_case();
    let ids = submit_initial_intent(&case);
    let writer = Connection::open(&case.database).expect("open intent fixture for corruption");
    writer
        .execute(
            "UPDATE pending_run_intents SET aggregate_version = ?1 WHERE intent_id = ?2",
            params![MAX_SAFE_JSON_INTEGER + 1, ids.intent_id],
        )
        .expect("inject representable unsafe JSON integer");
    let list = process_owned_request(
        &case.database,
        "owned_pending_run_intent_page",
        &json!({"owner": case.owner, "conversation_id": case.conversation_id, "limit": 25}),
    );
    assert_eq!(list["error"]["code"], "storage_corrupt", "{list}");

    writer
        .execute(
            "UPDATE pending_run_intents SET aggregate_version = 1 WHERE intent_id = ?1",
            [&ids.intent_id],
        )
        .expect("restore intent aggregate version");
    writer
        .execute(
            "UPDATE pending_run_intent_events SET occurred_at_ms = ?1 WHERE intent_id = ?2",
            params![MAX_SAFE_JSON_INTEGER + 1, ids.intent_id],
        )
        .expect("inject unsafe timeline event integer");
    let timeline = process_owned_request(
        &case.database,
        "owned_pending_run_intent_timeline_page",
        &json!({
            "owner": case.owner,
            "conversation_id": case.conversation_id,
            "intent_id": ids.intent_id,
            "after_sequence": 0,
            "limit": 128
        }),
    );
    assert_eq!(timeline["error"]["code"], "storage_corrupt", "{timeline}");
}
