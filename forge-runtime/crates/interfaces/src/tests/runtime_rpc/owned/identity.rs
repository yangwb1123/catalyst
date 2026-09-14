use super::*;

#[test]
fn owned_project_conversation_identity_is_minimal_and_exact_owner_scoped() {
    let case = project_identity_case();
    assert_private_project_identity(&case);
    let missing = assert_foreign_and_legacy_global_are_hidden(&case);
    assert_ownerless_project_is_hidden(&case, &missing);
    assert_non_project_scopes_conflict(&case);
}

struct ProjectIdentityCase {
    directory: tempfile::TempDir,
    database: std::path::PathBuf,
    project_id: String,
    legacy_global_id: String,
    owner: Value,
    conversation_id: String,
}

fn project_identity_case() -> ProjectIdentityCase {
    let directory = tempdir().expect("temp directory");
    let (database, project_id, legacy_global_id, _) = create_prompted_fixture(directory.path());
    let owner = test_owner("tenant-slate");
    let owned_project = process_owned_request(
        &database,
        "create_owned_conversation",
        &json!({
            "owner": owner,
            "scope": {"kind":"project", "id":project_id},
            "title": "private identity title",
            "idempotency_key": "identity-project"
        }),
    );
    assert_eq!(owned_project["ok"], true, "{owned_project}");
    let conversation_id = owned_project["result"]["id"]
        .as_str()
        .expect("owned Project Conversation ID")
        .to_owned();
    let prompt = process_owned_request(
        &database,
        "append_owned_prompt",
        &json!({
            "owner": owner,
            "conversation_id": conversation_id,
            "content": "private prompt content",
            "idempotency_key": "identity-prompt",
            "expected_version": 1
        }),
    );
    assert_eq!(prompt["ok"], true, "{prompt}");

    ProjectIdentityCase {
        directory,
        database,
        project_id,
        legacy_global_id,
        owner,
        conversation_id,
    }
}

fn assert_private_project_identity(case: &ProjectIdentityCase) {
    let identity = process_owned_request(
        &case.database,
        "owned_project_conversation_identity",
        &json!({"owner": case.owner, "conversation_id": case.conversation_id}),
    );
    assert_eq!(identity["ok"], true, "{identity}");
    let result = identity["result"].as_object().expect("identity object");
    assert_eq!(result.len(), 2);
    assert_eq!(result["conversation_id"], case.conversation_id);
    assert_eq!(result["project_id"], case.project_id);
    assert!(!identity.to_string().contains("private identity title"));
    assert!(!identity.to_string().contains("private prompt content"));
    assert!(
        !identity
            .to_string()
            .contains(&case.directory.path().display().to_string())
    );
}

fn assert_foreign_and_legacy_global_are_hidden(case: &ProjectIdentityCase) -> Value {
    let foreign_owner = test_owner("tenant-cedar");
    let foreign = process_owned_request(
        &case.database,
        "owned_project_conversation_identity",
        &json!({"owner": foreign_owner, "conversation_id": case.conversation_id}),
    );
    let legacy_global = process_owned_request(
        &case.database,
        "owned_project_conversation_identity",
        &json!({"owner": case.owner, "conversation_id": case.legacy_global_id}),
    );
    let missing = process_owned_request(
        &case.database,
        "owned_project_conversation_identity",
        &json!({"owner": case.owner, "conversation_id": "missing-conversation"}),
    );
    assert_eq!(foreign, missing);
    assert_eq!(legacy_global, missing);
    for hidden in [&foreign, &legacy_global, &missing] {
        assert_eq!(hidden["error"]["code"], "not_found", "{hidden}");
        assert!(hidden.get("result").is_none());
        assert!(!hidden.to_string().contains(&case.project_id));
    }
    missing
}

fn assert_ownerless_project_is_hidden(case: &ProjectIdentityCase, missing: &Value) {
    let store = Arc::new(super::super::SqliteHubStore::open(&case.database).expect("reopen Hub"));
    let service = super::super::HubService::new(store);
    let legacy_project = service
        .create_session(
            &ConversationScope::Project(case.project_id.clone()),
            "legacy Project title",
            "identity-legacy-project",
        )
        .expect("create ownerless legacy Project Conversation");
    let ownerless = process_owned_request(
        &case.database,
        "owned_project_conversation_identity",
        &json!({"owner": case.owner, "conversation_id": legacy_project.id}),
    );
    assert_eq!(ownerless["error"]["code"], "not_found", "{ownerless}");
    assert_eq!(ownerless, missing.clone());
    assert!(ownerless.get("result").is_none());
    assert!(!ownerless.to_string().contains(&case.project_id));
}

fn assert_non_project_scopes_conflict(case: &ProjectIdentityCase) {
    let store = Arc::new(super::super::SqliteHubStore::open(&case.database).expect("reopen Hub"));
    let service = super::super::HubService::new(store);
    let global = create_owned_conversation(&case.database, &case.owner);
    let group = service
        .create_group("Identity group", "identity-group")
        .expect("create Group");
    let grouped = process_owned_request(
        &case.database,
        "create_owned_conversation",
        &json!({
            "owner": case.owner,
            "scope": {"kind":"group", "id":group.id},
            "title": "private Group title",
            "idempotency_key": "identity-group-conversation"
        }),
    );
    assert_eq!(grouped["ok"], true, "{grouped}");

    for conversation_id in [
        global["id"].as_str().expect("Global ID"),
        grouped["result"]["id"].as_str().expect("Group ID"),
    ] {
        let non_project = process_owned_request(
            &case.database,
            "owned_project_conversation_identity",
            &json!({"owner": case.owner, "conversation_id": conversation_id}),
        );
        assert_eq!(non_project["error"]["code"], "conflict", "{non_project}");
        assert!(non_project.get("result").is_none());
        assert!(!non_project.to_string().contains(&case.project_id));
    }
}
#[test]
fn owned_project_identity_rpc_rejects_unknown_fields_before_database_open() {
    let directory = tempdir().expect("temp directory");
    let database = directory.path().join("missing.sqlite3");
    let rejected = process_owned_request_unchecked(
        &database,
        "owned_project_conversation_identity",
        &json!({
            "owner": test_owner("tenant-slate"),
            "conversation_id": "conversation-1",
            "project_id": "caller-supplied-project"
        }),
    );
    assert_eq!(rejected["error"]["code"], "invalid_request");
    assert!(!database.exists());
}
