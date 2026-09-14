use super::*;

#[test]
fn saved_credential_issuer_keeps_the_exact_snaplink_identifier() {
    for configured in [
        "https://id.example",
        "https://id.example/",
        "https://ID.example:443/",
    ] {
        assert_eq!(validated_issuer(configured).unwrap(), configured);
    }
    assert!(validated_issuer(" https://id.example").is_err());
}
#[test]
fn explicit_access_token_takes_priority_without_reading_saved_credentials() {
    let mut fallback_called = false;
    let token = resolve_access_token(Some("explicit-token".into()), || {
        fallback_called = true;
        Err(super::RemoteError("should not load credentials".into()))
    })
    .unwrap();
    assert_eq!(token, "explicit-token");
    assert!(!fallback_called);
}
#[test]
fn saved_credential_is_used_only_when_no_explicit_access_token_exists() {
    let token = resolve_access_token(None, || Ok("saved-token".into())).unwrap();
    assert_eq!(token, "saved-token");
}
#[test]
fn remote_api_url_requires_tls_except_for_loopback_development() {
    assert!(parse_api_url("https://forge.example").is_ok());
    assert!(parse_api_url("http://127.0.0.1:8080").is_ok());
    assert!(parse_api_url("http://[::1]:8080").is_ok());
    assert!(parse_api_url("http://localhost:8080").is_ok());
    assert!(parse_api_url("http://forge.example").is_err());
    assert!(parse_api_url("https://forge.example/api/v1").is_err());
    assert!(parse_api_url("https://user:secret@forge.example").is_err());
    assert!(parse_api_url("https://forge.example?debug=1").is_err());
}
