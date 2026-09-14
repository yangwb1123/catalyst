use std::{io::Cursor, net::TcpListener, thread};

use serde_json::json;

use super::super::run_with_io;
use super::{
    accept_request, conversation_projection, respond, serve_conversation_page, test_client,
};

#[tokio::test]
async fn tui_reads_run_pages_and_payload_free_timeline_metadata() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || serve_run_observation(&listener));

    let client = test_client(address);
    let mut reader = Cursor::new(
        "runs\nruns --before 20 \"run-8\"\ntimeline \"run-8\"\ntimeline \"run-8\" 1\nquit\n",
    );
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Run \"run-8\" status=\"nonterminal\" latest_sequence=4"));
    assert!(output.contains("More Runs available: enter runs --before 20 \"run-8\"."));
    assert!(output.contains("Event seq=1 emitted_at_ms=21 type=\"run_started\""));
    assert!(output.contains("More events available: enter timeline \"run-8\" 1."));
    assert!(output.contains("Event seq=2 emitted_at_ms=22 type=\"activity\""));
    assert!(!output.contains("content"));
    assert!(!output.contains("tool_name"));
    assert!(!output.contains("output_path"));
}

fn serve_run_observation(listener: &TcpListener) {
    serve_conversation_page(
        listener,
        &json!({
            "conversations": [{
                "conversation": conversation_projection("c-1", "Shared"),
                "aggregate_version": 4
            }],
            "next_after_id": null,
            "has_more": false
        }),
    );
    serve_newest_run_page(listener);
    serve_older_run_page(listener);
    serve_first_timeline_page(listener);
    serve_next_timeline_page(listener);
}

fn serve_newest_run_page(listener: &TcpListener) {
    let (mut stream, request, _, _) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/conversations/c-1/runs?limit=25 "));
    respond(
        &mut stream,
        "200 OK",
        &json!({
            "conversation_id": "c-1",
            "runs": [{"run_id": "run-8", "prompt_id": "prompt-8", "created_at_ms": 20,
                "latest_sequence": 4, "status": "nonterminal"}],
            "next_cursor": {"created_at_ms": 20, "run_id": "run-8"},
            "has_more": true
        }),
    );
}

fn serve_older_run_page(listener: &TcpListener) {
    let (mut stream, request, _, _) = accept_request(listener);
    assert!(request.starts_with(
        "GET /api/v1/conversations/c-1/runs?limit=25&before_created_at_ms=20&before_run_id=run-8 "
    ));
    respond(
        &mut stream,
        "200 OK",
        &json!({
            "conversation_id": "c-1",
            "runs": [{"run_id": "run-7", "prompt_id": "prompt-7", "created_at_ms": 10,
                "latest_sequence": 2, "status": "completed"}],
            "next_cursor": null,
            "has_more": false
        }),
    );
}

fn serve_first_timeline_page(listener: &TcpListener) {
    let (mut stream, request, _, _) = accept_request(listener);
    assert!(request.starts_with(
        "GET /api/v1/conversations/c-1/runs/run-8/timeline?after_sequence=0&limit=128 "
    ));
    respond(
        &mut stream,
        "200 OK",
        &json!({
            "conversation_id": "c-1", "run_id": "run-8", "after_sequence": 0,
            "scanned_through_sequence": 1, "has_more": true,
            "events": [{"seq": 1, "emitted_at_ms": 21, "type": "run_started"}]
        }),
    );
}

fn serve_next_timeline_page(listener: &TcpListener) {
    let (mut stream, request, _, _) = accept_request(listener);
    assert!(request.starts_with(
        "GET /api/v1/conversations/c-1/runs/run-8/timeline?after_sequence=1&limit=128 "
    ));
    respond(
        &mut stream,
        "200 OK",
        &json!({
            "conversation_id": "c-1", "run_id": "run-8", "after_sequence": 1,
            "scanned_through_sequence": 2, "has_more": false,
            "events": [{"seq": 2, "emitted_at_ms": 22, "type": "activity"}]
        }),
    );
}

#[tokio::test]
async fn tui_rejects_and_does_not_print_detailed_timeline_payloads() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(
            &listener,
            &json!({
                "conversations": [{
                    "conversation": conversation_projection("c-1", "Shared"),
                    "aggregate_version": 4
                }],
                "next_after_id": null,
                "has_more": false
            }),
        );
        let (mut timeline, request, _, _) = accept_request(&listener);
        assert!(request.starts_with(
            "GET /api/v1/conversations/c-1/runs/run-8/timeline?after_sequence=0&limit=128 "
        ));
        respond(
            &mut timeline,
            "200 OK",
            &json!({
                "conversation_id": "c-1",
                "run_id": "run-8",
                "after_sequence": 0,
                "scanned_through_sequence": 1,
                "has_more": false,
                "events": [{
                    "seq": 1,
                    "emitted_at_ms": 21,
                    "type": "activity",
                    "content": "private event payload"
                }]
            }),
        );
    });

    let client = test_client(address);
    let mut reader = Cursor::new("timeline run-8\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Run timeline failed: Forge API returned an invalid Run timeline"));
    assert!(!output.contains("private event payload"));
}
