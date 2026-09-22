use super::*;

fn request() -> Value {
    json!({
        "device_id": "device-1",
        "action": "issue",
        "approval_state": "approved",
        "credential_id": "credential-1",
        "key_id": "key-1",
        "public_key_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "key_generation": 1,
        "issued_at_ms": 100,
        "expires_at_ms": 1100,
        "next_credential_id": "",
        "next_key_id": "",
        "next_public_key_sha256": "",
        "observed_at_ms": 100,
        "expected_device_revision": 7
    })
}

fn response() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-device-credential-candidate-v1.json"
    ))
    .expect("credential candidate fixture")
}

#[tokio::test]
async fn credential_candidate_posts_exact_owner_scoped_request_once() {
    let request = request();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-enrollment-heartbeat/credential-candidate ",
        required_headers: &["content-type: application/json"],
        body_fields: request.clone(),
        response_status: "200 OK",
        response: response(),
    }]);

    let returned = client
        .preview_device_credential_candidate(&request)
        .await
        .expect("credential candidate response");
    assert_eq!(returned["device_id"], "device-1");
    assert_eq!(returned["revision"], 7);
    server.join().expect("candidate request server");
}

#[tokio::test]
async fn credential_candidate_http_errors_are_not_replayed() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().expect("address");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("one candidate request");
        let _ = capture_request(&mut stream);
        write_json_response(
            &mut stream,
            "401 Unauthorized",
            &json!({"error":"invalid_token"}),
        );
    });

    let error = test_remote_client(address)
        .preview_device_credential_candidate(&request())
        .await
        .expect_err("401 must fail");
    assert!(error.to_string().contains("401"), "{error}");
    server.join().expect("candidate 401 server");
}

#[test]
fn credential_candidate_request_and_response_binding_reject_drift() {
    let request = request();
    crate::device_credential_candidate_command::validate_remote_request(&request)
        .expect("valid request");

    let mut unknown = request.clone();
    unknown["credential_material"] = json!("secret");
    assert!(crate::device_credential_candidate_command::validate_remote_request(&unknown).is_err());

    let mut drifted = response();
    drifted["device_id"] = json!("device-2");
    assert!(
        crate::device_credential_candidate_command::validate_remote_response(&drifted, &request)
            .is_err()
    );

    let mut authority = response();
    authority["authority"]["persisted"] = json!(true);
    assert!(
        crate::device_credential_candidate_command::validate_remote_response(&authority, &request)
            .is_err()
    );
}
