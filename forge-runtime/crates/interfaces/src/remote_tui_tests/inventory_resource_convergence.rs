use std::{io::Cursor, net::TcpListener, thread};

use serde_json::{Value, json};

use super::{
    helpers::{accept_request, conversation_page, respond, serve_conversation_page, test_client},
    run_with_io,
};

fn canonical() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json"
    ))
    .expect("device inventory/resource convergence fixture")
}

#[tokio::test]
async fn remote_tui_consumes_the_canonical_inventory_resource_pair_during_sync() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let fixture = canonical();
    let inventory = fixture["inventory"].clone();
    let resource = fixture["resource_view"].clone();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));

        let (mut inventory_response, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/devices/observations/v2 "));
        assert!(body.is_empty());
        respond(&mut inventory_response, "200 OK", &inventory);

        let (mut resource_response, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        assert!(body.is_empty());
        respond(&mut resource_response, "200 OK", &resource);

        let (mut changes, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversation-changes?after_cursor=0&limit=128 "));
        respond(
            &mut changes,
            "200 OK",
            &json!({
                "after_cursor": 0,
                "scanned_through_cursor": 0,
                "has_more": false,
                "changes": []
            }),
        );
        serve_conversation_page(&listener, &conversation_page(1));

        let (mut history, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
        respond(
            &mut history,
            "200 OK",
            &json!({"conversation_id": "c-1", "prompts": [], "has_more": false}),
        );

        let (mut refreshed_inventory, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/devices/observations/v2 "));
        respond(&mut refreshed_inventory, "200 OK", &inventory);
        let (mut refreshed_resource, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        respond(&mut refreshed_resource, "200 OK", &resource);
    });

    let client = test_client(address);
    let mut reader = Cursor::new("inventory read-v2\nclient-instances resource-view\nsync\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("remote device inventory [forge.device-inventory-observation/v2]"),
        "{output}"
    );
    assert!(output.contains("device-a / runner-a"), "{output}");
    assert!(
        output.contains("revision=3 generation=2 heartbeat=4"),
        "{output}"
    );
    assert!(
        output.contains(
            "remote client-instance/resource-view [forge.client-instance-resource-view/v1]"
        ),
        "{output}"
    );
    assert!(
        output
            .matches("revision=3 generation=2 heartbeat=4")
            .count()
            >= 2,
        "{output}"
    );
}
