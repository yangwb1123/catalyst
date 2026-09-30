use super::*;

#[tokio::test]
async fn remote_tui_can_show_bounded_session_device_observation_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-session-device-observation-v1.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "inventory session-observation --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("offline device resource summary [forge.device-resource-summary/v1]"));
    assert!(output.contains("owner=user-1 conversation=conversation-001 run=run-001"));
    assert!(output.contains("eligible_devices=2 eligible_instances=2"));
    assert!(output.contains("authority: identity_verified=false"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_session_observation_preview_uses_the_authenticated_session_route() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (request, response) = session_observation_request_and_response();
    let expected_request = request.clone();
    let server =
        thread::spawn(move || serve_session_observation(listener, response, expected_request));
    let input = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(input.path(), serde_json::to_vec(&request).unwrap()).unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "session-observation-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert_session_observation_output(&output);
}

fn serve_session_observation(listener: TcpListener, response: Value, expected_request: Value) {
    serve_conversation_page(&listener, &conversation_page(1));
    let (mut stream, request_line, headers, body) =
        super::super::helpers::accept_request(&listener);
    assert!(
        request_line
            .starts_with("POST /api/v1/conversations/c-1/runs/run-1/device-observation/preview ")
    );
    assert!(headers.contains("authorization: bearer test-token"));
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap(),
        expected_request
    );
    super::super::helpers::respond(&mut stream, "200 OK", &response);
}

fn assert_session_observation_output(output: &str) {
    assert!(
        output.contains("offline session device observation"),
        "{output}"
    );
    assert!(
        output.contains("owner=user-1 conversation=c-1 run=run-1"),
        "{output}"
    );
    assert!(
        output.contains("eligible_devices=2 eligible_instances=2"),
        "{output}"
    );
    assert!(
        output.contains("candidate-a/runner-a: matches resources=cpu:8 memory:16384 storage:8192 gpu:true gpu_memory:4096"),
        "{output}"
    );
    assert!(
        output.contains("candidate-b/runner-b: excluded"),
        "{output}"
    );
    assert!(
        output.contains("authority: identity_verified=false"),
        "{output}"
    );
}

fn session_observation_request_and_response() -> (Value, Value) {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-session-device-observation-v1.json");
    let mut response: Value = serde_json::from_slice(&std::fs::read(&fixture).unwrap()).unwrap();
    let request = session_observation_request(&response);
    for pointer in [
        "/conversation_id",
        "/run_id",
        "/placement_observation/conversation_id",
        "/placement_observation/run_id",
        "/resource_summary/conversation_id",
        "/resource_summary/run_id",
    ] {
        *response.pointer_mut(pointer).unwrap() = Value::String(if pointer.ends_with("run_id") {
            "run-1".into()
        } else {
            "c-1".into()
        });
    }
    (request, response)
}

fn session_observation_request(response: &Value) -> Value {
    let candidates = response["inventory"]["devices"].clone();
    let devices = candidates
        .as_array()
        .unwrap()
        .iter()
        .map(|candidate| candidate["device"].clone())
        .collect::<Vec<_>>();
    json!({
        "owner": response["owner"].clone(),
        "conversation_id": "c-1",
        "run_id": "run-1",
        "placement": {
            "schema_version": "forge.device-placement-dry-run/v1",
            "evaluated_at_ms": response["evaluated_at_ms"].clone(),
            "owner": response["owner"].clone(),
            "max_snapshot_age_ms": 60000,
            "requirements": {
                "os": "linux",
                "architecture": "amd64",
                "min_cpu_cores": 4,
                "min_memory_bytes": 8192,
                "min_storage_bytes": 4096,
                "runtime": "oci",
                "gpu": {"required": false, "min_memory_bytes": 0, "runtime": ""},
                "data_residency_zones": ["us-west"],
                "minimum_trust_zone": "standard",
                "sandbox_floor": "container",
                "concurrency_slots": 1
            },
            "devices": devices,
        },
        "candidates": candidates,
    })
}
