use super::*;

#[test]
fn owner_run_rpc_projects_only_status_and_closed_event_types() {
    let fixture = owned_run_fixture();
    assert_owned_run_summary(&fixture);
    assert_owned_run_timeline(&fixture);
}

struct OwnedRunFixture {
    _directory: tempfile::TempDir,
    database: std::path::PathBuf,
    owner: Value,
    conversation_id: String,
}

fn owned_run_fixture() -> OwnedRunFixture {
    let directory = tempdir().expect("temp directory");
    let (database, project_id, _, _) = create_prompted_fixture(directory.path());
    let owner = test_owner("tenant-slate");
    let conversation_id = create_run_conversation(&database, &owner, &project_id);
    let prompt_id = append_run_prompt(&database, &owner, &conversation_id);
    insert_private_run_event(&database, &conversation_id, &project_id, &prompt_id);
    OwnedRunFixture {
        _directory: directory,
        database,
        owner,
        conversation_id,
    }
}

fn create_run_conversation(database: &std::path::Path, owner: &Value, project_id: &str) -> String {
    let conversation = process_owned_request(
        database,
        "create_owned_conversation",
        &json!({
            "owner": owner,
            "scope": {"kind":"project", "id":project_id},
            "title": "Private title",
            "idempotency_key": "run-rpc-conversation"
        }),
    );
    assert_eq!(conversation["ok"], true, "{conversation}");
    conversation["result"]["id"]
        .as_str()
        .expect("owner Conversation ID")
        .to_owned()
}

fn append_run_prompt(database: &std::path::Path, owner: &Value, conversation_id: &str) -> String {
    let prompt = append_owned_prompt(
        database,
        owner,
        conversation_id,
        "private prompt body",
        "run-rpc-prompt-key",
        1,
    );
    prompt["result"]["prompt"]["id"]
        .as_str()
        .expect("owner Prompt ID")
        .to_owned()
}

fn insert_private_run_event(
    database: &std::path::Path,
    conversation_id: &str,
    project_id: &str,
    prompt_id: &str,
) {
    let store = super::super::SqliteHubStore::open(database).expect("open Run store");
    store
        .begin_run(&BeginRun {
            v: RUN_STORE_VERSION,
            run_id: "run-rpc-a".into(),
            conversation_id: conversation_id.into(),
            prompt_id: prompt_id.into(),
            project_id: project_id.into(),
            execution: RunExecution {
                provider: RunProvider::DeterministicRead {
                    path: "private path".into(),
                },
                system_prompt: "private system prompt".into(),
                allowed_read_paths: vec!["private path".into()],
                limits: RunLimits::default(),
            },
            idempotency_key: "private run idempotency key".into(),
            created_at_ms: 10,
        })
        .expect("create local Run fixture");
    insert_private_assistant_event(database, conversation_id);
}

fn insert_private_assistant_event(database: &std::path::Path, conversation_id: &str) {
    let writer = Connection::open(database).expect("open fixture event writer");
    writer
        .execute(
            "INSERT INTO run_events(run_id, seq, event_json) VALUES (?1, 1, ?2)",
            params![
                "run-rpc-a",
                json!({
                    "v": 1,
                    "session_id": conversation_id,
                    "run_id": "run-rpc-a",
                    "seq": 1,
                    "emitted_at_ms": 10,
                    "type": "assistant_delta",
                    "delta": "private assistant content"
                })
                .to_string()
            ],
        )
        .expect("insert private event");
}

fn assert_owned_run_summary(fixture: &OwnedRunFixture) {
    let page = process_owned_request(
        &fixture.database,
        "owned_run_page",
        &json!({"owner": fixture.owner, "conversation_id": fixture.conversation_id, "limit": 25}),
    );
    assert_eq!(page["ok"], true, "{page}");
    let run = &page["result"]["runs"][0];
    assert_eq!(run["run_id"], "run-rpc-a");
    assert_eq!(run["latest_sequence"], 1);
    assert_eq!(run["status"], "nonterminal");
    assert!(run.get("execution").is_none());
    assert!(run.get("idempotency_key").is_none());
    assert!(page["result"].get("next_cursor").is_none());
    assert!(!page.to_string().contains("private"));
}

fn assert_owned_run_timeline(fixture: &OwnedRunFixture) {
    let timeline = process_owned_request(
        &fixture.database,
        "owned_run_timeline_page",
        &json!({
            "owner": fixture.owner,
            "conversation_id": fixture.conversation_id,
            "run_id": "run-rpc-a",
            "after_sequence": 0,
            "limit": 128
        }),
    );
    assert_eq!(timeline["ok"], true, "{timeline}");
    assert_eq!(timeline["result"]["events"][0]["seq"], 1);
    assert_eq!(timeline["result"]["events"][0]["type"], "activity");
    assert!(!timeline.to_string().contains("assistant_delta"));
    assert!(!timeline.to_string().contains("private"));
}

#[test]
fn owner_run_rpc_rejects_unknown_fields_mismatched_cursors_and_unbounded_inputs_before_open() {
    let directory = tempdir().expect("temp directory");
    let database = directory.path().join("missing.sqlite3");
    let owner = test_owner("tenant-slate");
    for (operation, fields, expected_code) in [
        (
            "owned_run_page",
            json!({"owner":owner, "conversation_id":"c1", "limit":0}),
            "invalid_owned_run_request",
        ),
        (
            "owned_run_page",
            json!({"owner":owner, "conversation_id":"c1", "limit":26}),
            "invalid_owned_run_request",
        ),
        (
            "owned_run_page",
            json!({"owner":owner, "conversation_id":"c1", "limit":1, "before_created_at_ms":1}),
            "invalid_owned_run_request",
        ),
        (
            "owned_run_page",
            json!({"owner":owner, "conversation_id":"c1", "limit":1, "unknown":true}),
            "invalid_request",
        ),
        (
            "owned_run_timeline_page",
            json!({"owner":owner, "conversation_id":"c1", "run_id":"r1", "after_sequence":u64::MAX, "limit":1}),
            "invalid_owned_run_request",
        ),
        (
            "owned_run_timeline_page",
            json!({"owner":owner, "conversation_id":"c1", "run_id":"r1", "after_sequence":0, "limit":129}),
            "invalid_owned_run_request",
        ),
        (
            "owned_run_timeline_page",
            json!({"owner":owner, "conversation_id":"c1", "run_id":"r1", "after_sequence":0, "limit":1, "unknown":true}),
            "invalid_request",
        ),
    ] {
        let response = process_owned_request_unchecked(&database, operation, &fields);
        assert_eq!(response["error"]["code"], expected_code, "{response}");
    }
    assert!(!database.exists());
}
