use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::Arc,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::{Client, Url};
use serde_json::json;

use super::super::{
    client::RemoteClient,
    credentials::{CredentialStore, MemoryRefreshTokenBackend, OwnerSelector, StoredCredential},
};
use super::SavedTokenProvider;

#[cfg(unix)]
#[tokio::test]
async fn refresh_rotation_is_used_for_the_api_request_and_persisted() {
    let (issuer, server) = spawn_server(ServerReply::Refresh, 2);
    let next_token = jwt(&issuer, "forge-cli", "user-a", "tenant-a", FUTURE_EXPIRY);
    let (provider, store, old, _directory) = saved_provider(&issuer, "refresh-old");
    let http = Client::builder().build().unwrap();
    let client = RemoteClient {
        http,
        base_url: Url::parse(&issuer).unwrap(),
        access_token: old.access_token.clone(),
        change_cursor: None,
        token_refresh: Some(Arc::new(provider)),
    };

    let response = client
        .send_json(client.http.get(client.endpoint("/protected").unwrap()))
        .await
        .unwrap();
    assert_eq!(response, json!({"ok": true}));

    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].path, "/token");
    assert!(requests[0].authorization.is_none());
    let form: std::collections::HashMap<_, _> =
        url::form_urlencoded::parse(requests[0].body.as_bytes())
            .into_owned()
            .collect();
    assert_eq!(
        form.get("grant_type").map(String::as_str),
        Some("refresh_token")
    );
    assert_eq!(
        form.get("refresh_token").map(String::as_str),
        Some("refresh-old")
    );
    assert_eq!(form.get("client_id").map(String::as_str), Some("forge-cli"));
    assert!(!form.contains_key("client_secret"));
    assert_eq!(requests[1].path, "/protected");
    assert_eq!(
        requests[1].authorization.as_deref(),
        Some(format!("Bearer {next_token}").as_str())
    );

    let selector = OwnerSelector {
        subject: Some("user-a"),
        tenant_id: Some("tenant-a"),
    };
    let persisted = store.load(&issuer, "forge-cli", selector).unwrap();
    assert_eq!(persisted.access_token, next_token);
    assert_eq!(
        store.load_refresh_token(&persisted).unwrap().as_deref(),
        Some("refresh-new")
    );
}

#[cfg(unix)]
#[tokio::test]
async fn concurrent_saved_providers_only_rotate_a_refresh_token_once() {
    let (issuer, server) = spawn_server(ServerReply::Refresh, 2);
    let next_token = jwt(&issuer, "forge-cli", "user-a", "tenant-a", FUTURE_EXPIRY);
    let (first, store, old, _directory) = saved_provider(&issuer, "refresh-old");
    let second = SavedTokenProvider::new(old, store).unwrap();

    let (first_result, second_result) = tokio::join!(first.access_token(), second.access_token());

    assert_eq!(first_result.unwrap(), next_token);
    assert_eq!(second_result.unwrap(), next_token);
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/token");
}

#[cfg(unix)]
#[tokio::test]
async fn invalid_grant_removes_the_stale_refresh_token() {
    let (issuer, server) = spawn_server(ServerReply::InvalidGrant, 1);
    let (provider, store, old, _directory) = saved_provider(&issuer, "refresh-old");

    let error = provider.access_token().await.unwrap_err();

    assert!(error.to_string().contains("run remote login"));
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        store.load_refresh_token(&old).unwrap(),
        None,
        "invalid_grant must clear the shared keyring entry"
    );
}

#[cfg(unix)]
fn saved_provider(
    issuer: &str,
    refresh_token: &str,
) -> (
    SavedTokenProvider,
    CredentialStore,
    StoredCredential,
    tempfile::TempDir,
) {
    let directory = tempfile::tempdir_in(std::env::var("HOME").unwrap()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let backend = Arc::new(MemoryRefreshTokenBackend::default());
    let store = CredentialStore::with_test_backend(directory.path().to_path_buf(), backend);
    let expired = unix_now().saturating_sub(1);
    let old = StoredCredential {
        issuer: issuer.to_owned(),
        client_id: "forge-cli".into(),
        subject: "user-a".into(),
        tenant_id: "tenant-a".into(),
        access_token: jwt(issuer, "forge-cli", "user-a", "tenant-a", expired),
        expires_at_unix: expired,
    };
    store.save(&old).unwrap();
    store.save_refresh_token(&old, refresh_token).unwrap();
    let provider = SavedTokenProvider::new(old.clone(), store.clone()).unwrap();
    (provider, store, old, directory)
}

#[cfg(unix)]
#[derive(Clone, Copy)]
enum ServerReply {
    Refresh,
    InvalidGrant,
}

#[cfg(unix)]
const FUTURE_EXPIRY: u64 = 4_102_444_800;

#[cfg(unix)]
fn spawn_server(
    reply: ServerReply,
    maximum_requests: usize,
) -> (String, thread::JoinHandle<Vec<Request>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let issuer = format!("http://{address}");
    let server_issuer = issuer.clone();
    let server = thread::spawn(move || {
        let mut requests = Vec::new();
        let mut last_request = Instant::now();
        while requests.len() < maximum_requests {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let request = read_request(&mut stream);
                    let (status, body) = if request.path == "/token" {
                        match reply {
                            ServerReply::Refresh => (
                                200,
                                json!({
                                    "access_token": jwt(&server_issuer, "forge-cli", "user-a", "tenant-a", FUTURE_EXPIRY),
                                    "token_type": "Bearer",
                                    "expires_in": 3600,
                                    "refresh_token": "refresh-new",
                                })
                                .to_string(),
                            ),
                            ServerReply::InvalidGrant => {
                                (400, json!({"error": "invalid_grant"}).to_string())
                            }
                        }
                    } else {
                        (200, r#"{"ok":true}"#.into())
                    };
                    write_response(&mut stream, status, &body);
                    requests.push(request);
                    last_request = Instant::now();
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if !requests.is_empty() && last_request.elapsed() > Duration::from_millis(300) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("could not accept test request: {error}"),
            }
        }
        requests
    });
    (issuer, server)
}

#[cfg(unix)]
struct Request {
    path: String,
    authorization: Option<String>,
    body: String,
}

#[cfg(unix)]
fn read_request(stream: &mut TcpStream) -> Request {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    reader.read_line(&mut request_line).unwrap();
    let path = request_line
        .split_ascii_whitespace()
        .nth(1)
        .unwrap()
        .to_owned();
    let mut authorization = None;
    let mut content_length = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" || line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("authorization") {
                authorization = Some(value.trim().to_owned());
            } else if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap();
            }
        }
    }
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body).unwrap();
    Request {
        path,
        authorization,
        body: String::from_utf8(body).unwrap(),
    }
}

#[cfg(unix)]
fn write_response(stream: &mut TcpStream, status: u16, body: &str) {
    let status_text = if status == 200 { "OK" } else { "Bad Request" };
    write!(
        stream,
        "HTTP/1.1 {status} {status_text}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    stream.flush().unwrap();
}

fn jwt(issuer: &str, client_id: &str, subject: &str, tenant_id: &str, expiry: u64) -> String {
    let claims = json!({
        "iss": issuer,
        "client_id": client_id,
        "sub": subject,
        "tenant_id": tenant_id,
        "exp": expiry,
        "aud": ["forge-api"],
        "scopes": ["forge:conversations:read", "forge:conversations:write"],
    });
    let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap());
    format!("e30.{payload}.signature")
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
