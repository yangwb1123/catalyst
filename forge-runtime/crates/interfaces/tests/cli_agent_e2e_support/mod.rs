use std::{
    fmt::Write as FmtWrite,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Command, Output},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

use serde_json::{Value, json};

pub(super) struct LocalResponses {
    endpoint: String,
    requests: Arc<Mutex<Vec<Value>>>,
    worker: Option<JoinHandle<()>>,
}

impl LocalResponses {
    pub(super) fn start(responses: Vec<String>) -> Self {
        Self::start_delayed(responses, Duration::ZERO)
    }

    pub(super) fn start_delayed(responses: Vec<String>, response_delay: Duration) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local Responses server");
        let address = listener.local_addr().expect("local Responses address");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&requests);
        let worker = thread::spawn(move || serve(&listener, responses, &captured, response_delay));
        Self {
            endpoint: format!("http://{address}/v1"),
            requests,
            worker: Some(worker),
        }
    }

    pub(super) fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub(super) fn finish(mut self) -> Vec<Value> {
        self.worker
            .take()
            .expect("server worker")
            .join()
            .expect("local Responses server succeeded");
        Arc::try_unwrap(self.requests)
            .expect("all request owners released")
            .into_inner()
            .expect("request capture lock")
    }
}

pub(super) fn invoke_agent(
    endpoint: &str,
    state: &std::path::Path,
    project: &std::path::Path,
    dev: bool,
) -> Output {
    invoke_agent_with_output(endpoint, state, project, dev, false)
}

pub(super) fn invoke_agent_json(
    endpoint: &str,
    state: &std::path::Path,
    project: &std::path::Path,
) -> Output {
    invoke_agent_with_output(endpoint, state, project, false, true)
}

pub(super) fn invoke_agent_stdin(
    endpoint: &str,
    state: &std::path::Path,
    project: &std::path::Path,
    prompt: &[u8],
) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args(["--state-dir", path_text(state)])
        .args(["-C", path_text(project)])
        .args(["agent", "--model", "offline-test-model", "-"])
        .env("OPENAI_BASE_URL", endpoint)
        .env("OPENAI_API_KEY", "dummy-offline-key")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn Agent with stdin Prompt");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(prompt)
        .expect("write stdin Prompt");
    child.wait_with_output().expect("wait for stdin Agent")
}

#[allow(dead_code)]
pub(super) fn spawn_resume(
    state: &std::path::Path,
    project: &std::path::Path,
    run_id: &str,
) -> std::process::Child {
    Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "--state-dir",
            path_text(state),
            "-C",
            path_text(project),
            "run",
            "resume",
            run_id,
        ])
        .env("OPENAI_API_KEY", "dummy-offline-key")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn explicit resume contender")
}

pub(super) fn invoke_keyed_agent(
    endpoint: &str,
    state: &std::path::Path,
    project: &std::path::Path,
    key: &str,
    agent_options: &[&str],
    prompt: &str,
) -> Output {
    invoke_keyed_agent_with_output(endpoint, state, project, key, agent_options, prompt, false)
}

pub(super) fn invoke_keyed_agent_json(
    endpoint: &str,
    state: &std::path::Path,
    project: &std::path::Path,
    key: &str,
    agent_options: &[&str],
    prompt: &str,
) -> Output {
    invoke_keyed_agent_with_output(endpoint, state, project, key, agent_options, prompt, true)
}

fn invoke_keyed_agent_with_output(
    endpoint: &str,
    state: &std::path::Path,
    project: &std::path::Path,
    key: &str,
    agent_options: &[&str],
    prompt: &str,
    json: bool,
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_forge-runtime"));
    command.args(["--state-dir", path_text(state)]);
    if json {
        command.arg("--json");
    }
    command.args(["--idempotency-key", key, "-C", path_text(project), "agent"]);
    command.args(agent_options).arg(prompt);
    command
        .env("OPENAI_BASE_URL", endpoint)
        .env("OPENAI_API_KEY", "dummy-offline-key")
        .output()
        .expect("invoke keyed Agent")
}

pub(super) fn assert_agent_trust(event: &Value, dev: bool) {
    assert_eq!(event["dev"], dev);
    assert_eq!(
        event["agent_trust_boundary"],
        json!({
            "same_user_execution": true,
            "workspace_read": true,
            "workspace_write": dev,
            "workspace_delete_possible": dev,
            "process_execution": dev,
            "provider_egress": true,
            "provider_payload_scope": [
                "prompt",
                "conversation_history",
                "model_selected_workspace_content",
                "tool_arguments_and_outputs",
            ],
            "local_plaintext_journal": true,
            "filesystem_isolation_enforced": false,
            "network_isolation_enforced": false,
        })
    );
}

pub(super) fn assert_dev_warning(output: &Output) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    for warning in [
        "may modify or delete",
        "same-user",
        "not an OS sandbox",
        "sent off-machine",
        "stored locally in plaintext",
    ] {
        assert!(
            stderr.contains(warning),
            "missing {warning:?} in:\n{stderr}"
        );
    }
}

pub(super) fn assert_prior_conversation_history(request: &Value) {
    let input = request["input"].as_array().expect("input array");
    let messages: Vec<_> = input
        .iter()
        .filter(|item| item["type"] == "message")
        .map(|item| {
            (
                item["role"].as_str().expect("message role"),
                item["content"].as_str().expect("message content"),
            )
        })
        .collect();
    assert_eq!(
        messages,
        [
            ("user", "repair note.txt and verify it"),
            ("assistant", "first answer"),
            ("user", "repair note.txt and verify it"),
        ]
    );
}

fn invoke_agent_with_output(
    endpoint: &str,
    state: &std::path::Path,
    project: &std::path::Path,
    dev: bool,
    json: bool,
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_forge-runtime"));
    command.args(["--state-dir", path_text(state)]);
    if json {
        command.arg("--json");
    }
    command
        .args(["-C", path_text(project)])
        .args(["agent", "--model", "offline-test-model"]);
    if dev {
        command.arg("--dev");
    }
    command
        .arg("repair note.txt and verify it")
        .env("OPENAI_BASE_URL", endpoint)
        .env("OPENAI_API_KEY", "dummy-offline-key")
        .output()
        .expect("run first-party agent CLI")
}

pub(super) fn tool_stream(sequence: usize, name: &str, arguments: &Value) -> String {
    let item_id = format!("fc-{sequence}");
    let call_id = format!("call-{sequence}");
    let arguments = arguments.to_string();
    let added = json!({
        "type": "response.output_item.added",
        "item": {"id": item_id, "type": "function_call", "call_id": call_id, "name": name}
    });
    let done = json!({
        "type": "response.function_call_arguments.done",
        "item_id": item_id,
        "name": name,
        "arguments": arguments,
    });
    let completed = json!({
        "type": "response.completed",
        "response": {"status": "completed", "output": [function_item(
            sequence, name, &arguments
        )], "usage": null}
    });
    frames(&[added, done, completed])
}

pub(super) fn final_stream(answer: &str) -> String {
    let added = json!({
        "type": "response.output_item.added",
        "item": {"id": "msg-final", "type": "message", "role": "assistant",
                 "phase": "final_answer"}
    });
    let delta = json!({
        "type": "response.output_text.delta", "item_id": "msg-final", "delta": answer
    });
    let completed = json!({
        "type": "response.completed",
        "response": {"status": "completed", "output": [message_item(answer)], "usage": null}
    });
    frames(&[added, delta, completed])
}

pub(super) fn error_stream(message: &str) -> String {
    frames(&[json!({
        "type": "error",
        "code": "provider_fixture_failure",
        "message": message,
    })])
}

fn serve(
    listener: &TcpListener,
    responses: Vec<String>,
    requests: &Mutex<Vec<Value>>,
    response_delay: Duration,
) {
    for response in responses {
        let (mut stream, _) = listener.accept().expect("accept Responses request");
        let body = read_request(&mut stream);
        requests
            .lock()
            .expect("request capture lock")
            .push(serde_json::from_slice(&body).expect("Responses request JSON"));
        thread::sleep(response_delay);
        write_response(&mut stream, &response);
    }
}

fn read_request(stream: &mut TcpStream) -> Vec<u8> {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("set request timeout");
    let mut bytes = Vec::new();
    let header_end = read_headers(stream, &mut bytes);
    let length = content_length(&bytes[..header_end]);
    while bytes.len() < header_end + length {
        read_chunk(stream, &mut bytes);
    }
    bytes[header_end..header_end + length].to_vec()
}

fn read_headers(stream: &mut TcpStream, bytes: &mut Vec<u8>) -> usize {
    loop {
        if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            return index + 4;
        }
        read_chunk(stream, bytes);
        assert!(bytes.len() <= 64 * 1024, "request headers too large");
    }
}

fn read_chunk(stream: &mut TcpStream, bytes: &mut Vec<u8>) {
    let mut chunk = [0_u8; 8 * 1024];
    let count = stream.read(&mut chunk).expect("read Responses request");
    assert_ne!(count, 0, "Responses request ended early");
    bytes.extend_from_slice(&chunk[..count]);
}

fn content_length(headers: &[u8]) -> usize {
    let text = std::str::from_utf8(headers).expect("HTTP headers are UTF-8");
    text.lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().expect("numeric content length"))
        })
        .expect("content-length header")
}

fn write_response(stream: &mut TcpStream, body: &str) {
    let headers = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(headers.as_bytes())
        .and_then(|()| stream.write_all(body.as_bytes()))
        .and_then(|()| stream.flush())
        .expect("write Responses stream");
}

fn function_item(sequence: usize, name: &str, arguments: &str) -> Value {
    json!({
        "type": "function_call", "id": format!("fc-{sequence}"),
        "call_id": format!("call-{sequence}"), "name": name, "arguments": arguments,
        "status": "completed", "caller": {"type": "direct"}
    })
}

fn message_item(answer: &str) -> Value {
    json!({
        "type": "message", "id": "msg-final", "status": "completed",
        "role": "assistant", "phase": "final_answer",
        "content": [{"type": "output_text", "text": answer, "annotations": []}]
    })
}

fn frames(events: &[Value]) -> String {
    let mut output = String::new();
    for event in events {
        write!(
            output,
            "event: {}\ndata: {event}\n\n",
            event["type"].as_str().unwrap()
        )
        .expect("write event to String");
    }
    output
}

fn path_text(path: &std::path::Path) -> &str {
    path.to_str().expect("test path is UTF-8")
}
