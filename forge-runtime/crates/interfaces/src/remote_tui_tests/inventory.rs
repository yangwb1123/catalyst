use std::{io::Cursor, net::TcpListener, thread};

use serde_json::{Value, json};
use tempfile::NamedTempFile;

use super::{
    helpers::{accept_request, conversation_page, respond, serve_conversation_page, test_client},
    run_with_io,
};

#[tokio::test]
async fn remote_tui_can_show_bounded_offline_inventory_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-device-inventory-observation-v1.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "inventory show --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("offline device inventory [forge.device-inventory-observation/v1]"));
    assert!(output.contains("authority: identity_verified=false"));
    assert!(output.contains("device-a / runner-a"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_can_read_owner_bound_inventory_candidate() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-device-inventory-observation-v1.json"
    ))
    .unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut stream, request, headers, body) = super::helpers::accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/devices "), "{request}");
        assert!(headers.contains("authorization: bearer test-token"));
        assert!(body.is_empty());
        super::helpers::respond(&mut stream, "200 OK", &fixture);
    });

    let client = test_client(address);
    let mut reader = Cursor::new("inventory read\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("remote device inventory [forge.device-inventory-observation/v1]"));
    assert!(output.contains("device-a / runner-a"));
    assert!(output.contains("authority: identity_verified=false"));
}

#[tokio::test]
async fn remote_tui_can_read_owner_bound_lossless_v2_inventory_candidate() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-device-inventory-observation-v2.json"
    ))
    .unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut stream, request, headers, body) = super::helpers::accept_request(&listener);
        assert!(
            request.starts_with("GET /api/v1/devices/observations/v2 "),
            "{request}"
        );
        assert!(headers.contains("authorization: bearer test-token"));
        assert!(body.is_empty());
        super::helpers::respond(&mut stream, "200 OK", &fixture);
    });

    let client = test_client(address);
    let mut reader = Cursor::new("inventory read-v2\nquit\n");
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
        output.contains("revision=1 generation=1 heartbeat=1"),
        "{output}"
    );
    assert!(output.contains("reservation=reserved"), "{output}");
    assert!(output.contains("gpus=2"), "{output}");
    assert!(
        output.contains("authority: identity_verified=false"),
        "{output}"
    );
}

#[tokio::test]
async fn remote_tui_show_converged_commits_inventory_and_resource_pair() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let pair: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json"
    ))
    .unwrap();
    let inventory = pair["inventory"].clone();
    let resource = pair["resource_view"].clone();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));

        let (mut inventory_response, request, headers, body) = accept_request(&listener);
        assert!(
            request.starts_with("GET /api/v1/devices/observations/v2 "),
            "{request}"
        );
        assert!(headers.contains("authorization: bearer test-token"));
        assert!(body.is_empty());
        respond(&mut inventory_response, "200 OK", &inventory);

        let (mut resource_response, request, headers, body) = accept_request(&listener);
        assert!(
            request.starts_with("GET /api/v1/client-instances/resource-view "),
            "{request}"
        );
        assert!(headers.contains("authorization: bearer test-token"));
        assert!(body.is_empty());
        respond(&mut resource_response, "200 OK", &resource);
    });

    let client = test_client(address);
    let mut reader = Cursor::new("inventory show-converged\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "remote inventory/resource-convergence [forge.device-inventory-resource-convergence/v1] converged=true read_only=true"
        ),
        "{output}"
    );
    assert!(
        output.contains("remote device inventory [forge.device-inventory-observation/v2]"),
        "{output}"
    );
    assert!(
        output.contains(
            "remote client-instance/resource-view [forge.client-instance-resource-view/v1]"
        ),
        "{output}"
    );
    assert!(
        output.contains("Inventory/resource observations converged; both snapshots committed."),
        "{output}"
    );
}

#[tokio::test]
async fn remote_tui_inventory_convergence_reconciles_revoked_instance_selection() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let pair: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json"
    ))
    .unwrap();
    let inventory = pair["inventory"].clone();
    let resource = pair["resource_view"].clone();
    let mut revoked_resource = resource.clone();
    for instance in revoked_resource["instances"].as_array_mut().unwrap() {
        if instance["instance_id"] == "client-web-001" {
            instance["session_ids"] = json!([]);
        }
    }
    let page = json!({
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
    });
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &page);

        let (mut initial_inventory, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/devices/observations/v2 "));
        assert!(body.is_empty());
        respond(&mut initial_inventory, "200 OK", &inventory);

        let (mut initial_resource, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        assert!(body.is_empty());
        respond(&mut initial_resource, "200 OK", &resource);

        let (mut history, request, _, body) = accept_request(&listener);
        assert!(
            request.starts_with("GET /api/v1/conversations/conversation-001/prompts?limit=128 ")
        );
        assert!(body.is_empty());
        respond(
            &mut history,
            "200 OK",
            &json!({
                "conversation_id": "conversation-001",
                "prompts": [{
                    "id": "prompt-before-revocation",
                    "conversation_id": "conversation-001",
                    "role": "user",
                    "content": "private prompt before refresh",
                    "created_at_ms": 10
                }],
                "has_more": false
            }),
        );

        let (mut refreshed_inventory, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/devices/observations/v2 "));
        assert!(body.is_empty());
        respond(&mut refreshed_inventory, "200 OK", &inventory);

        let (mut refreshed_resource, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        assert!(body.is_empty());
        respond(&mut refreshed_resource, "200 OK", &revoked_resource);
    });

    let client = test_client(address);
    let mut reader = Cursor::new(
        "inventory show-converged\ninstance client-web-001\nopen conversation-001\ninventory show-converged\nquit\n",
    );
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("Opened session \"conversation-001\""),
        "{output}"
    );
    assert!(output.contains("private prompt before refresh"), "{output}");
    let last_render = output
        .rsplit("Forge shared sessions")
        .next()
        .unwrap_or_default();
    assert!(
        last_render.contains("Client-instance filter: \"client-web-001\""),
        "{last_render}"
    );
    assert!(
        last_render.contains("No sessions match this client-instance filter"),
        "{last_render}"
    );
    assert!(!last_render.contains("Prompt history for"), "{last_render}");
    assert!(
        !last_render.contains("private prompt before refresh"),
        "{last_render}"
    );
}

#[tokio::test]
async fn remote_tui_show_converged_rejects_inventory_resource_drift_without_partial_commit() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let pair: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json"
    ))
    .unwrap();
    let inventory = pair["inventory"].clone();
    let mut resource = pair["resource_view"].clone();
    resource["devices"][0]["heartbeat_sequence"] = json!(99);
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
    });

    let client = test_client(address);
    let mut reader = Cursor::new("inventory show-converged\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "Remote inventory/resource convergence request failed: Forge API inventory/resource observations did not converge"
        ),
        "{output}"
    );
    assert!(
        output.contains(
            "Previous inventory/resource snapshots were retained; no mixed pair was committed."
        ),
        "{output}"
    );
    assert!(!output.contains("remote device inventory ["), "{output}");
    assert!(
        !output.contains("remote client-instance/resource-view ["),
        "{output}"
    );
}

#[tokio::test]
async fn remote_tui_sync_refreshes_an_explicitly_opened_inventory_view() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-device-inventory-observation-v2.json"
    ))
    .unwrap();
    let mut refreshed_fixture = fixture.clone();
    refreshed_fixture["evaluated_at_ms"] = json!(300_000);
    refreshed_fixture["devices"][0]["revision"] = json!(2);
    refreshed_fixture["devices"][0]["heartbeat_sequence"] = json!(2);
    refreshed_fixture["devices"][0]["device"]["available_cpu_cores"] = json!(3);
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));

        let (mut initial_inventory, request, headers, body) =
            super::helpers::accept_request(&listener);
        assert!(
            request.starts_with("GET /api/v1/devices/observations/v2 "),
            "{request}"
        );
        assert!(headers.contains("authorization: bearer test-token"));
        assert!(body.is_empty());
        super::helpers::respond(&mut initial_inventory, "200 OK", &fixture);

        let (mut changes, request, _, _) = super::helpers::accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversation-changes?after_cursor=0&limit=128 "));
        super::helpers::respond(
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

        let (mut history, request, _, _) = super::helpers::accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
        super::helpers::respond(
            &mut history,
            "200 OK",
            &json!({"conversation_id": "c-1", "prompts": [], "has_more": false}),
        );

        let (mut refreshed_inventory, request, headers, body) =
            super::helpers::accept_request(&listener);
        assert!(
            request.starts_with("GET /api/v1/devices/observations/v2 "),
            "{request}"
        );
        assert!(headers.contains("authorization: bearer test-token"));
        assert!(body.is_empty());
        super::helpers::respond(&mut refreshed_inventory, "200 OK", &refreshed_fixture);
    });

    let client = test_client(address);
    let mut reader = Cursor::new("inventory read-v2\nsync\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert_eq!(
        output
            .matches("remote device inventory [forge.device-inventory-observation/v2]")
            .count(),
        2,
        "{output}"
    );
    assert!(output.contains("at 200000 devices=2"), "{output}");
    assert!(output.contains("at 300000 devices=2"), "{output}");
    assert!(
        output.contains(
            "device-a / runner-a: revision=2 generation=1 heartbeat=2 reservation=reserved cpu=3"
        ),
        "{output}"
    );
    assert!(output.contains("Selected inventory observation refreshed."));
    assert!(output.contains("Synced 0 owner-visible changes through cursor 0"));
}

#[tokio::test]
async fn remote_tui_retains_previous_pair_until_inventory_resource_refresh_converges() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let pair: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json"
    ))
    .unwrap();
    let inventory = pair["inventory"].clone();
    let resource = pair["resource_view"].clone();
    let mut inventory_revision_two = inventory.clone();
    inventory_revision_two["devices"][0]["revision"] = json!(2);
    inventory_revision_two["devices"][0]["heartbeat_sequence"] = json!(2);
    let mut resource_revision_three = resource.clone();
    resource_revision_three["devices"][0]["revision"] = json!(3);
    resource_revision_three["devices"][0]["heartbeat_sequence"] = json!(3);
    let mut inventory_revision_three = inventory_revision_two.clone();
    inventory_revision_three["devices"][0]["revision"] = json!(3);
    inventory_revision_three["devices"][0]["heartbeat_sequence"] = json!(3);
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut initial_inventory, request, _, _) = super::helpers::accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/devices/observations/v2 "));
        super::helpers::respond(&mut initial_inventory, "200 OK", &inventory);
        let (mut initial_resource, request, _, _) = super::helpers::accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        super::helpers::respond(&mut initial_resource, "200 OK", &resource);

        for (next_inventory, next_resource) in [
            (
                inventory_revision_two.clone(),
                resource_revision_three.clone(),
            ),
            (inventory_revision_three, resource_revision_three),
        ] {
            let (mut changes, request, _, _) = super::helpers::accept_request(&listener);
            assert!(
                request.starts_with("GET /api/v1/conversation-changes?after_cursor=0&limit=128 ")
            );
            super::helpers::respond(
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
            let (mut history, request, _, _) = super::helpers::accept_request(&listener);
            assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
            super::helpers::respond(
                &mut history,
                "200 OK",
                &json!({"conversation_id": "c-1", "prompts": [], "has_more": false}),
            );
            let (mut refreshed_inventory, request, _, _) =
                super::helpers::accept_request(&listener);
            assert!(request.starts_with("GET /api/v1/devices/observations/v2 "));
            super::helpers::respond(&mut refreshed_inventory, "200 OK", &next_inventory);
            let (mut refreshed_resource, request, _, _) = super::helpers::accept_request(&listener);
            assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
            super::helpers::respond(&mut refreshed_resource, "200 OK", &next_resource);
        }
    });

    let client = test_client(address);
    let mut reader =
        Cursor::new("inventory read-v2\nclient-instances resource-view\nsync\nsync\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "Inventory/resource observations did not converge; previous snapshots were retained"
        ),
        "{output}"
    );
    assert_eq!(
        output
            .matches("Synced 0 owner-visible changes through cursor 0")
            .count(),
        1,
        "{output}"
    );
}

#[tokio::test]
async fn remote_tui_sync_refreshes_an_explicitly_opened_v1_inventory_view() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-device-inventory-observation-v1.json"
    ))
    .unwrap();
    let mut refreshed_fixture = fixture.clone();
    refreshed_fixture["evaluated_at_ms"] = json!(300_000);
    refreshed_fixture["devices"][0]["device"]["liveness"] = json!("offline");
    refreshed_fixture["devices"][0]["device"]["available_cpu_cores"] = json!(2);
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));

        let (mut initial_inventory, request, headers, body) =
            super::helpers::accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/devices "), "{request}");
        assert!(headers.contains("authorization: bearer test-token"));
        assert!(body.is_empty());
        super::helpers::respond(&mut initial_inventory, "200 OK", &fixture);

        let (mut changes, request, _, _) = super::helpers::accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversation-changes?after_cursor=0&limit=128 "));
        super::helpers::respond(
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

        let (mut history, request, _, _) = super::helpers::accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
        super::helpers::respond(
            &mut history,
            "200 OK",
            &json!({"conversation_id": "c-1", "prompts": [], "has_more": false}),
        );

        let (mut refreshed_inventory, request, headers, body) =
            super::helpers::accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/devices "), "{request}");
        assert!(headers.contains("authorization: bearer test-token"));
        assert!(body.is_empty());
        super::helpers::respond(&mut refreshed_inventory, "200 OK", &refreshed_fixture);
    });

    let client = test_client(address);
    let mut reader = Cursor::new("inventory read\nsync\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert_eq!(
        output
            .matches("remote device inventory [forge.device-inventory-observation/v1]")
            .count(),
        2,
        "{output}"
    );
    assert!(
        output
            .contains("remote device inventory [forge.device-inventory-observation/v1] at 200000"),
        "{output}"
    );
    assert!(
        output
            .contains("remote device inventory [forge.device-inventory-observation/v1] at 300000"),
        "{output}"
    );
    assert!(
        output
            .contains("device-a / runner-a: approval=approved cordon=clear liveness=offline cpu=2"),
        "{output}"
    );
    assert!(output.contains("Selected inventory observation refreshed."));
    assert!(output.contains("Synced 0 owner-visible changes through cursor 0"));
}

#[tokio::test]
async fn remote_tui_can_show_bounded_offline_inventory_status_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-device-inventory-status-contract-v1.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "inventory status --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "offline device inventory status [forge.device-inventory-status-contract/v1]"
        )
    );
    assert!(output.contains("approved_online_is_declared_eligible: status=online"));
    assert!(output.contains("future_snapshot_rejected: error=snapshot_from_future"));
    assert!(output.contains("authority: identity_verified=false"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_can_preview_placement_batch_from_a_file_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../../docs/contracts/fixtures/forge-device-inventory-placement-batch-evaluation-v1.json",
    );
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "inventory placement-batch-evaluation --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("offline placement batch evaluation"),
        "{output}"
    );
    assert!(output.contains("decisions=6"), "{output}");
    assert!(output.contains("selected=none"), "{output}");
    assert!(
        output.contains("authority: identity_verified=false"),
        "{output}"
    );
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_can_preview_persisted_placement_from_a_file_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../../docs/contracts/fixtures/forge-device-inventory-placement-evaluation-v1.json",
    );
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "inventory placement-evaluation --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("offline placement evaluation"), "{output}");
    assert!(output.contains("source=online"), "{output}");
    assert!(output.contains("device=device-a"), "{output}");
    assert!(output.contains("instance=runner-a"), "{output}");
    assert!(output.contains("matches=false"), "{output}");
    assert!(
        output.contains("authority: placement_evaluated=false"),
        "{output}"
    );
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_can_preview_v2_placement_from_a_file_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../../docs/contracts/fixtures/forge-device-inventory-placement-evaluation-v2.json",
    );
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "inventory placement-evaluation-v2 --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("offline placement evaluation [offline_static_only]"),
        "{output}"
    );
    assert!(output.contains("reservation=reserved"), "{output}");
    assert!(output.contains("gpus=2"), "{output}");
    assert!(output.contains("selected=none"), "{output}");
    assert!(
        output.contains("authority: identity_verified=false"),
        "{output}"
    );
    assert!(!output.contains("/api/v1/devices"), "{output}");
}

#[tokio::test]
async fn remote_tui_can_preview_inventory_persistence_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-device-inventory-persistence-v1.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "inventory persistence-preview --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "offline device inventory persistence preview [forge.device-inventory-persistence/v1]"
        ),
        "{output}"
    );
    assert!(
        output.contains("project_online: accepted=true revision=3"),
        "{output}"
    );
    assert!(
        output.contains("revision_conflict: accepted=false error=revision_conflict"),
        "{output}"
    );
    assert!(
        output.contains("authority: identity_verified=false"),
        "{output}"
    );
    assert!(!output.contains("/api/v1/devices"), "{output}");
}

#[tokio::test]
async fn remote_tui_can_show_bounded_offline_inventory_snapshot_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-device-inventory-snapshot-canonical-v1.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "inventory snapshot-canonical --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("offline device inventory snapshot canonical"));
    assert!(output.contains("sorts_by_device_then_instance_without_mutating_input"));
    assert!(output.contains("owner_mismatch"));
    assert!(output.contains("authority: identity_verified=false"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_can_show_bounded_offline_resource_summary_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-device-resource-summary-v1.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "inventory resource-summary --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("offline device resource summary [forge.device-resource-summary/v1]"));
    assert!(output.contains(
        "resources: devices=9 runner_instances=9 cpu=66 memory=135168 storage=67584 gpus=1 gpu_memory=4096 eligible_devices=2 eligible_instances=2"
    ));
    assert!(output.contains("authority: identity_verified=false"));
    assert!(!output.contains("/api/v1/devices"));
}

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
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-session-device-observation-v1.json");
    let mut response: Value = serde_json::from_slice(&std::fs::read(&fixture).unwrap()).unwrap();
    let candidates = response["inventory"]["devices"].clone();
    let devices = candidates
        .as_array()
        .unwrap()
        .iter()
        .map(|candidate| candidate["device"].clone())
        .collect::<Vec<_>>();
    let request = json!({
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
    });
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
    let expected_request = request.clone();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut stream, request_line, headers, body) = super::helpers::accept_request(&listener);
        assert!(
            request_line.starts_with(
                "POST /api/v1/conversations/c-1/runs/run-1/device-observation/preview "
            )
        );
        assert!(headers.contains("authorization: bearer test-token"));
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            expected_request
        );
        super::helpers::respond(&mut stream, "200 OK", &response);
    });
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

#[tokio::test]
async fn remote_tui_can_preview_a_run_intent_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });

    let fixtures =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/contracts/fixtures");
    let run = fixtures.join("forge-run-intent-observation-v1.json");
    let placement = fixtures.join("forge-session-placement-observation-v1.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "run-intent-preview --input {} --placement-input {}\nquit\n",
        run.display(),
        placement.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("offline Run-intent preview [forge.run-intent-observation/v1]"));
    assert!(output.contains("decisions=9 eligible=2"));
    assert!(output.contains("selected_device=none selected_instance=none"));
    assert!(output.contains("execution_authorized=false dispatch_performed=false"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_can_preview_a_runner_terminal_receipt_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-runner-command-terminal-receipt-v1.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "runner-receipt-preview --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "offline Runner terminal receipt preview [forge.runner-command-terminal-receipt/v1]"
        ),
        "{output}"
    );
    assert!(output.contains("command=command-1 attempt=attempt-1 target=runner-1"));
    assert!(output.contains("disposition=completed"));
    assert!(output.contains("receipt_valid=true uncertain=false"));
    assert!(output.contains("audit_published=false"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_can_preview_runner_lease_fencing_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-runner-lease-fencing-v1.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "runner-lease-fencing-preview --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("offline Runner lease fencing preview [forge.runner-lease-fencing/v1]"),
        "{output}"
    );
    assert!(output.contains("attempt=attempt-1 target=runner-1 epoch=1"));
    assert!(output.contains("terminal_replay: accepted=true replayed=true"));
    assert!(output.contains("terminal_uncertain: accepted=true replayed=false uncertain=true"));
    assert!(output.contains("authority: device_identity_verified=false"));
    assert!(!output.contains("fence-1"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_can_preview_execution_lease_checkpoint_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-execution-lease-checkpoint-v1.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "execution-lease-checkpoint-preview --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "offline execution lease checkpoint preview [forge.execution-lease-checkpoint/v1]"
        ),
        "{output}"
    );
    assert!(output.contains(
        "uncertain_receipt_remains_terminal: accepted=true terminal=true uncertain=true"
    ));
    assert!(output.contains("foreign_proof_rejected: accepted=false error=invalid_checkpoint"));
    assert!(output.contains("authority: lease_issued=false"));
    assert!(!output.contains("fence-1"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_can_preview_client_instance_session_view_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "client-session-view-preview --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "offline client-instance/session-view [forge.client-instance-session-view/v1]"
        ),
        "{output}"
    );
    assert!(output.contains(
        "instance client-cli-001: client_kind=cli status=active observed_at_ms=200500 sessions=conversation-001,conversation-002"
    ));
    assert!(output.contains(
        "instance client-web-001: client_kind=web status=idle observed_at_ms=200500 sessions=conversation-001"
    ));
    assert!(output.contains("read_only=true"));
    assert!(output.contains("prompt_write_authorized=false"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_can_preview_client_instance_resource_view_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "client-instance-resource-view-preview --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "offline client-instance/resource-view [forge.client-instance-resource-view/v1]"
        ),
        "{output}"
    );
    assert!(output.contains("instance client-cli-001: client_kind=cli"));
    assert!(
        output.contains("device device-a: runner=runner-a revision=1 generation=1 heartbeat=1")
    );
    assert!(output.contains("read_only=true device_attributes_unverified=true"));
    assert!(output.contains("prompt_write_authorized=false"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_can_preview_a_session_runner_receipt_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-session-runner-receipt-observation-v1.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "session-runner-receipt-offline-preview --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "offline session Runner terminal receipt observation [forge.session-runner-receipt-observation/v1]"
        ),
        "{output}"
    );
    assert!(
        output.contains("owner=user-1 conversation=conversation-001 prompt=prompt-001 run=run-001")
    );
    assert!(output.contains("receipt_command=command-001 attempt=attempt-001 target=runner-1"));
    assert!(output.contains("selected_target=none"));
    assert!(output.contains("receipt_persisted=false execution_authorized=false"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_can_reduce_session_runner_receipt_history_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-session-runner-receipt-history-v1.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "session-runner-receipt-history-offline-preview --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "offline session Runner receipt history [forge.session-runner-receipt-history/v1]"
        ),
        "{output}"
    );
    assert!(output.contains("attempt_count=2"));
    assert!(output.contains(
        "attempt[1]: command=command-001 attempt=attempt-001 target=runner-1 disposition=failed"
    ));
    assert!(output.contains(
        "attempt[2]: command=command-002 attempt=attempt-002 target=runner-2 disposition=uncertain"
    ));
    assert!(output.contains("follow_up=reconciliation_manual"));
    assert!(output.contains("automatic_retry=false"));
    assert!(output.contains("selected_target=none"));
    assert!(output.contains("receipt_persisted=false execution_authorized=false"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_rejects_session_runner_receipt_history_summary_drift_locally() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-session-runner-receipt-history-v1.json");
    let mut malformed: Value = serde_json::from_slice(&std::fs::read(fixture).unwrap()).unwrap();
    malformed["attempt_count"] = json!(3);
    let input = NamedTempFile::new().unwrap();
    std::fs::write(input.path(), serde_json::to_vec(&malformed).unwrap()).unwrap();

    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "session-runner-receipt-history-offline-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("Session Runner receipt history preview failed:"),
        "{output}"
    );
    assert!(!output.contains("attempt_count=3"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_session_runner_receipt_preview_uses_the_authenticated_session_route() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-session-runner-receipt-observation-v1.json");
    let mut request: Value = serde_json::from_slice(&std::fs::read(&fixture).unwrap()).unwrap();
    request["conversation_id"] = Value::String("c-1".into());
    request["run_id"] = Value::String("run-1".into());
    let expected_request = request.clone();
    let response = request.clone();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut stream, request_line, headers, body) = accept_request(&listener);
        assert!(request_line.starts_with(
            "POST /api/v1/conversations/c-1/runs/run-1/runner-receipt-observation/preview "
        ));
        assert!(headers.contains("authorization: bearer test-token"));
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            expected_request
        );
        respond(&mut stream, "200 OK", &response);
    });
    let input = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(input.path(), serde_json::to_vec(&request).unwrap()).unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "session-runner-receipt-preview --input {}\nquit\n",
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
            "authenticated session Runner terminal receipt observation [forge.session-runner-receipt-observation/v1]"
        ),
        "{output}"
    );
    assert!(output.contains("owner=user-1 conversation=c-1 prompt=prompt-001 run=run-1"));
    assert!(output.contains("receipt_command=command-001 attempt=attempt-001 target=runner-1"));
    assert!(output.contains("selected_target=none"));
    assert!(output.contains("receipt_persisted=false execution_authorized=false"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_run_execution_evidence_preview_uses_the_authenticated_session_route() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let receipt_fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-session-runner-receipt-observation-v1.json");
    let receipt: Value = serde_json::from_slice(&std::fs::read(&receipt_fixture).unwrap()).unwrap();
    let evidence_fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-run-execution-evidence-v1.json");
    let evidence: Value =
        serde_json::from_slice(&std::fs::read(&evidence_fixture).unwrap()).unwrap();
    let request = serde_json::json!({
        "run_observed": {
            "api_version": "forge.run.observed.v1",
            "owner_ref": evidence["owner_ref"].clone(),
            "conversation_id": "c-1",
            "run_id": "run-1",
            "prompt_id": "prompt-001",
            "created_at_ms": 200,
            "latest_sequence": 5,
            "status": "nonterminal",
            "metadata_observed": true,
            "content_included": false,
            "authority": {
                "identity_verified": false, "owner_authorized": false,
                "run_authoritative": false, "persistence_attested": false,
                "content_provenance_verified": false, "reservation_created": false,
                "execution_authorized": false, "dispatch_performed": false
            }
        },
        "session_receipt_observed": {
            "schema_version": receipt["schema_version"].clone(),
            "evaluation_mode": receipt["evaluation_mode"].clone(),
            "owner": receipt["owner"].clone(),
            "conversation_id": "c-1",
            "prompt_id": receipt["prompt_id"].clone(),
            "run_id": "run-1",
            "receipt_observation": receipt["receipt_observation"].clone(),
            "prompt_run_binding_valid": true,
            "receipt_binding_valid": true,
            "preview_only": true,
            "selected_target_id": Value::Null,
            "authority": receipt["authority"].clone()
        }
    });
    let response = serde_json::json!({
        "api_version": "forge.run.execution-evidence.v1",
        "evaluation_mode": "pure_run_execution_evidence_binding",
        "owner_ref": evidence["owner_ref"].clone(),
        "conversation_id": "c-1", "run_id": "run-1", "prompt_id": "prompt-001",
        "run_status": "nonterminal", "attempt_id": "attempt-001", "target_id": "runner-1",
        "command_id": "command-001", "command_sha256": evidence["command_sha256"].clone(),
        "disposition_kind": "completed", "receipt_observed_at_ms": 300,
        "uncertain": false, "reconciliation_required": false,
        "metadata_observed": true, "content_included": false,
        "authority": {
            "identity_verified": false, "owner_authorized": false, "run_authoritative": false,
            "receipt_persisted": false, "reservation_created": false,
            "execution_authorized": false, "dispatch_performed": false, "audit_published": false
        }
    });
    let expected_request = request.clone();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut history, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
        respond(
            &mut history,
            "200 OK",
            &serde_json::json!({"conversation_id":"c-1","prompts":[],"has_more":false}),
        );
        let (mut stream, request_line, headers, body) = accept_request(&listener);
        assert!(
            request_line.starts_with(
                "POST /api/v1/conversations/c-1/runs/run-1/execution-evidence/preview "
            ),
            "{request_line}"
        );
        assert!(headers.contains("authorization: bearer test-token"));
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            expected_request
        );
        respond(&mut stream, "200 OK", &response);
    });
    let input = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(input.path(), serde_json::to_vec(&request).unwrap()).unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "open c-1\nrun-execution-evidence-remote-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("authenticated Run execution evidence [forge.run.execution-evidence.v1]"),
        "{output}"
    );
    assert!(output.contains("conversation=c-1 prompt=prompt-001 run=run-1 status=nonterminal"));
    assert!(output.contains("disposition=completed observed_at_ms=300"));
    assert!(
        output.contains("content_included=false uncertain=false reconciliation_required=false")
    );
    assert!(output.contains("execution_authorized=false dispatch_performed=false"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_can_preview_a_runner_execution_intent_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-runner-execution-intent-v1.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "runner-execution-intent-preview --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output
            .contains("offline Runner execution intent preview [forge.runner-execution-intent/v1]"),
        "{output}"
    );
    assert!(output.contains("conversation=conversation-001 prompt=prompt-001 run=run-001"));
    assert!(output.contains("attempt=attempt-001 command=command-001 target=runner-1"));
    assert!(output.contains("runner_command_binding_valid=true preview_only=true"));
    assert!(output.contains("execution_authorized=false dispatch_performed=false"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_can_preview_persisted_inventory_observation_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../../docs/contracts/fixtures/forge-device-inventory-persisted-observation-v1.json",
    );
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "inventory persisted-observation --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "offline persisted inventory observation [forge.device-inventory-observation/v1]"
        ),
        "{output}"
    );
    assert!(output.contains("devices=2"), "{output}");
    assert!(
        output.contains("device-a / runner-a: approval=pending"),
        "{output}"
    );
    assert!(
        output.contains("cpu=7 memory=8192 storage=51200"),
        "{output}"
    );
    assert!(
        output.contains("authority: identity_verified=false"),
        "{output}"
    );
    assert!(!output.contains("/api/v1/devices"), "{output}");
}

#[tokio::test]
async fn remote_tui_can_preview_persisted_inventory_observation_v2_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-device-inventory-observation-v2.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "inventory persisted-observation-v2 --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "offline persisted inventory observation v2 [forge.device-inventory-observation/v2]"
        ),
        "{output}"
    );
    assert!(output.contains("devices=2"), "{output}");
    assert!(output.contains("reservation=reserved"), "{output}");
    assert!(
        output.contains("gpus=[gpu-a:12884901888,gpu-b:4294967296]"),
        "{output}"
    );
    assert!(
        output.contains("authority: execution_authorized=false reservation_created=false dispatch_performed=false"),
        "{output}"
    );
    assert!(!output.contains("/api/v1/devices"), "{output}");
}

#[tokio::test]
async fn remote_tui_rejects_noncanonical_persisted_inventory_v2_tags() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });
    let mut malformed: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-device-inventory-observation-v2.json"
    ))
    .unwrap();
    malformed["devices"][0]["device"]["runtimes"] = json!(["oci/container"]);
    let mut file = NamedTempFile::new().unwrap();
    serde_json::to_writer(file.as_file_mut(), &malformed).unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "inventory persisted-observation-v2 --input {}\nquit\n",
        file.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("Device inventory persisted-observation-v2 failed"),
        "{output}"
    );
    assert!(output.contains("canonical tag"), "{output}");
    assert!(!output.contains("/api/v1/devices"), "{output}");
}

#[tokio::test]
async fn remote_tui_can_preview_run_execution_evidence_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-run-execution-evidence-v1.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "run-execution-evidence-preview --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("offline Run execution-evidence preview [forge.run.execution-evidence.v1]"),
        "{output}"
    );
    assert!(output.contains("conversation=conversation-001 prompt=prompt-001 run=run-001"));
    assert!(output.contains("content_included=false"));
    assert!(output.contains("authority: identity_verified=false"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_can_preview_run_observed_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-run-observed-v1.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "run-observed-preview --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("offline Run observed preview [forge.run.observed.v1]"),
        "{output}"
    );
    assert!(output.contains("conversation=conversation-001 prompt=prompt-001 run=run-001"));
    assert!(output.contains("content_included=false"));
    assert!(output.contains("authority: identity_verified=false"));
    assert!(!output.contains("/api/v1/devices"));
}
