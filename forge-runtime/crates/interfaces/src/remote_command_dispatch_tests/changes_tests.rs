use super::*;

#[tokio::test]
async fn visible_instance_change_list_reads_converged_pair_and_filters_rows() {
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/session-view ",
            response_status: "200 OK",
            response: session_view(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/resource-view ",
            response_status: "200 OK",
            response: resource_view(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/conversation-changes?after_cursor=4&limit=128 ",
            response_status: "200 OK",
            response: mixed_conversation_changes(),
        },
    ]);
    let command = RemoteCommand::ChangesList {
        after_cursor: Some(4),
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let returned = execute_changes_command(&client, &command)
        .await
        .expect("visible instance change list");
    assert_eq!(returned["scanned_through_cursor"], 6);
    assert_eq!(returned["changes"].as_array().unwrap().len(), 1);
    assert_eq!(
        returned["changes"][0]["conversation_id"],
        "conversation-001"
    );
    server.join().expect("mock server");
}

#[test]
fn instance_change_projection_hides_rows_without_rewriting_owner_cursor() {
    let scope = crate::client_instance_session_scope::ClientInstanceSessionScope {
        instance_id: "client-web-001".into(),
        client_kind: "web".into(),
        session_ids: ["conversation-001".to_owned()].into_iter().collect(),
    };
    let response = super::super::project_change_feed_response(
        json!({
            "start_cursor": 4,
            "scanned_through_cursor": 6,
            "has_more": true,
            "changes": [
                {"cursor": 5, "conversation_id": "conversation-001"},
                {"cursor": 6, "conversation_id": "conversation-002"}
            ]
        }),
        Some(&scope),
    )
    .expect("project change response");
    assert_eq!(response["scanned_through_cursor"], 6);
    assert_eq!(response["has_more"], true);
    assert_eq!(response["changes"].as_array().unwrap().len(), 1);
    assert_eq!(response["changes"][0]["cursor"], 5);
}

fn mixed_conversation_changes() -> Value {
    json!({
        "after_cursor": 4,
        "scanned_through_cursor": 6,
        "has_more": false,
        "changes": [
            {
                "cursor": 5,
                "schema_version": 1,
                "conversation_id": "conversation-001",
                "entity_id": "prompt-001",
                "aggregate_version": 2,
                "kind": "prompt_appended",
                "created_at_ms": 20
            },
            {
                "cursor": 6,
                "schema_version": 1,
                "conversation_id": "conversation-002",
                "entity_id": "prompt-002",
                "aggregate_version": 2,
                "kind": "prompt_appended",
                "created_at_ms": 21
            }
        ]
    })
}
