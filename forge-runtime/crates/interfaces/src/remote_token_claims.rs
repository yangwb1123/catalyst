use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::unix_time;

const FORGE_AUDIENCE: &str = "forge-api";
const REQUIRED_SCOPES: [&str; 2] = ["forge:conversations:read", "forge:conversations:write"];

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::remote_command) struct StoredCredential {
    pub issuer: String,
    pub client_id: String,
    pub subject: String,
    pub tenant_id: String,
    pub access_token: String,
    pub expires_at_unix: u64,
}

#[derive(Clone, Copy)]
pub(in crate::remote_command) struct OwnerSelector<'a> {
    pub subject: Option<&'a str>,
    pub tenant_id: Option<&'a str>,
}

pub(in crate::remote_command) fn credential_from_token(
    issuer: &str,
    client_id: &str,
    access_token: String,
    expires_at_unix: u64,
) -> Result<StoredCredential, String> {
    let claims = decode_access_token_claims(&access_token)?;
    let owner = token_owner(&claims, issuer, client_id)?;
    validate_forge_token(&claims)?;
    let expires_at_unix = expires_at_unix.min(owner.expires_at);
    if expires_at_unix <= unix_time()? {
        return Err("Snaplink access token is already expired".into());
    }
    let credential = StoredCredential {
        issuer: issuer.to_owned(),
        client_id: client_id.to_owned(),
        subject: owner.subject,
        tenant_id: owner.tenant_id,
        access_token,
        expires_at_unix,
    };
    validate_credential(&credential)?;
    Ok(credential)
}

struct TokenOwner {
    subject: String,
    tenant_id: String,
    expires_at: u64,
}

fn decode_access_token_claims(token: &str) -> Result<serde_json::Value, String> {
    if token.is_empty()
        || token.len() > 8192
        || token.trim() != token
        || !token.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
    {
        return Err("Snaplink returned an invalid access token".into());
    }
    let mut parts = token.split('.');
    let (Some(_header), Some(payload), Some(_signature), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err("Snaplink access token is not a supported JWT".into());
    };
    if payload.len() > 16 * 1024 {
        return Err("Snaplink access token claims exceed the size limit".into());
    }
    let payload = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| "Snaplink access token claims are invalid".to_owned())?;
    serde_json::from_slice(&payload)
        .map_err(|_| "Snaplink access token claims are invalid".to_owned())
}

fn token_owner(
    claims: &serde_json::Value,
    issuer: &str,
    client_id: &str,
) -> Result<TokenOwner, String> {
    let token_issuer = claim_string(claims, "iss")?;
    let token_client = claim_string(claims, "client_id")?;
    let subject = claim_string(claims, "sub")?;
    let tenant_id = claim_string(claims, "tenant_id")?;
    if token_issuer != issuer {
        return Err("Snaplink access token issuer does not match SNAPLINK_ISSUER_URL".into());
    }
    if token_client != client_id {
        return Err("Snaplink access token client does not match SNAPLINK_CLIENT_ID".into());
    }
    if subject.len() > 255 || tenant_id.len() > 256 {
        return Err("Snaplink access token owner claims exceed the size limit".into());
    }
    let expires_at = claims
        .get("exp")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "Snaplink access token is missing a valid exp claim".to_owned())?;
    Ok(TokenOwner {
        subject: subject.to_owned(),
        tenant_id: tenant_id.to_owned(),
        expires_at,
    })
}

fn validate_forge_token(claims: &serde_json::Value) -> Result<(), String> {
    if !claim_contains(claims, "aud", FORGE_AUDIENCE) {
        return Err("Snaplink access token does not include the forge-api audience".into());
    }
    if !REQUIRED_SCOPES
        .iter()
        .all(|scope| claim_contains_scope(claims, scope))
    {
        return Err("Snaplink access token is missing required Forge conversation scopes".into());
    }
    Ok(())
}

fn claim_string<'a>(claims: &'a serde_json::Value, name: &str) -> Result<&'a str, String> {
    claims
        .get(name)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty() && !value.chars().any(char::is_control))
        .ok_or_else(|| format!("Snaplink access token is missing a valid {name} claim"))
}

fn claim_contains(claims: &serde_json::Value, name: &str, expected: &str) -> bool {
    match claims.get(name) {
        Some(serde_json::Value::String(value)) => value == expected,
        Some(serde_json::Value::Array(values)) => {
            values.iter().any(|value| value.as_str() == Some(expected))
        }
        _ => false,
    }
}

fn claim_contains_scope(claims: &serde_json::Value, expected: &str) -> bool {
    if let Some(scopes) = claims.get("scopes") {
        return scopes
            .as_array()
            .is_some_and(|values| values.iter().any(|scope| scope.as_str() == Some(expected)));
    }
    claims
        .get("scope")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|scopes| {
            scopes
                .split_ascii_whitespace()
                .any(|scope| scope == expected)
        })
}

pub(super) fn validate_credential(credential: &StoredCredential) -> Result<(), String> {
    if credential.issuer.is_empty()
        || credential.client_id.is_empty()
        || credential.client_id.len() > 128
        || !credential
            .client_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        || credential.subject.is_empty()
        || credential.tenant_id.is_empty()
        || credential.expires_at_unix == 0
    {
        return Err("Forge credential binding is incomplete".into());
    }
    if credential.access_token.is_empty()
        || credential.access_token.len() > 8192
        || credential.access_token.trim() != credential.access_token
        || credential
            .access_token
            .bytes()
            .any(|byte| !(0x21..=0x7e).contains(&byte))
    {
        return Err("Forge credential token has an invalid format".into());
    }
    Ok(())
}

pub(super) fn credential_key(issuer: &str, client: &str, tenant: &str, subject: &str) -> String {
    let mut digest = Sha256::new();
    for field in [issuer, client, tenant, subject] {
        digest.update(u64::try_from(field.len()).unwrap_or(u64::MAX).to_be_bytes());
        digest.update(field.as_bytes());
    }
    format!("{:x}", digest.finalize())
}

#[cfg(test)]
#[path = "remote_credentials_token_tests.rs"]
mod tests;
