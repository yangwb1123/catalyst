use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    thread,
    time::Duration,
};

use reqwest::{Client, Url, redirect::Policy};
use serde_json::json;

use super::RemoteClient;
use super::credentials::{CredentialStore, StoredCredential};

#[cfg(unix)]
#[tokio::test]
async fn resumed_timeline_reads_and_advances_the_owner_run_checkpoint() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut request_line = String::new();
        reader.read_line(&mut request_line).unwrap();
        assert!(request_line.starts_with(
            "GET /api/v1/conversations/conversation-1/runs/run-1/timeline?after_sequence=3&limit=2 "
        ));
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            if header == "\r\n" {
                break;
            }
        }
        write_response(
            &mut stream,
            "200 OK",
            &json!({
                "conversation_id": "conversation-1",
                "run_id": "run-1",
                "after_sequence": 3,
                "scanned_through_sequence": 4,
                "has_more": false,
                "events": [{"seq": 4, "emitted_at_ms": 77, "type": "activity"}]
            }),
        );
    });

    let root = tempfile::tempdir_in(std::env::var("HOME").unwrap()).unwrap();
    set_private_mode(&root);
    let store = CredentialStore::for_test(root.path().to_path_buf());
    let credential = StoredCredential {
        issuer: "https://id.example".into(),
        client_id: "forge-cli".into(),
        subject: "user-a".into(),
        tenant_id: "tenant-a".into(),
        access_token: "test-token".into(),
        expires_at_unix: 4_000_000_000,
    };
    store.save(&credential).unwrap();
    let cursor = store.change_cursor_store(&format!("http://{address}/"), &credential);
    let run_cursor = cursor.run_timeline_cursor_store("conversation-1", "run-1");
    assert!(run_cursor.save(3).unwrap());

    let client = RemoteClient {
        http: Client::builder()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap(),
        base_url: Url::parse(&format!("http://{address}/")).unwrap(),
        access_token: "test-token".into(),
        change_cursor: Some(cursor),
        token_refresh: None,
    };
    let page = client
        .resumed_run_timeline("conversation-1", "run-1", 2)
        .await
        .unwrap();
    assert_eq!(page["scanned_through_sequence"], 4);
    assert_eq!(run_cursor.load().unwrap(), 4);
    server.join().unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn failed_resumed_timeline_does_not_advance_the_checkpoint() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut request_line = String::new();
        reader.read_line(&mut request_line).unwrap();
        while request_line != "\r\n" {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            if header == "\r\n" {
                break;
            }
        }
        write_response(
            &mut stream,
            "200 OK",
            &json!({
                "conversation_id": "conversation-1",
                "run_id": "run-1",
                "after_sequence": 3,
                "scanned_through_sequence": 5,
                "has_more": false,
                "events": [{"seq": 5, "emitted_at_ms": 77, "type": "activity"}]
            }),
        );
    });

    let root = tempfile::tempdir_in(std::env::var("HOME").unwrap()).unwrap();
    set_private_mode(&root);
    let store = CredentialStore::for_test(root.path().to_path_buf());
    let credential = StoredCredential {
        issuer: "https://id.example".into(),
        client_id: "forge-cli".into(),
        subject: "user-a".into(),
        tenant_id: "tenant-a".into(),
        access_token: "test-token".into(),
        expires_at_unix: 4_000_000_000,
    };
    store.save(&credential).unwrap();
    let cursor = store.change_cursor_store(&format!("http://{address}/"), &credential);
    let run_cursor = cursor.run_timeline_cursor_store("conversation-1", "run-1");
    assert!(run_cursor.save(3).unwrap());

    let client = RemoteClient {
        http: Client::builder()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap(),
        base_url: Url::parse(&format!("http://{address}/")).unwrap(),
        access_token: "test-token".into(),
        change_cursor: Some(cursor),
        token_refresh: None,
    };
    assert!(
        client
            .resumed_run_timeline("conversation-1", "run-1", 2)
            .await
            .is_err()
    );
    assert_eq!(run_cursor.load().unwrap(), 3);
    server.join().unwrap();
}

fn write_response(stream: &mut TcpStream, status: &str, body: &serde_json::Value) {
    let encoded = serde_json::to_vec(body).unwrap();
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        encoded.len()
    )
    .unwrap();
    stream.write_all(&encoded).unwrap();
}

#[cfg(unix)]
fn set_private_mode(directory: &tempfile::TempDir) {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
}
