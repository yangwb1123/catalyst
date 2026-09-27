use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    thread,
    time::Duration,
};

use reqwest::{Client, Url, redirect::Policy};

use super::{
    DEFAULT_CLIENT_ID, DEVICE_GRANT, DeviceClient, RESOURCE, SCOPES, increase_interval,
    parse_issuer, validate_verification_uri,
};

const DEVICE_OBSERVATION_SCOPE: &str = "forge:devices:read";
const DEVICE_OBSERVATION_CLIENT_ID: &str = "forge-device-observer";

#[tokio::test]
async fn device_flow_uses_form_encoding_without_credentials_or_bearer() {
    let (issuer, server) =
        spawn_server(vec![(200, authorization_body("__ISSUER_ORIGIN__/verify"))]);
    let client = test_client(&issuer);
    let authorization = client.request_device_code().await.unwrap();
    assert_eq!(authorization.verification_uri, format!("{issuer}verify"));
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 1);
    assert_no_oauth_client_secret_or_authorization(&requests[0]);
    let form = parse_form(&requests[0].body);
    assert_eq!(form.get("client_id").map(String::as_str), Some("forge-cli"));
    assert_eq!(form.get("scope").map(String::as_str), Some(SCOPES));
    assert_eq!(form.get("resource").map(String::as_str), Some(RESOURCE));
}

#[tokio::test]
async fn preserves_server_poll_interval_without_shortening_it() {
    let body = authorization_body("__ISSUER_ORIGIN__/verify")
        .replace("\"interval\":1", "\"interval\":120");
    let (issuer, server) = spawn_server(vec![(200, body)]);
    let authorization = test_client(&issuer).request_device_code().await.unwrap();
    assert_eq!(authorization.interval, 120);
    server.join().unwrap();
}

#[tokio::test]
async fn pending_then_approved_polls_with_device_grant_and_never_persists_in_hub() {
    let (issuer, server) = spawn_server(vec![
        (200, authorization_body("__ISSUER_ORIGIN__/verify")),
        (400, r#"{"error":"authorization_pending"}"#.into()),
        (
            200,
            r#"{"access_token":"secret-token","token_type":"Bearer","expires_in":900,"refresh_token":"refresh-secret"}"#.into(),
        ),
    ]);
    let client = test_client(&issuer);
    let authorization = client.request_device_code().await.unwrap();
    let token = client.poll_until_approved(&authorization).await.unwrap();
    assert_eq!(token.access_token, "secret-token");
    assert_eq!(token.refresh_token.as_deref(), Some("refresh-secret"));
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 3);
    for request in &requests {
        assert_no_oauth_client_secret_or_authorization(request);
    }
    let form = parse_form(&requests[1].body);
    assert_eq!(
        form.get("grant_type").map(String::as_str),
        Some(DEVICE_GRANT)
    );
    assert_eq!(
        form.get("device_code").map(String::as_str),
        Some("device+code")
    );
    assert_eq!(form.get("client_id").map(String::as_str), Some("forge-cli"));
}

#[tokio::test]
async fn slow_down_increases_poll_interval_by_rfc_amount() {
    assert_eq!(increase_interval(1), 6);
    assert_eq!(increase_interval(296), 301);
    let (issuer, server) = spawn_server(vec![
        (200, authorization_body("__ISSUER_ORIGIN__/verify")),
        (400, r#"{"error":"slow_down"}"#.into()),
        (400, r#"{"error":"access_denied"}"#.into()),
    ]);
    let client = test_client(&issuer);
    let authorization = client.request_device_code().await.unwrap();
    let started = tokio::time::Instant::now();
    assert!(client.poll_until_approved(&authorization).await.is_err());
    assert!(started.elapsed() >= Duration::from_secs(6));
    assert_eq!(server.join().unwrap().len(), 3);
}

#[tokio::test]
async fn denied_and_expired_token_errors_do_not_return_credentials() {
    for error in ["access_denied", "expired_token"] {
        let (issuer, server) = spawn_server(vec![
            (200, authorization_body("__ISSUER_ORIGIN__/verify")),
            (400, format!(r#"{{"error":"{error}"}}"#)),
        ]);
        let client = test_client(&issuer);
        let authorization = client.request_device_code().await.unwrap();
        let result = client.poll_until_approved(&authorization).await;
        assert!(result.is_err());
        assert_eq!(server.join().unwrap().len(), 2);
    }
}

#[tokio::test]
async fn rejects_response_overflow_redirect_and_unsafe_verification_uri() {
    let (issuer, server) = spawn_server(vec![(200, "x".repeat(65 * 1024))]);
    assert!(test_client(&issuer).request_device_code().await.is_err());
    assert_eq!(server.join().unwrap().len(), 1);

    let (issuer, server) = spawn_redirect_server();
    assert!(test_client(&issuer).request_device_code().await.is_err());
    assert_eq!(server.join().unwrap().len(), 1);

    let remote_issuer = parse_issuer("https://login.example").unwrap().endpoint;
    assert!(validate_verification_uri("http://login.example/verify", &remote_issuer).is_err());
    assert!(validate_verification_uri("http://127.0.0.1:9000/verify", &remote_issuer).is_err());
    assert!(validate_verification_uri("https://verify.example/verify", &remote_issuer).is_ok());
    assert!(
        validate_verification_uri("https://user@verify.example/verify", &remote_issuer).is_err()
    );
    let loopback_issuer = parse_issuer("http://127.0.0.1:9000").unwrap().endpoint;
    assert!(validate_verification_uri("http://127.0.0.1:9000/verify", &loopback_issuer).is_ok());
    assert!(validate_verification_uri("http://127.0.0.1:9001/verify", &loopback_issuer).is_err());
}

#[test]
fn issuer_and_client_defaults_are_safe_and_loopback_only_http_is_allowed() {
    assert!(parse_issuer("https://login.example").is_ok());
    assert!(parse_issuer("http://127.0.0.1:9000").is_ok());
    assert!(parse_issuer("http://login.example").is_err());
    assert!(parse_issuer("https://user:pw@login.example").is_err());
}

#[test]
fn issuer_identity_preserves_the_exact_configured_string() {
    for configured in [
        "https://login.example",
        "https://login.example/",
        "https://LOGIN.example:443/",
    ] {
        assert_eq!(parse_issuer(configured).unwrap().identifier, configured);
    }
    assert!(parse_issuer(" https://login.example").is_err());
}

fn test_client(address: &Url) -> DeviceClient {
    DeviceClient {
        http: Client::builder()
            .redirect(Policy::none())
            .https_only(false)
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap(),
        issuer: address.clone(),
        issuer_id: address.origin().ascii_serialization(),
        client_id: "forge-cli".into(),
    }
}

fn authorization_body(verification_uri: &str) -> String {
    format!(
        r#"{{"device_code":"device+code","user_code":"ABCD-EFGH","verification_uri":"{verification_uri}","expires_in":60,"interval":1}}"#
    )
}

#[derive(Debug)]
struct Request {
    headers: String,
    body: String,
}

fn spawn_server(responses: Vec<(u16, String)>) -> (Url, thread::JoinHandle<Vec<Request>>) {
    spawn_server_with_header(responses, None)
}

fn spawn_redirect_server() -> (Url, thread::JoinHandle<Vec<Request>>) {
    spawn_server_with_header(
        vec![(302, String::new())],
        Some("Location: http://127.0.0.1:1/"),
    )
}

fn spawn_server_with_header(
    responses: Vec<(u16, String)>,
    extra_header: Option<&'static str>,
) -> (Url, thread::JoinHandle<Vec<Request>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let origin = format!("http://{address}");
        responses
            .into_iter()
            .map(|(status, body)| {
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_request(&mut stream);
                let body = body.replace("__ISSUER_ORIGIN__", &origin);
                let reason = if status == 200 { "OK" } else { "Found" };
                let extra = extra_header.map_or(String::new(), |header| format!("{header}\r\n"));
                write!(
                    stream,
                    "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n{body}",
                    body.len()
                )
                .unwrap();
                request
            })
            .collect()
    });
    (Url::parse(&format!("http://{address}/")).unwrap(), server)
}

fn read_request(stream: &mut TcpStream) -> Request {
    let mut reader = BufReader::new(stream);
    let mut headers = String::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" || line.is_empty() {
            break;
        }
        headers.push_str(&line);
    }
    let length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().unwrap())
        })
        .unwrap_or(0);
    let mut body = vec![0; length];
    reader.read_exact(&mut body).unwrap();
    Request {
        headers,
        body: String::from_utf8(body).unwrap(),
    }
}

fn parse_form(body: &str) -> std::collections::HashMap<String, String> {
    url::form_urlencoded::parse(body.as_bytes())
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect()
}

fn assert_no_oauth_client_secret_or_authorization(request: &Request) {
    let headers = request.headers.to_ascii_lowercase();
    assert!(!headers.contains("authorization:"));
    assert!(!request.body.contains("client_secret"));
    assert!(!request.body.contains("authorization"));
}

#[test]
fn snaplink_profile_fixture_matches_the_cli_device_client() {
    const PROFILE: &str =
        include_str!("../../../../docs/contracts/fixtures/forge-snaplink-profile-v1.json");
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Profile {
        schema_version: String,
        evaluation_mode: String,
        issuer: String,
        audience: String,
        resource: String,
        conversation_scopes: Vec<String>,
        device_observation_scopes: Vec<String>,
        clients: std::collections::BTreeMap<String, ClientProfile>,
        optional_clients: std::collections::BTreeMap<String, OptionalClientProfile>,
        authority: Authority,
    }
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ClientProfile {
        client_id: String,
        public: bool,
        grant_types: Vec<String>,
        scopes: Vec<String>,
    }
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct OptionalClientProfile {
        client_id: String,
        enabled: bool,
        public: bool,
        grant_types: Vec<String>,
        scopes: Vec<String>,
    }
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Authority {
        issuer_verified: bool,
        audience_verified: bool,
        client_provisioned: bool,
        token_issued: bool,
        device_authorized: bool,
        conversation_access_granted: bool,
    }

    let profile: Profile = serde_json::from_str(PROFILE).expect("strict profile fixture");
    assert_eq!(profile.schema_version, "forge.snaplink-profile/v1");
    assert_eq!(profile.evaluation_mode, "configuration_parity_only");
    assert_eq!(profile.issuer, "https://id.example");
    assert_eq!(profile.audience, "forge-api");
    assert_eq!(profile.resource, RESOURCE);
    assert_eq!(
        profile.conversation_scopes,
        vec![
            "forge:conversations:read".to_owned(),
            "forge:conversations:write".to_owned()
        ]
    );
    assert_eq!(
        profile.device_observation_scopes,
        vec![DEVICE_OBSERVATION_SCOPE.to_owned()]
    );
    let cli = profile.clients.get("cli").expect("cli profile");
    assert_eq!(cli.client_id, DEFAULT_CLIENT_ID);
    assert!(cli.public);
    assert_eq!(
        cli.grant_types,
        vec![DEVICE_GRANT.to_owned(), "refresh_token".to_owned()]
    );
    assert_eq!(cli.scopes.join(" "), SCOPES);
    let console = profile.clients.get("console").expect("console profile");
    assert_eq!(console.client_id, "forge-console");
    assert!(console.public);
    assert_eq!(
        console.grant_types,
        vec!["authorization_code", "refresh_token"]
    );
    assert_eq!(console.scopes, cli.scopes);
    let observer = profile
        .optional_clients
        .get("device_observer")
        .expect("device observation profile");
    assert_eq!(observer.client_id, DEVICE_OBSERVATION_CLIENT_ID);
    assert!(!observer.enabled);
    assert!(observer.public);
    assert_eq!(
        observer.grant_types,
        vec!["authorization_code", "refresh_token"]
    );
    assert_eq!(
        observer.scopes,
        vec![
            "forge:conversations:read".to_owned(),
            DEVICE_OBSERVATION_SCOPE.to_owned()
        ]
    );
    assert!(!profile.authority.issuer_verified);
    assert!(!profile.authority.audience_verified);
    assert!(!profile.authority.client_provisioned);
    assert!(!profile.authority.token_issued);
    assert!(!profile.authority.device_authorized);
    assert!(!profile.authority.conversation_access_granted);
}
