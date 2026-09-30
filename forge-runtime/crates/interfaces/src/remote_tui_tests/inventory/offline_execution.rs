use super::*;

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
