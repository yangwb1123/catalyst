use serde_json::{Value, json};

use super::super::{RemoteCommand, execute_remote};

#[path = "attempt_projection_support.rs"]
mod support;
use support::{ExpectedRequest, fixture, request, response, spawn_mock_server};

fn get(path: &'static str, response: Value) -> ExpectedRequest {
    ExpectedRequest {
        request_prefix: path,
        response_status: "200 OK",
        response,
        body: None,
    }
}

fn post(request: &Value) -> ExpectedRequest {
    ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/runner-attempt-boundary/preview ",
        response_status: "200 OK",
        response: response(request),
        body: Some(request.clone()),
    }
}

fn observations() -> Vec<ExpectedRequest> {
    let pair = fixture("forge-device-inventory-resource-convergence-v1");
    vec![
        get(
            "GET /api/v1/client-instances/session-view ",
            fixture("forge-client-instance-session-view-v1"),
        ),
        get(
            "GET /api/v1/client-instances/resource-view ",
            fixture("forge-client-instance-resource-view-v1"),
        ),
        get(
            "GET /api/v1/devices/observations/v2 ",
            pair["inventory"].clone(),
        ),
        get(
            "GET /api/v1/client-instances/resource-view ",
            pair["resource_view"].clone(),
        ),
    ]
}

async fn preview(
    request: &Value,
    replies: Vec<ExpectedRequest>,
    instance_id: Option<&str>,
    local_view: Option<&Value>,
) -> Result<Value, String> {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("request.json");
    std::fs::write(&input, serde_json::to_vec(request).unwrap()).unwrap();
    let instance_view = local_view.map(|view| {
        let file = directory.path().join("view.json");
        std::fs::write(&file, serde_json::to_vec(view).unwrap()).unwrap();
        file.to_string_lossy().into_owned()
    });
    let command = RemoteCommand::RunnerAttemptBoundaryPreview {
        input: input.to_string_lossy().into_owned(),
        instance_id: instance_id.map(str::to_owned),
        instance_view,
    };
    let (client, server) = spawn_mock_server(replies);
    let result = execute_remote(&client, &command, None, None)
        .await
        .map_err(|error| error.to_string());
    server.join().unwrap();
    result
}

#[tokio::test]
async fn unfiltered_attempt_preview_keeps_one_metadata_post() {
    let request = request();
    let returned = preview(&request, vec![post(&request)], None, None)
        .await
        .unwrap();
    assert_eq!(returned, response(&request));
    assert!(!returned.to_string().contains("token-a"));
    assert!(!returned.to_string().contains("forge-task"));
}

#[tokio::test]
async fn visible_attempt_preview_refreshes_both_pairs_and_accepts_device_or_runner_target() {
    for target in ["runner-a", "device-a"] {
        let mut request = request();
        request["command"]["lease_proof"]["target_id"] = json!(target);
        request["transport"]["path"] = json!(format!("/api/v1/runners/{target}/dispatch"));
        let mut replies = observations();
        replies.push(post(&request));
        let returned = preview(&request, replies, Some("client-web-001"), None)
            .await
            .unwrap();
        assert_eq!(returned["target_id"], target);
        assert_eq!(returned["authority"]["attempt_persisted"], false);
    }
}

#[tokio::test]
async fn foreign_attempt_target_stops_after_four_observations_without_post() {
    let mut request = request();
    request["command"]["lease_proof"]["target_id"] = json!("foreign-runner");
    request["transport"]["path"] = json!("/api/v1/runners/foreign-runner/dispatch");
    let error = preview(&request, observations(), Some("client-web-001"), None)
        .await
        .unwrap_err();
    assert!(error.contains("rejected target"), "{error}");
    assert!(error.contains("no Runner Attempt boundary request was sent"));
}

#[tokio::test]
async fn hidden_attempt_conversation_stops_after_session_resource_pair() {
    let mut replies = observations();
    replies.truncate(2);
    let error = preview(&request(), replies, Some("client-mobile-001"), None)
        .await
        .unwrap_err();
    assert!(error.contains("rejected conversation"), "{error}");
}

#[tokio::test]
async fn drifted_session_resource_pair_stops_before_inventory_or_post() {
    let mut replies = observations();
    replies.truncate(2);
    replies[1].response["instances"][0]["status"] = json!("idle");
    let error = preview(&request(), replies, Some("client-web-001"), None)
        .await
        .unwrap_err();
    assert!(error.contains("did not converge"), "{error}");
}

#[tokio::test]
async fn drifted_inventory_resource_pair_stops_before_attempt_post() {
    let mut replies = observations();
    replies[3].response["devices"][0]["heartbeat_sequence"] = json!(99);
    let error = preview(&request(), replies, Some("client-web-001"), None)
        .await
        .unwrap_err();
    assert!(error.contains("did not converge"), "{error}");
}

#[tokio::test]
async fn revoked_conversation_in_refreshed_resource_stops_before_attempt_post() {
    let mut replies = observations();
    for instance in replies[3].response["instances"].as_array_mut().unwrap() {
        if instance["instance_id"] == "client-web-001" {
            instance["session_ids"] = json!([]);
        }
    }
    let error = preview(&request(), replies, Some("client-web-001"), None)
        .await
        .unwrap_err();
    assert!(
        error.contains("refreshed client-instance filter rejected conversation"),
        "{error}"
    );
}

#[tokio::test]
async fn malformed_or_unauthorized_resource_blocks_attempt_post() {
    for unauthorized in [false, true] {
        let mut replies = observations();
        replies.truncate(2);
        if unauthorized {
            replies[1].response_status = "403 Forbidden";
            replies[1].response = json!({"code":"insufficient_scope"});
        } else {
            replies[1].response["devices"] = json!(false);
        }
        let error = preview(&request(), replies, Some("client-web-001"), None)
            .await
            .unwrap_err();
        let expected = if unauthorized {
            "HTTP 403"
        } else {
            "invalid client-instance resource view"
        };
        assert!(error.contains(expected), "{error}");
    }
}

#[tokio::test]
async fn local_resource_or_converged_view_needs_no_observation_requests() {
    let request = request();
    for name in [
        "forge-client-instance-resource-view-v1",
        "forge-client-instance-session-resource-convergence-v1",
    ] {
        let returned = preview(
            &request,
            vec![post(&request)],
            Some("client-web-001"),
            Some(&fixture(name)),
        )
        .await
        .unwrap();
        assert_eq!(returned, response(&request));
    }
}

#[tokio::test]
async fn local_session_only_or_foreign_target_view_is_request_free() {
    let session = fixture("forge-client-instance-session-view-v1");
    let error = preview(&request(), vec![], Some("client-web-001"), Some(&session))
        .await
        .unwrap_err();
    assert!(
        error.contains("requires a resource or converged view"),
        "{error}"
    );
    let mut request = request();
    request["command"]["lease_proof"]["target_id"] = json!("foreign-runner");
    request["transport"]["path"] = json!("/api/v1/runners/foreign-runner/dispatch");
    let resource = fixture("forge-client-instance-resource-view-v1");
    let error = preview(&request, vec![], Some("client-web-001"), Some(&resource))
        .await
        .unwrap_err();
    assert!(error.contains("rejected target"), "{error}");
}
