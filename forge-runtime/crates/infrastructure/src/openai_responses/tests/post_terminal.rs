use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread,
    time::Duration,
};

use forge_runtime_domain::{
    Cancellation, Message, ModelEvent, ModelFinishReason, ModelRequest, PreparedModelProvider,
};
use futures_util::StreamExt;

use super::{OpenAiResponsesProvider, response_fixtures::text_stream};

const SECRET: &str = "sk-post-terminal-test";

#[tokio::test]
async fn prepared_stream_requires_real_http_eof_after_terminal_frame() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind loopback server");
    let address = listener.local_addr().expect("loopback address");
    let server = thread::spawn(move || serve_terminal_then_extra(&listener));
    let provider = OpenAiResponsesProvider::new_insecure_for_test(
        format!("http://{address}/v1"),
        "test-model",
        SECRET,
    )
    .expect("loopback provider");
    let prepared = provider
        .prepare_request(request())
        .expect("prepare request");

    let events = provider.stream_prepared(prepared).collect::<Vec<_>>().await;
    server.join().expect("loopback server");

    assert!(events.iter().any(|event| {
        matches!(
            event,
            Ok(ModelEvent::Finished {
                reason: ModelFinishReason::Completed
            })
        )
    }));
    let error = events
        .last()
        .expect("post-terminal result")
        .as_ref()
        .expect_err("event after terminal must fail");
    assert_eq!(error.code, "provider_protocol");
    assert!(error.message.contains("followed a terminal event"));
}

fn serve_terminal_then_extra(listener: &TcpListener) {
    let (mut socket, _) = listener.accept().expect("accept request");
    read_request(&mut socket);
    socket
        .write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\
              Transfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
        )
        .expect("write response headers");
    write_chunk(&mut socket, text_stream().as_bytes());
    socket.flush().expect("flush terminal chunk");
    thread::sleep(Duration::from_millis(25));
    write_chunk(
        &mut socket,
        b"event: response.created\ndata: {\"type\":\"response.created\"}\n\n",
    );
    socket
        .write_all(b"0\r\n\r\n")
        .expect("finish chunked response");
}

fn read_request(socket: &mut TcpStream) {
    socket
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("request timeout");
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
        let read = socket.read(&mut buffer).expect("read request");
        assert!(read > 0, "request ended before its headers");
        request.extend_from_slice(&buffer[..read]);
    }
}

fn write_chunk(socket: &mut TcpStream, bytes: &[u8]) {
    socket
        .write_all(format!("{:X}\r\n", bytes.len()).as_bytes())
        .expect("write chunk length");
    socket.write_all(bytes).expect("write chunk body");
    socket.write_all(b"\r\n").expect("finish chunk");
}

fn request() -> ModelRequest {
    ModelRequest {
        system_prompt: "system".into(),
        messages: vec![Message::User { text: "go".into() }],
        tools: Vec::new(),
        max_output_tokens: 1_024,
        cancellation: Cancellation::default(),
    }
}
