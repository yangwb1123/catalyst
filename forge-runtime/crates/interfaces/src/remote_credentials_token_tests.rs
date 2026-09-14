use std::time::SystemTime;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

use super::{claim_contains_scope, credential_from_token, credential_key};

#[test]
fn credential_partition_key_binds_issuer_client_tenant_and_subject() {
    let expected = credential_key("https://id.example", "forge-cli", "tenant-a", "user-a");
    for (issuer, client, tenant, subject) in [
        ("https://other.example", "forge-cli", "tenant-a", "user-a"),
        ("https://id.example", "other-client", "tenant-a", "user-a"),
        ("https://id.example", "forge-cli", "tenant-b", "user-a"),
        ("https://id.example", "forge-cli", "tenant-a", "user-b"),
    ] {
        assert_ne!(expected, credential_key(issuer, client, tenant, subject));
    }
}

#[test]
fn token_identity_is_bound_to_the_configured_issuer_and_complete_owner() {
    let issuer = "https://id.example";
    let token = jwt(issuer, "user-a", "tenant-a");
    let credential = credential_from_token(issuer, "forge-cli", token, future_time()).unwrap();
    assert_eq!(credential.subject, "user-a");
    assert_eq!(credential.tenant_id, "tenant-a");
    assert!(
        credential_from_token(
            "https://other.example",
            "forge-cli",
            credential.access_token,
            future_time(),
        )
        .is_err()
    );
}

#[test]
fn token_storage_requires_forge_audience_both_scopes_and_the_expected_client() {
    let issuer = "https://id.example";
    let expiration = future_time();
    for token in [
        jwt_with(
            issuer,
            "user-a",
            "tenant-a",
            "other-api",
            "forge:conversations:read forge:conversations:write",
        ),
        jwt_with(
            issuer,
            "user-a",
            "tenant-a",
            "forge-api",
            "forge:conversations:read",
        ),
    ] {
        assert!(credential_from_token(issuer, "forge-cli", token, expiration).is_err());
    }
    assert!(
        credential_from_token(
            issuer,
            "different-cli",
            jwt(issuer, "user-a", "tenant-a"),
            expiration,
        )
        .is_err()
    );
}

#[test]
fn accepts_snaplink_scopes_array_and_oauth_scope_string_compatibility() {
    let expected = "forge:conversations:read";
    assert!(claim_contains_scope(
        &serde_json::json!({"scopes": [expected]}),
        expected,
    ));
    assert!(claim_contains_scope(
        &serde_json::json!({"scope": "openid forge:conversations:read"}),
        expected,
    ));
    assert!(!claim_contains_scope(
        &serde_json::json!({"scopes": "forge:conversations:read", "scope": expected}),
        expected,
    ));
}

fn jwt(issuer: &str, subject: &str, tenant: &str) -> String {
    jwt_with(
        issuer,
        subject,
        tenant,
        "forge-api",
        "forge:conversations:read forge:conversations:write",
    )
}

fn jwt_with(issuer: &str, subject: &str, tenant: &str, audience: &str, scopes: &str) -> String {
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
    let payload = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&serde_json::json!({
            "iss": issuer,
            "sub": subject,
            "tenant_id": tenant,
            "client_id": "forge-cli",
            "aud": audience,
            "scopes": scopes.split_ascii_whitespace().collect::<Vec<_>>(),
            "exp": future_time(),
        }))
        .unwrap(),
    );
    format!("{header}.{payload}.inert-signature")
}

fn future_time() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3600
}
