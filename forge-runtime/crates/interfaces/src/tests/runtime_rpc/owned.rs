use std::path::Path;

use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::sync::Arc;
use tempfile::tempdir;

use crate::runtime_domain::{
    BeginRun, ConversationScope, MAX_CONVERSATION_CHANGE_PAGE_LIMIT, RUN_STORE_VERSION,
    RunExecution, RunLimits, RunProvider, RunStore,
};

use super::{create_prompted_fixture, process_owned_request};

#[path = "owned/identity.rs"]
mod identity;
#[path = "owned/imports.rs"]
mod imports;
#[path = "owned/intents.rs"]
mod intents;
#[path = "owned/runs.rs"]
mod runs;
#[path = "owned/scopes_changes.rs"]
mod scopes_changes;

fn assert_rpc_grant_and_replay(database: &Path, grant_fields: &Value) -> String {
    let first = process_owned_request(database, "grant_project_execution_consent", grant_fields);
    assert_eq!(first["ok"], true, "{first}");
    assert_eq!(first["api_version"], super::super::WRITE_API_VERSION);
    assert_eq!(
        first["result"]["grant"]["profile_id"],
        "opaque-server-profile-v1"
    );
    assert_eq!(
        first["result"]["grant"]["profile_sha256"]
            .as_array()
            .unwrap()
            .len(),
        32
    );
    assert_eq!(first["result"]["replayed"], false);
    let grant_id = first["result"]["grant"]["grant_id"]
        .as_str()
        .expect("grant ID")
        .to_owned();

    let replay = process_owned_request(database, "grant_project_execution_consent", grant_fields);
    assert_eq!(replay["ok"], true, "{replay}");
    assert_eq!(replay["result"]["grant"]["grant_id"], grant_id);
    assert_eq!(replay["result"]["replayed"], true);
    grant_id
}

fn assert_rpc_revoke_and_replay(database: &Path, owner: &Value, grant_id: &str) {
    let revoked = process_owned_request(
        database,
        "revoke_project_execution_consent",
        &json!({
            "owner": owner,
            "grant_id": grant_id,
            "idempotency_key": "go-consent-revoke-1"
        }),
    );
    assert_eq!(revoked["ok"], true, "{revoked}");
    assert_eq!(revoked["result"]["replayed"], false);
    let revoke_replay = process_owned_request(
        database,
        "revoke_project_execution_consent",
        &json!({
            "owner": owner,
            "grant_id": grant_id,
            "idempotency_key": "go-consent-revoke-1"
        }),
    );
    assert_eq!(revoke_replay["ok"], true, "{revoke_replay}");
    assert_eq!(revoke_replay["result"]["replayed"], true);
    assert_eq!(
        revoke_replay["result"]["revocation"]["event_id"],
        revoked["result"]["revocation"]["event_id"]
    );
}

fn unix_time_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after UNIX epoch")
            .as_millis(),
    )
    .expect("milliseconds fit in SQLite's signed integer range")
}

fn process_owned_request_unchecked(
    database: &Path,
    operation: &str,
    fields: &serde_json::Value,
) -> serde_json::Value {
    let mut value = json!({
        "api_version": super::WRITE_API_VERSION,
        "request_id": "owned-run-test",
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
    let response = super::process_request(database, &request);
    serde_json::from_slice(super::without_lf(&response)).expect("owner-scoped response JSON")
}

fn create_scoped_owned_conversation(
    database: &Path,
    owner: &Value,
    scope: &Value,
    key: &str,
    project_path: &Path,
) {
    let response = process_owned_request(
        database,
        "create_owned_conversation",
        &json!({
            "owner": owner,
            "scope": scope,
            "title": "Remote project session",
            "idempotency_key": key
        }),
    );
    assert_eq!(response["ok"], true, "{response}");
    assert_eq!(&response["result"]["scope"], scope, "{response}");
    for field in ["owner", "path", "run", "run_id"] {
        assert!(response["result"].get(field).is_none(), "{response}");
    }
    assert!(
        !response
            .to_string()
            .contains(&project_path.to_string_lossy().to_string())
    );
}

fn assert_owned_scopes_and_feed(
    database: &Path,
    owner: &Value,
    expected_scopes: &[(Value, &str); 2],
    project_path: &Path,
) {
    let listed = process_owned_request(
        database,
        "list_owned_conversations",
        &json!({"owner": owner, "limit": 8}),
    );
    assert_eq!(listed["ok"], true, "{listed}");
    let conversations = listed["result"]["conversations"]
        .as_array()
        .expect("owned conversation page");
    assert_eq!(conversations.len(), 2, "{listed}");
    let scopes = conversations
        .iter()
        .map(|entry| entry["conversation"]["scope"].clone())
        .collect::<Vec<_>>();
    assert!(scopes.contains(&expected_scopes[0].0));
    assert!(scopes.contains(&expected_scopes[1].0));
    for entry in conversations {
        assert_other_owner_cannot_read_scoped_conversation(
            database,
            entry["conversation"]["id"]
                .as_str()
                .expect("scoped Conversation ID"),
        );
    }

    let changes = process_owned_request(
        database,
        "owned_conversation_changes_after",
        &json!({"owner": owner, "after_cursor": 0, "limit": 8}),
    );
    assert_eq!(changes["ok"], true, "{changes}");
    let rows = changes["result"]["changes"]
        .as_array()
        .expect("owner change page");
    assert_eq!(rows.len(), 2, "{changes}");
    assert_eq!(changes["result"]["scanned_through_cursor"], 2);
    assert!(
        !changes
            .to_string()
            .contains(&project_path.to_string_lossy().to_string())
    );
}

fn assert_other_owner_cannot_read_scoped_conversation(database: &Path, conversation_id: &str) {
    let other_owner = json!({
        "issuer": "https://identity.example",
        "subject": "account-43",
        "tenant_id": "tenant-slate"
    });
    let listed = process_owned_request(
        database,
        "list_owned_conversations",
        &json!({"owner": other_owner, "limit": 8}),
    );
    assert_eq!(listed["ok"], true, "{listed}");
    assert!(
        listed["result"]["conversations"]
            .as_array()
            .expect("other owner's conversations")
            .is_empty()
    );
    let history = process_owned_request(
        database,
        "owned_conversation_prompt_page",
        &json!({"owner": other_owner, "conversation_id": conversation_id, "limit": 8}),
    );
    assert_eq!(history["error"]["code"], "not_found", "{history}");
}

fn process_raw_request(database: &Path, request: &Value) -> Value {
    let request = serde_json::to_vec(request).expect("encode raw request");
    let response = super::super::process_request(database, &request);
    serde_json::from_slice(super::without_lf(&response)).expect("raw request response JSON")
}

struct OwnerChangeRpcFixture {
    database: std::path::PathBuf,
    owner: Value,
    legacy_id: String,
    owned_ids: [String; 2],
    owned_prompt_id: String,
    foreign_ids: Vec<String>,
}

fn owner_change_rpc_fixture(directory: &Path) -> OwnerChangeRpcFixture {
    let (database, _, legacy_id, _) = create_prompted_fixture(directory);
    let account = test_owner("tenant-slate");
    let other_tenant = test_owner("tenant-cedar");
    let first = create_owned_conversation_with_key(&database, &account, "owned-a1");
    let foreign = create_owned_conversation_with_key(&database, &other_tenant, "owned-b1");
    let first_id = first["id"].as_str().expect("first owner Conversation");
    let foreign_id = foreign["id"].as_str().expect("foreign Conversation");
    let foreign_prompt = append_owned_prompt(
        &database,
        &other_tenant,
        foreign_id,
        "foreign change",
        "foreign-prompt-1",
        1,
    );
    let owned_prompt = append_owned_prompt(
        &database,
        &account,
        first_id,
        "private change",
        "owned-prompt-1",
        1,
    );
    let second = create_owned_conversation_with_key(&database, &account, "owned-a2");
    create_owned_conversation_with_key(&database, &other_tenant, "owned-b2");
    OwnerChangeRpcFixture {
        database,
        owner: account,
        legacy_id,
        owned_ids: [first_id.into(), second["id"].as_str().unwrap().into()],
        owned_prompt_id: owned_prompt["result"]["prompt"]["id"]
            .as_str()
            .expect("owned Prompt ID")
            .into(),
        foreign_ids: [
            foreign_id.into(),
            foreign_prompt["result"]["prompt"]["id"]
                .as_str()
                .expect("foreign Prompt ID")
                .into(),
        ]
        .into(),
    }
}

fn owner_change_rpc_pages(fixture: &OwnerChangeRpcFixture) -> [Value; 3] {
    let first = owned_changes_request(&fixture.database, &fixture.owner, 0, Some(1));
    let prompt = owned_changes_request(
        &fixture.database,
        &fixture.owner,
        first["result"]["scanned_through_cursor"]
            .as_u64()
            .expect("first scanned cursor"),
        Some(1),
    );
    let last = owned_changes_request(
        &fixture.database,
        &fixture.owner,
        prompt["result"]["scanned_through_cursor"]
            .as_u64()
            .expect("second scanned cursor"),
        None,
    );
    [first, prompt, last]
}

fn assert_owner_rpc_foreign_suffix_is_hidden(fixture: &OwnerChangeRpcFixture, page: &Value) {
    let after = page["result"]["scanned_through_cursor"]
        .as_u64()
        .expect("last scanned cursor");
    let suffix = owned_changes_request(&fixture.database, &fixture.owner, after, None);
    assert_eq!(suffix["ok"], true, "{suffix}");
    assert!(suffix["result"]["changes"].as_array().unwrap().is_empty());
    assert_eq!(suffix["result"]["scanned_through_cursor"], after);
    assert_eq!(suffix["result"]["has_more"], false);
}

fn create_owned_conversation_with_key(database: &Path, owner: &Value, key: &str) -> Value {
    let response = process_owned_request(
        database,
        "create_owned_conversation",
        &json!({
            "owner": owner,
            "scope": {"kind":"global"},
            "title": key,
            "idempotency_key": key
        }),
    );
    assert_eq!(response["ok"], true, "{response}");
    response["result"].clone()
}

fn owned_changes_request(
    database: &Path,
    owner: &Value,
    after_cursor: u64,
    limit: Option<usize>,
) -> Value {
    let mut fields = json!({"owner": owner, "after_cursor": after_cursor});
    if let Some(limit) = limit {
        fields["limit"] = json!(limit);
    }
    process_owned_request(database, "owned_conversation_changes_after", &fields)
}

fn assert_owned_change_page(response: &Value, after: u64, through: u64, has_more: bool) {
    assert_eq!(response["ok"], true, "{response}");
    let result = response["result"].as_object().expect("change page object");
    assert_eq!(result.len(), 4);
    assert_eq!(result["after_cursor"], after);
    assert_eq!(result["scanned_through_cursor"], through);
    assert_eq!(result["has_more"], has_more);
    assert!(result.get("head_cursor").is_none());
    assert!(result.get("total_count").is_none());
}

fn test_owner(tenant_id: &str) -> Value {
    json!({
        "issuer": "https://identity.example",
        "subject": "account-42",
        "tenant_id": tenant_id
    })
}

fn create_owned_conversation(database: &Path, owner: &Value) -> Value {
    let created = process_owned_request(
        database,
        "create_owned_conversation",
        &json!({
            "owner": owner,
            "scope": {"kind":"global"},
            "title": "Shared from web",
            "idempotency_key": "create-web-1"
        }),
    );
    assert_eq!(created["ok"], true, "{created}");
    let conversation = created["result"].clone();
    assert_eq!(conversation["title"], "Shared from web");
    assert!(conversation.get("owner").is_none());
    conversation
}

fn assert_owned_list(database: &Path, owner: &Value, conversation_id: &str) {
    let listed = process_owned_request(
        database,
        "list_owned_conversations",
        &json!({"owner": owner, "limit": 8}),
    );
    assert_eq!(listed["ok"], true, "{listed}");
    assert_eq!(
        listed["result"]["conversations"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        listed["result"]["conversations"][0]["conversation"]["id"],
        conversation_id
    );
    assert_eq!(listed["result"]["conversations"][0]["aggregate_version"], 1);
    assert_eq!(listed["result"]["has_more"], false);
}

fn assert_tenant_isolation(database: &Path, owner: &Value) {
    let response = process_owned_request(
        database,
        "list_owned_conversations",
        &json!({"owner": owner, "limit": 8}),
    );
    assert_eq!(response["ok"], true, "{response}");
    assert!(
        response["result"]["conversations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

fn assert_legacy_conversation_is_hidden(database: &Path, owner: &Value, conversation_id: &str) {
    let response = process_owned_request(
        database,
        "owned_conversation_prompt_page",
        &json!({"owner": owner, "conversation_id": conversation_id, "limit": 8}),
    );
    assert_eq!(response["error"]["code"], "not_found");
}

fn append_owned_prompt(
    database: &Path,
    owner: &Value,
    conversation_id: &str,
    content: &str,
    idempotency_key: &str,
    expected_version: u64,
) -> Value {
    process_owned_request(
        database,
        "append_owned_prompt",
        &json!({
            "owner": owner,
            "conversation_id": conversation_id,
            "content": content,
            "idempotency_key": idempotency_key,
            "expected_version": expected_version
        }),
    )
}

fn assert_initial_owned_prompt(response: &Value) {
    assert_eq!(response["ok"], true, "{response}");
    assert_eq!(response["result"]["prompt"]["role"], "user");
    assert_eq!(response["result"]["aggregate_version"], 2);
    assert_eq!(response["result"]["replayed"], false);
    assert!(
        response["result"]["prompt"]
            .get("idempotency_key")
            .is_none()
    );
}

fn assert_owned_prompt_replay(replayed: &Value, original: &Value) {
    assert_eq!(replayed["ok"], true, "{replayed}");
    assert_eq!(replayed["result"]["replayed"], true);
    assert_eq!(
        replayed["result"]["prompt"]["id"],
        original["result"]["prompt"]["id"]
    );
}

fn assert_owned_history(database: &Path, owner: &Value, conversation_id: &str) {
    let history = process_owned_request(
        database,
        "owned_conversation_prompt_page",
        &json!({"owner": owner, "conversation_id": conversation_id, "limit": 8}),
    );
    assert_eq!(history["ok"], true, "{history}");
    assert_eq!(history["result"]["prompts"].as_array().unwrap().len(), 1);
    assert_eq!(
        history["result"]["prompts"][0]["content"],
        "run tests on the remote device"
    );
    assert!(!history.to_string().contains("prompt-web-1"));
}
