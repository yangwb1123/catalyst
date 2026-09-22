pub(super) fn test_client(address: std::net::SocketAddr) -> RemoteClient {
    RemoteClient {
        http: Client::builder()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap(),
        base_url: Url::parse(&format!("http://{address}")).unwrap(),
        access_token: "test-token".into(),
        change_cursor: None,
        token_refresh: None,
    }
}

pub(super) fn serve_conversation_page(listener: &TcpListener, payload: &Value) {
    let (mut stream, request, _, _) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/conversations?"));
    respond(&mut stream, "200 OK", payload);
}

pub(super) fn accept_stream(
    listener: &TcpListener,
) -> (std::net::TcpStream, std::net::SocketAddr) {
    listener.set_nonblocking(true).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        match listener.accept() {
            Ok((stream, address)) => {
                // Windows may carry the listener's nonblocking mode onto the
                // accepted socket; request reads expect a blocking stream.
                stream.set_nonblocking(false).unwrap();
                return (stream, address);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(
                    std::time::Instant::now() < deadline,
                    "timed out waiting for the TUI test client connection"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("accepting the TUI test client failed: {error}"),
        }
    }
}

pub(super) fn accept_request(
    listener: &TcpListener,
) -> (std::net::TcpStream, String, String, Vec<u8>) {
    let (mut stream, _) = accept_stream(listener);
    let (request, headers, body) = read_request(&mut stream);
    (stream, request, headers, body)
}

pub(super) fn read_request(stream: &mut std::net::TcpStream) -> (String, String, Vec<u8>) {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    reader.read_line(&mut request_line).unwrap();
    let mut headers = String::new();
    let mut content_length = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" || line.is_empty() {
            break;
        }
        let normalized = line.to_ascii_lowercase();
        if let Some(value) = normalized.strip_prefix("content-length:") {
            content_length = value.trim().parse::<usize>().unwrap();
        }
        headers.push_str(&normalized);
    }
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body).unwrap();
    (request_line.trim().into(), headers, body)
}

pub(super) fn respond(stream: &mut std::net::TcpStream, status: &str, payload: &Value) {
    let bytes = serde_json::to_vec(payload).unwrap();
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        bytes.len()
    )
    .unwrap();
    stream.write_all(&bytes).unwrap();
}
