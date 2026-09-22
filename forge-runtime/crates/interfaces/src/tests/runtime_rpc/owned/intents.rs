use super::*;

#[path = "intents/numeric_transport.rs"]
mod numeric_transport;

#[test]
fn owner_scoped_rpc_creates_lists_reads_and_appends_with_isolation_and_cas() {
    let directory = tempdir().expect("temp directory");
    let (database, _, legacy_conversation_id, _) = create_prompted_fixture(directory.path());
    let owner = test_owner("tenant-slate");
    let other_tenant = test_owner("tenant-cedar");
    let conversation = create_owned_conversation(&database, &owner);
    let conversation_id = conversation["id"].as_str().expect("Conversation ID");

    assert_owned_list(&database, &owner, conversation_id);
    assert_tenant_isolation(&database, &other_tenant);
    assert_legacy_conversation_is_hidden(&database, &owner, &legacy_conversation_id);

    let appended = append_owned_prompt(
        &database,
        &owner,
        conversation_id,
        "run tests on the remote device",
        "prompt-web-1",
        1,
    );
    assert_initial_owned_prompt(&appended);
    let replayed = append_owned_prompt(
        &database,
        &owner,
        conversation_id,
        "run tests on the remote device",
        "prompt-web-1",
        1,
    );
    assert_owned_prompt_replay(&replayed, &appended);

    let stale = append_owned_prompt(
        &database,
        &owner,
        conversation_id,
        "a competing prompt",
        "prompt-web-2",
        1,
    );
    assert_eq!(stale["error"]["code"], "conflict");
    assert_owned_history(&database, &owner, conversation_id);
}
#[test]
fn project_execution_consent_rpc_grants_replays_and_revokes_exact_owner_scope() {
    let directory = tempdir().expect("temp directory");
    let (database, project_id, _, _) = create_prompted_fixture(directory.path());
    let owner = test_owner("tenant-slate");
    let profile_digest = (0..32_u8).collect::<Vec<_>>();
    let expiry = unix_time_ms() + 60_000;
    let grant_fields = json!({
        "owner": owner,
        "project_id": project_id,
        "profile_id": "opaque-server-profile-v1",
        "profile_sha256": profile_digest,
        "expires_at_ms": expiry,
        "idempotency_key": "go-consent-grant-1"
    });
    let grant_id = assert_rpc_grant_and_replay(&database, &grant_fields);
    assert_rpc_revoke_and_replay(&database, &owner, &grant_id);
}
#[test]
fn pending_run_intent_rpc_submits_replays_after_revocation_and_reads_separate_projections() {
    let case = pending_intent_case();
    let intent = submit_initial_intent(&case);
    assert_stale_intent_conflicts(&case);
    revoke_consent_after_later_prompt(&case);
    assert_replay_preserves_original_intent(&case, &intent);
    assert_intent_projections_hide_payloads(&case, &intent);
    assert_no_run_state_was_created(&case);
}

struct PendingIntentCase {
    _directory: tempfile::TempDir,
    database: std::path::PathBuf,
    owner: Value,
    conversation_id: String,
    grant_id: String,
    profile_digest: Vec<u8>,
    submit_fields: Value,
}

struct PendingIntentIds {
    intent_id: String,
    prompt_id: String,
}

fn pending_intent_case() -> PendingIntentCase {
    let directory = tempdir().expect("temp directory");
    let (database, project_id, _, _) = create_prompted_fixture(directory.path());
    let owner = test_owner("tenant-slate");
    let conversation_id = create_intent_conversation(&database, &owner, &project_id);
    let profile_digest = vec![0x71; 32];
    let grant_id = grant_intent_consent(&database, &owner, &project_id, &profile_digest);
    let submit_fields = json!({
        "owner": owner,
        "conversation_id": conversation_id,
        "content": "first line\nsecond line",
        "idempotency_key": "intent-submit-once",
        "expected_version": 1,
        "profile_id": "opaque-profile-v1",
        "profile_sha256": profile_digest
    });
    PendingIntentCase {
        _directory: directory,
        database,
        owner,
        conversation_id,
        grant_id,
        profile_digest,
        submit_fields,
    }
}

fn create_intent_conversation(
    database: &std::path::Path,
    owner: &Value,
    project_id: &str,
) -> String {
    let conversation = process_owned_request(
        database,
        "create_owned_conversation",
        &json!({
            "owner": owner,
            "scope": {"kind":"project", "id":project_id},
            "title": "Intent conversation",
            "idempotency_key": "intent-project-conversation"
        }),
    );
    assert_eq!(conversation["ok"], true, "{conversation}");
    conversation["result"]["id"]
        .as_str()
        .expect("Conversation ID")
        .to_owned()
}

fn grant_intent_consent(
    database: &std::path::Path,
    owner: &Value,
    project_id: &str,
    profile_digest: &[u8],
) -> String {
    let grant = process_owned_request(
        database,
        "grant_project_execution_consent",
        &json!({
            "owner": owner,
            "project_id": project_id,
            "profile_id": "opaque-profile-v1",
            "profile_sha256": profile_digest,
            "expires_at_ms": unix_time_ms() + 60_000,
            "idempotency_key": "intent-consent-grant"
        }),
    );
    assert_eq!(grant["ok"], true, "{grant}");
    grant["result"]["grant"]["grant_id"]
        .as_str()
        .expect("consent grant ID")
        .to_owned()
}

fn submit_initial_intent(case: &PendingIntentCase) -> PendingIntentIds {
    let submitted = process_owned_request(
        &case.database,
        "submit_owned_prompt_run_intent",
        &case.submit_fields,
    );
    assert_eq!(submitted["ok"], true, "{submitted}");
    assert_eq!(submitted["api_version"], super::super::WRITE_API_VERSION);
    assert_eq!(
        submitted["result"]["prompt"]["content"],
        "first line\nsecond line"
    );
    assert_eq!(submitted["result"]["intent"]["status"], "pending");
    assert_eq!(
        submitted["result"]["intent"]["profile_id"],
        "opaque-profile-v1"
    );
    assert_eq!(submitted["result"]["initial_event"]["seq"], 1);
    assert_eq!(submitted["result"]["initial_event"]["type"], "submitted");
    assert_eq!(submitted["result"]["replayed"], false);
    assert!(
        submitted["result"]["intent"]
            .get("profile_sha256")
            .is_none()
    );
    assert!(
        submitted["result"]["initial_event"]
            .get("content")
            .is_none()
    );
    PendingIntentIds {
        intent_id: submitted["result"]["intent"]["intent_id"]
            .as_str()
            .expect("pending intent ID")
            .to_owned(),
        prompt_id: submitted["result"]["prompt"]["id"]
            .as_str()
            .expect("Prompt ID")
            .to_owned(),
    }
}

fn assert_stale_intent_conflicts(case: &PendingIntentCase) {
    let stale = process_owned_request(
        &case.database,
        "submit_owned_prompt_run_intent",
        &json!({
            "owner": case.owner,
            "conversation_id": case.conversation_id,
            "content": "stale submission",
            "idempotency_key": "stale-intent",
            "expected_version": 1,
            "profile_id": "opaque-profile-v1",
            "profile_sha256": case.profile_digest
        }),
    );
    assert_eq!(stale["error"]["code"], "conflict");
}

fn revoke_consent_after_later_prompt(case: &PendingIntentCase) {
    let later = append_owned_prompt(
        &case.database,
        &case.owner,
        &case.conversation_id,
        "later prompt",
        "later-prompt-key",
        2,
    );
    assert_eq!(later["ok"], true, "{later}");
    let revoke = process_owned_request(
        &case.database,
        "revoke_project_execution_consent",
        &json!({
            "owner": case.owner,
            "grant_id": case.grant_id,
            "idempotency_key": "intent-consent-revoke"
        }),
    );
    assert_eq!(revoke["ok"], true, "{revoke}");
}

fn assert_replay_preserves_original_intent(case: &PendingIntentCase, ids: &PendingIntentIds) {
    assert_intent_replay(case, ids);
    assert_changed_intent_content_conflicts(case);
}

fn assert_intent_replay(case: &PendingIntentCase, ids: &PendingIntentIds) {
    let mut replay_fields = case.submit_fields.clone();
    replay_fields["profile_id"] = json!("new-server-profile");
    replay_fields["profile_sha256"] = json!(vec![0x72; 32]);
    let replay = process_owned_request(
        &case.database,
        "submit_owned_prompt_run_intent",
        &replay_fields,
    );
    assert_eq!(replay["ok"], true, "{replay}");
    assert_eq!(replay["result"]["replayed"], true);
    assert_eq!(replay["result"]["prompt"]["id"], ids.prompt_id);
    assert_eq!(replay["result"]["intent"]["intent_id"], ids.intent_id);
    assert_eq!(
        replay["result"]["intent"]["profile_id"],
        "opaque-profile-v1"
    );
}

fn assert_changed_intent_content_conflicts(case: &PendingIntentCase) {
    let changed_content = process_owned_request(
        &case.database,
        "submit_owned_prompt_run_intent",
        &json!({
            "owner": case.owner,
            "conversation_id": case.conversation_id,
            "content": "changed body",
            "idempotency_key": "intent-submit-once",
            "expected_version": 1,
            "profile_id": "new-server-profile",
            "profile_sha256": vec![0x72; 32]
        }),
    );
    assert_eq!(changed_content["error"]["code"], "conflict");
}

fn assert_intent_projections_hide_payloads(case: &PendingIntentCase, ids: &PendingIntentIds) {
    assert_intent_list_hides_payloads(case, &ids.intent_id);
    assert_intent_timeline_hides_payloads(case, &ids.intent_id);
}

fn assert_intent_list_hides_payloads(case: &PendingIntentCase, intent_id: &str) {
    let list = process_owned_request(
        &case.database,
        "owned_pending_run_intent_page",
        &json!({"owner": case.owner, "conversation_id": case.conversation_id, "limit": 25}),
    );
    assert_eq!(list["ok"], true, "{list}");
    assert_eq!(list["result"]["intents"][0]["intent_id"], intent_id);
    assert_eq!(list["result"]["intents"][0]["status"], "pending");
    assert!(!list.to_string().contains("first line"));
    assert!(!list.to_string().contains("profile_sha256"));
}

fn assert_intent_timeline_hides_payloads(case: &PendingIntentCase, intent_id: &str) {
    let timeline = process_owned_request(
        &case.database,
        "owned_pending_run_intent_timeline_page",
        &json!({
            "owner": case.owner,
            "conversation_id": case.conversation_id,
            "intent_id": intent_id,
            "after_sequence": 0,
            "limit": 128
        }),
    );
    assert_eq!(timeline["ok"], true, "{timeline}");
    assert_eq!(timeline["result"]["events"][0]["type"], "submitted");
    assert!(!timeline.to_string().contains("first line"));
    assert!(!timeline.to_string().contains("profile_sha256"));
}

fn assert_no_run_state_was_created(case: &PendingIntentCase) {
    let connection = Connection::open(&case.database).expect("inspect inert intent database");
    for table in ["runs", "run_events"] {
        assert_eq!(
            connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("count unchanged Run state"),
            0
        );
    }
}

#[test]
fn pending_run_intent_rpc_rejects_malformed_and_caller_owned_execution_fields_before_open() {
    let directory = tempdir().expect("temp directory");
    let database = directory.path().join("missing.sqlite3");
    let base = json!({
        "owner": test_owner("tenant-slate"),
        "conversation_id": "conversation-1",
        "content": "prompt",
        "idempotency_key": "intent-key",
        "expected_version": 1,
        "profile_id": "server-profile-v1",
        "profile_sha256": vec![0x71; 31]
    });
    let malformed =
        process_owned_request_unchecked(&database, "submit_owned_prompt_run_intent", &base);
    assert_eq!(malformed["error"]["code"], "invalid_request");
    assert!(!database.exists());

    let mut caller_owned = base;
    caller_owned["profile_sha256"] = json!(vec![0x71; 32]);
    caller_owned["project_id"] = json!("caller-project");
    caller_owned["provider"] = json!("caller-provider");
    caller_owned["runner"] = json!({"path": "/caller/path", "tool": "shell"});
    let denied =
        process_owned_request_unchecked(&database, "submit_owned_prompt_run_intent", &caller_owned);
    assert_eq!(denied["error"]["code"], "invalid_request");
    assert!(!database.exists());
}

#[test]
fn project_execution_consent_rpc_rejects_malformed_owner_digest_and_unknown_fields_before_open() {
    let directory = tempdir().expect("temp directory");
    let database = directory.path().join("missing.sqlite3");
    let base = json!({
        "owner": test_owner("tenant-slate"),
        "project_id": "project-1",
        "profile_id": "profile-1",
        "profile_sha256": [1, 2, 3],
        "expires_at_ms": 2_000_000_000_000_u64,
        "idempotency_key": "consent-key"
    });
    let malformed =
        process_owned_request_unchecked(&database, "grant_project_execution_consent", &base);
    assert_eq!(malformed["error"]["code"], "invalid_request");
    assert!(!database.exists());

    let mut unknown = base;
    unknown["profile_sha256"] = json!(vec![0_u8; 32]);
    unknown["profile_payload"] = json!({"provider": "not allowed"});
    let unknown =
        process_owned_request_unchecked(&database, "grant_project_execution_consent", &unknown);
    assert_eq!(unknown["error"]["code"], "invalid_request");
    assert!(!database.exists());

    let invalid_owner = process_owned_request_unchecked(
        &database,
        "revoke_project_execution_consent",
        &json!({
            "owner": {"issuer": "issuer", "subject": "subject", "tenant_id": " "},
            "grant_id": "grant-1",
            "idempotency_key": "revoke-key"
        }),
    );
    assert_eq!(
        invalid_owner["error"]["code"],
        "invalid_project_execution_consent_request"
    );
    assert!(!database.exists());
}
