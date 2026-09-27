use std::{
    io::{Cursor, ErrorKind},
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use tempfile::NamedTempFile;

use super::{
    accept_request,
    helpers::{serve_conversation_page, test_client},
    respond, run_with_io,
};

fn spawn_dispatch_plan_post_server(
    listener: TcpListener,
    expected_request: Value,
    expected_response: Value,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        serve_conversation_page(
            &listener,
            &json!({
                "conversations": [{
                    "conversation": {
                        "id": "conversation-001",
                        "scope": {"kind": "global"},
                        "title": "Shared",
                        "created_at_ms": 1,
                        "updated_at_ms": 1
                    },
                    "aggregate_version": 1
                }],
                "next_after_id": null,
                "has_more": false
            }),
        );
        let (mut stream, request_line, headers, body) = accept_request(&listener);
        assert!(request_line.starts_with(
            "POST /api/v1/conversations/conversation-001/runs/run-001/runner-dispatch-plan-preview "
        ));
        assert!(headers.contains("authorization: bearer test-token"));
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            expected_request
        );
        respond(&mut stream, "200 OK", &expected_response);
    })
}

#[tokio::test]
async fn remote_tui_posts_runner_dispatch_plan_for_the_selected_session() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let request = super::super::super::run_attempt_lease_dispatch_preflight::test_request();
    let expected_request = request["dispatch_plan"].clone();
    let response: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-runner-dispatch-plan-preview-v1.json"
    ))
    .unwrap();
    let expected_response = response.clone();
    let server = spawn_dispatch_plan_post_server(listener, expected_request, expected_response);
    let input = NamedTempFile::new().unwrap();
    std::fs::write(input.path(), serde_json::to_vec(&request).unwrap()).unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "runner-dispatch-plan-remote-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("offline Runner dispatch-plan preview"),
        "{output}"
    );
    assert!(output.contains("selected_target=none"), "{output}");
    assert!(output.contains("dispatch_performed=false"), "{output}");
    assert!(!output.contains("fence-001"), "{output}");
    assert!(!output.contains("/api/v1/devices"), "{output}");
}

fn dispatch_plan_resource_view() -> Value {
    let mut view: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json"
    ))
    .unwrap();
    view["instances"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|instance| instance["instance_id"] == json!("client-web-001"))
        .unwrap()["session_ids"] = json!(["conversation-001"]);
    view
}

fn dispatch_plan_conversation_page() -> Value {
    let mut page = super::helpers::conversation_page(1);
    page["conversations"][0]["conversation"]["id"] = json!("conversation-001");
    page
}

fn assert_no_dispatch_plan_post(listener: &TcpListener) {
    listener.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + Duration::from_millis(250);
    loop {
        match listener.accept() {
            Ok((_stream, address)) => panic!("unexpected dispatch-plan request: {address}"),
            Err(error) if error.kind() == ErrorKind::WouldBlock && Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => return,
            Err(error) => panic!("unexpected listener failure: {error}"),
        }
    }
}

fn spawn_target_guard_server(
    listener: TcpListener,
    initial_resource: Value,
    inventory: Value,
    resource: Value,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        serve_conversation_page(&listener, &dispatch_plan_conversation_page());
        let (mut initial, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        assert!(body.is_empty());
        respond(&mut initial, "200 OK", &initial_resource);
        for (path, payload) in [
            ("GET /api/v1/devices/observations/v2 ", &inventory),
            ("GET /api/v1/client-instances/resource-view ", &resource),
            ("GET /api/v1/devices/observations/v2 ", &inventory),
            ("GET /api/v1/client-instances/resource-view ", &resource),
        ] {
            let (mut stream, request, _, body) = accept_request(&listener);
            assert!(request.starts_with(path), "{request}");
            assert!(body.is_empty());
            respond(&mut stream, "200 OK", payload);
        }
        assert_no_dispatch_plan_post(&listener);
    })
}

#[tokio::test]
async fn remote_tui_blocks_selected_instance_dispatch_target_absent_from_resource() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let request = super::super::super::run_attempt_lease_dispatch_preflight::test_request();
    let initial_resource = dispatch_plan_resource_view();
    let canonical: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json"
    ))
    .unwrap();
    let server = spawn_target_guard_server(
        listener,
        initial_resource,
        canonical["inventory"].clone(),
        canonical["resource_view"].clone(),
    );
    let input = NamedTempFile::new().unwrap();
    std::fs::write(input.path(), serde_json::to_vec(&request).unwrap()).unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "client-instances resource-view\nfilter instance:client-web-001\ninventory show-converged\nrunner-dispatch-plan-remote-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "Runner dispatch-plan preview blocked by selected client-instance resource projection: target \"runner-1\" is absent from the current resource observation. No request was sent."
        ),
        "{output}"
    );
    assert!(
        !output.contains("offline Runner dispatch-plan preview ["),
        "{output}"
    );
}

#[tokio::test]
async fn remote_tui_requires_converged_resource_pair_for_selected_dispatch_plan() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let request = super::super::super::run_attempt_lease_dispatch_preflight::test_request();
    let resource = dispatch_plan_resource_view();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &dispatch_plan_conversation_page());
        let (mut stream, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        assert!(body.is_empty());
        respond(&mut stream, "200 OK", &resource);
        assert_no_dispatch_plan_post(&listener);
    });
    let input = NamedTempFile::new().unwrap();
    std::fs::write(input.path(), serde_json::to_vec(&request).unwrap()).unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "client-instances resource-view\nfilter instance:client-web-001\nrunner-dispatch-plan-remote-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "Runner dispatch-plan preview blocked by selected client-instance resource projection: open converged inventory/resource observations first. No request was sent."
        ),
        "{output}"
    );
    assert!(
        !output.contains("offline Runner dispatch-plan preview ["),
        "{output}"
    );
}
