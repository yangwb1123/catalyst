use super::*;

#[test]
fn owner_import_rpc_commits_an_entire_transcript_and_replays_after_followup() {
    let directory = tempdir().expect("temp directory");
    let (database, _, _, _) = create_prompted_fixture(directory.path());
    let account = test_owner("tenant-slate");
    let prompts = json!([
        {"role":"user", "content":"inspect the change"},
        {"role":"assistant", "content":"I will inspect it."}
    ]);
    let imported = import_transcript(&database, &account, &prompts);
    let conversation_id = assert_import_contract(&imported);
    assert_import_replay(&database, &account, &prompts, Some(&conversation_id));
    append_import_followup(&database, &account, &conversation_id);
    assert_import_replay(&database, &account, &prompts, None);
}

fn import_transcript(database: &std::path::Path, account: &Value, prompts: &Value) -> Value {
    process_owned_request(
        database,
        "import_owned_conversation",
        &json!({
            "owner": account,
            "title": "Imported review",
            "prompts": prompts,
            "idempotency_key": "import-once"
        }),
    )
}

fn assert_import_contract(imported: &Value) -> String {
    assert_eq!(imported["ok"], true, "{imported}");
    assert_eq!(
        imported["result"]["conversation"]["scope"]["kind"],
        "global"
    );
    assert_eq!(imported["result"]["aggregate_version"], 3);
    assert_eq!(imported["result"]["imported_prompt_count"], 2);
    assert_eq!(imported["result"]["replayed"], false);
    for field in ["owner", "path", "run", "run_id", "prompts"] {
        assert!(imported["result"].get(field).is_none(), "{imported}");
    }
    imported["result"]["conversation"]["id"]
        .as_str()
        .expect("imported ID")
        .to_owned()
}

fn assert_import_replay(
    database: &std::path::Path,
    account: &Value,
    prompts: &Value,
    expected_conversation_id: Option<&str>,
) {
    let retry = import_transcript(database, account, prompts);
    assert_eq!(retry["ok"], true, "{retry}");
    if let Some(expected_id) = expected_conversation_id {
        assert_eq!(retry["result"]["conversation"]["id"], expected_id);
    }
    assert_eq!(retry["result"]["replayed"], true);
}

fn append_import_followup(database: &std::path::Path, account: &Value, conversation_id: &str) {
    let followup = process_owned_request(
        database,
        "append_owned_prompt",
        &json!({
            "owner": account,
            "conversation_id": conversation_id,
            "content": "continue",
            "idempotency_key": "followup",
            "expected_version": 3
        }),
    );
    assert_eq!(followup["ok"], true, "{followup}");
}
#[test]
fn owner_import_rpc_rejects_non_user_visible_or_oversized_requests_before_open() {
    let directory = tempdir().expect("temp directory");
    let database = directory.path().join("missing.sqlite3");
    for prompts in [
        json!([{"role":"tool", "content":"payload"}]),
        json!([{"role":"user", "content":" "}]),
        json!([
            {"role":"user", "content": "a".repeat(256 * 1024)},
            {"role":"assistant", "content": "b"}
        ]),
    ] {
        let response = process_owned_request(
            &database,
            "import_owned_conversation",
            &json!({
                "owner": test_owner("tenant-slate"),
                "title": "Import",
                "prompts": prompts,
                "idempotency_key": "import-key"
            }),
        );
        assert_eq!(
            response["error"]["code"],
            "invalid_owned_conversation_import"
        );
        assert!(!database.exists());
    }
    let unknown_prompt_field = process_raw_request(
        &database,
        &json!({
            "api_version": super::super::WRITE_API_VERSION,
            "request_id": "unknown-import-prompt-field",
            "operation": "import_owned_conversation",
            "owner": test_owner("tenant-slate"),
            "title": "Import",
            "prompts": [{"role":"user", "content":"ok", "tool_payload":"must be rejected"}],
            "idempotency_key": "import-key"
        }),
    );
    assert_eq!(unknown_prompt_field["error"]["code"], "invalid_request");
    assert!(!database.exists());
}
#[test]
fn owner_import_rpc_accepts_maximum_text_when_json_escaping_expands_body() {
    let directory = tempdir().expect("temp directory");
    let (database, _, _, _) = create_prompted_fixture(directory.path());
    // NUL is valid non-whitespace UTF-8 text but expands to six JSON bytes.
    let content = format!("{}x", "\0".repeat(256 * 1024 - 1));
    let response = process_owned_request(
        &database,
        "import_owned_conversation",
        &json!({
            "owner": test_owner("tenant-slate"),
            "title": "Large escaped import",
            "prompts": [{"role":"user", "content": content}],
            "idempotency_key": "large-import"
        }),
    );
    assert_eq!(response["ok"], true, "{response}");
    assert_eq!(response["result"]["imported_prompt_count"], 1);
}
