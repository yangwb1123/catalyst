#![cfg(unix)]

#[path = "cli_agent_e2e_support/mod.rs"]
#[allow(dead_code)]
mod e2e_support;
#[allow(dead_code)]
mod support;

use std::fs;

use serde_json::{Value, json};
use tempfile::TempDir;

use e2e_support::{LocalResponses, final_stream, invoke_agent, tool_stream};
use support::assert_success;

#[test]
fn read_only_agent_discovers_searches_and_reads_through_the_real_cli() {
    let project = TempDir::new().expect("project");
    let state = TempDir::new().expect("state");
    fs::create_dir_all(project.path().join(".git/objects")).expect("Git metadata directory");
    fs::create_dir(project.path().join("src")).expect("source directory");
    for index in 0..1_100 {
        fs::write(
            project.path().join(format!(".git/objects/{index}")),
            "needle must stay outside discovery",
        )
        .expect("Git metadata fixture");
    }
    fs::write(
        project.path().join("src/lib.rs"),
        "pub fn needle() -> bool { true }\n",
    )
    .expect("source fixture");
    let server = LocalResponses::start(discovery_responses());

    let output = invoke_agent(server.endpoint(), state.path(), project.path(), false);
    let requests = server.finish();

    assert_success(&output);
    assert_eq!(requests.len(), 4);
    for request in &requests {
        assert_eq!(
            tool_names(request),
            vec!["list_files", "read_file", "search_text"]
        );
    }
    assert_output(&requests, 1, "call-1", "src/lib.rs");
    assert_output(&requests, 2, "call-2", "needle");
    assert_output(&requests, 3, "call-3", "pub fn needle");
}

fn discovery_responses() -> Vec<String> {
    vec![
        tool_stream(
            1,
            "list_files",
            &json!({"path": ".", "max_depth": 4, "max_entries": 100}),
        ),
        tool_stream(
            2,
            "search_text",
            &json!({
                "query": "needle",
                "path": ".",
                "max_depth": 4,
                "max_entries": 100,
                "max_results": 10
            }),
        ),
        tool_stream(3, "read_file", &json!({"path": "src/lib.rs"})),
        final_stream("found the implementation"),
    ]
}

fn tool_names(request: &Value) -> Vec<&str> {
    request["tools"]
        .as_array()
        .expect("tool array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool name"))
        .collect()
}

fn assert_output(requests: &[Value], request_index: usize, call_id: &str, expected: &str) {
    let output = requests[request_index]["input"]
        .as_array()
        .expect("input array")
        .iter()
        .find(|item| item["type"] == "function_call_output" && item["call_id"] == call_id)
        .expect("function call output");
    assert!(output["output"].as_str().unwrap().contains(expected));
}
