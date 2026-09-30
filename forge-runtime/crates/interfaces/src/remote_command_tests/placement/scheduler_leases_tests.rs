use super::*;
use crate::remote_command::scheduler_lease as scheduler_lease_contract;
use crate::remote_command::scheduler_lease_release as scheduler_lease_release_contract;
use crate::remote_command::scheduler_lease_renew as scheduler_lease_renew_contract;

#[tokio::test]
async fn scheduler_selection_lease_posts_once_with_explicit_idempotency() {
    let request = scheduler_selection_lease_request();
    let response = scheduler_selection_lease_response();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/scheduler-lease ",
        required_headers: &["idempotency-key: lease-key-00000001"],
        body_fields: json!({
            "conversation_id": "conversation-1",
            "run_id": "run-1",
            "attempt_id": "attempt-1",
            "ttl_ms": 30000
        }),
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = client
        .claim_scheduler_selection_lease(&request, "lease-key-00000001")
        .await
        .unwrap();
    scheduler_lease_contract::validate_response(&returned).unwrap();
    assert_eq!(returned["grant"]["epoch"], 1);
    server.join().unwrap();

    let mut enabled = response;
    enabled["authority"]["dispatch_performed"] = Value::Bool(true);
    assert!(scheduler_lease_contract::validate_response(&enabled).is_err());
}

#[tokio::test]
async fn scheduler_selection_lease_renewal_posts_once_with_explicit_idempotency() {
    let request = scheduler_selection_lease_renewal_request();
    let mut response = scheduler_selection_lease_response();
    response["grant"]["epoch"] = json!(2);
    response["grant"]["fencing_token"] = json!("fence-token-b");
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/scheduler-lease/renew ",
        required_headers: &["idempotency-key: renew-key-00000001"],
        body_fields: json!({
            "conversation_id": "conversation-1",
            "target_id": "runner-a",
            "epoch": 1,
            "ttl_ms": 30000
        }),
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = client
        .renew_scheduler_selection_lease(&request, "renew-key-00000001")
        .await
        .unwrap();
    scheduler_lease_renew_contract::validate_response(&returned).unwrap();
    assert_eq!(returned["grant"]["epoch"], 2);
    server.join().unwrap();
}

#[tokio::test]
async fn scheduler_selection_lease_release_posts_once_with_explicit_idempotency() {
    let request = scheduler_selection_lease_release_request();
    let response = scheduler_selection_lease_release_response();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/scheduler-lease/release ",
        required_headers: &["idempotency-key: release-key-00000001"],
        body_fields: json!({
            "conversation_id": "conversation-1",
            "target_id": "runner-a",
            "epoch": 2
        }),
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = client
        .release_scheduler_selection_lease(&request, "release-key-00000001")
        .await
        .unwrap();
    scheduler_lease_release_contract::validate_response(&returned).unwrap();
    assert_eq!(returned["released_at_ms"], 1800000000100_i64);
    assert!(!returned.to_string().contains("fence-token-b"));
    server.join().unwrap();

    let mut authority = response;
    authority["authority"]["lease_issued"] = Value::Bool(true);
    assert!(scheduler_lease_release_contract::validate_response(&authority).is_err());
}

#[tokio::test]
async fn scheduler_lease_claim_client_rejects_a_response_for_another_attempt() {
    let request = scheduler_selection_lease_request();
    let mut response = scheduler_selection_lease_response();
    response["attempt_id"] = json!("attempt-foreign");
    response["grant"]["attempt_id"] = json!("attempt-foreign");
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/scheduler-lease ",
        required_headers: &["idempotency-key: lease-binding-key-0001"],
        body_fields: json!({
            "conversation_id": "conversation-1", "run_id": "run-1", "attempt_id": "attempt-1",
            "requirements": request["requirements"].clone(), "ttl_ms": 30000
        }),
        response_status: "200 OK",
        response,
    }]);
    assert!(
        client
            .claim_scheduler_selection_lease(&request, "lease-binding-key-0001")
            .await
            .is_err()
    );
    server.join().unwrap();
}

#[tokio::test]
async fn scheduler_lease_renewal_client_rejects_a_response_for_another_epoch() {
    let request = scheduler_selection_lease_renewal_request();
    let mut response = scheduler_selection_lease_response();
    response["grant"]["epoch"] = json!(3);
    response["grant"]["fencing_token"] = json!("fence-token-c");
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/scheduler-lease/renew ",
        required_headers: &["idempotency-key: renewal-binding-key-0001"],
        body_fields: json!({
            "conversation_id": "conversation-1", "run_id": "run-1", "attempt_id": "attempt-1",
            "target_id": "runner-a", "epoch": 1, "fencing_token": "fence-token-a", "ttl_ms": 30000
        }),
        response_status: "200 OK",
        response,
    }]);
    assert!(
        client
            .renew_scheduler_selection_lease(&request, "renewal-binding-key-0001")
            .await
            .is_err()
    );
    server.join().unwrap();
}

#[tokio::test]
async fn scheduler_lease_release_client_rejects_a_response_for_another_target() {
    let request = scheduler_selection_lease_release_request();
    let mut response = scheduler_selection_lease_release_response();
    response["instance_id"] = json!("runner-foreign");
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/scheduler-lease/release ",
        required_headers: &["idempotency-key: release-binding-key-0001"],
        body_fields: json!({
            "conversation_id": "conversation-1", "run_id": "run-1", "attempt_id": "attempt-1",
            "target_id": "runner-a", "epoch": 2, "fencing_token": "fence-token-b"
        }),
        response_status: "200 OK",
        response,
    }]);
    assert!(
        client
            .release_scheduler_selection_lease(&request, "release-binding-key-0001")
            .await
            .is_err()
    );
    server.join().unwrap();
}
