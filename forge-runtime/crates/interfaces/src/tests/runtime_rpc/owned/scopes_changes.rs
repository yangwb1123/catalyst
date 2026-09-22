use super::*;

#[test]
fn owner_scoped_rpc_creates_project_and_group_conversations_without_paths_or_run_authority() {
    let directory = tempdir().expect("temp directory");
    let (database, project_id, _, _) = create_prompted_fixture(directory.path());
    let owner = test_owner("tenant-slate");
    let store = Arc::new(super::super::SqliteHubStore::open(&database).expect("reopen Hub"));
    let service = super::super::HubService::new(store);
    let group = service
        .create_group("Known group", "known-group")
        .expect("create existing group");

    let expected_scopes = [
        (json!({"kind":"project","id":project_id}), "project-key"),
        (json!({"kind":"group","id":group.id}), "group-key"),
    ];
    for (scope, key) in &expected_scopes {
        create_scoped_owned_conversation(&database, &owner, scope, key, directory.path());
    }

    assert_owned_scopes_and_feed(&database, &owner, &expected_scopes, directory.path());
}
#[test]
fn owner_change_rpc_pages_only_exact_principal_changes() {
    let directory = tempdir().expect("temp directory");
    let fixture = owner_change_rpc_fixture(directory.path());
    let [first_page, prompt_page, final_page] = owner_change_rpc_pages(&fixture);

    assert_owned_change_page(&first_page, 0, 1, true);
    assert_eq!(first_page["result"]["changes"].as_array().unwrap().len(), 1);
    assert_eq!(
        first_page["result"]["changes"][0]["conversation_id"],
        fixture.owned_ids[0]
    );
    assert_owned_change_page(&prompt_page, 1, 2, true);
    assert_eq!(
        prompt_page["result"]["changes"][0]["entity_id"],
        fixture.owned_prompt_id
    );
    assert_owned_change_page(&final_page, 2, 3, false);
    assert_eq!(
        final_page["result"]["changes"][0]["conversation_id"],
        fixture.owned_ids[1]
    );
    for response in [&first_page, &prompt_page, &final_page] {
        let encoded = response.to_string();
        assert!(
            fixture
                .foreign_ids
                .iter()
                .chain(std::iter::once(&fixture.legacy_id))
                .all(|foreign_id| !encoded.contains(foreign_id))
        );
    }
    assert_owner_rpc_foreign_suffix_is_hidden(&fixture, &final_page);
}
#[test]
fn owner_change_rpc_rejects_invalid_owner_and_page_bounds() {
    let directory = tempdir().expect("temp directory");
    let (database, _, _, _) = create_prompted_fixture(directory.path());
    let account = test_owner("tenant-slate");

    for limit in [0, MAX_CONVERSATION_CHANGE_PAGE_LIMIT + 1] {
        let response = process_owned_request(
            &database,
            "owned_conversation_changes_after",
            &json!({"owner": account, "after_cursor": 0, "limit": limit}),
        );
        assert_eq!(
            response["error"]["code"],
            "invalid_owned_conversation_request"
        );
    }
    let invalid_owner =
        json!({"issuer":"https://identity.example","subject":" ","tenant_id":"tenant-slate"});
    let response = process_owned_request(
        &database,
        "owned_conversation_changes_after",
        &json!({"owner": invalid_owner, "after_cursor": 0}),
    );
    assert_eq!(
        response["error"]["code"],
        "invalid_owned_conversation_request"
    );

    let unsafe_cursor = process_owned_request(
        &database,
        "owned_conversation_changes_after",
        &json!({
            "owner": test_owner("tenant-slate"),
            "after_cursor": 9_007_199_254_740_992_u64,
            "limit": 1
        }),
    );
    assert_eq!(
        unsafe_cursor["error"]["code"],
        "invalid_owned_conversation_request"
    );

    let missing_cursor = process_raw_request(
        &database,
        &json!({
            "api_version": super::super::WRITE_API_VERSION,
            "request_id": "missing-cursor",
            "operation": "owned_conversation_changes_after",
            "owner": account
        }),
    );
    assert_eq!(missing_cursor["error"]["code"], "invalid_request");
}
#[test]
fn owner_change_rpc_rejects_future_and_negative_cursors() {
    let directory = tempdir().expect("temp directory");
    let (database, _, _, _) = create_prompted_fixture(directory.path());
    let account = test_owner("tenant-slate");
    let ahead = process_owned_request(
        &database,
        "owned_conversation_changes_after",
        &json!({"owner": account, "after_cursor": 10000}),
    );
    assert_eq!(ahead["error"]["code"], "conflict");
    let negative = json!({
        "api_version": super::super::WRITE_API_VERSION,
        "request_id": "negative-cursor",
        "operation": "owned_conversation_changes_after",
            "owner": account.clone(),
        "after_cursor": -1
    });
    let response = process_raw_request(&database, &negative);
    assert_eq!(response["error"]["code"], "invalid_request");
}
