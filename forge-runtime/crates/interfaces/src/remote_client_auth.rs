use std::env;

use reqwest::Url;

use super::credentials::{ChangeCursorStore, CredentialStore, OwnerSelector, StoredCredential};
use super::{RemoteError, is_loopback_host};

pub(super) fn access_token_from_env(
    coordinator: &str,
) -> Result<(String, Option<ChangeCursorStore>), RemoteError> {
    let explicit = match env::var("FORGE_ACCESS_TOKEN") {
        Ok(value) => Some(value),
        Err(env::VarError::NotPresent) => None,
        Err(_) => {
            return Err(RemoteError(
                "FORGE_ACCESS_TOKEN is not valid Unicode".into(),
            ));
        }
    };
    if let Some(token) = explicit {
        return Ok((validate_access_token(token)?, None));
    }
    let (credential, store) = load_saved_access_credential()?;
    let cursor = store.change_cursor_store(coordinator, &credential);
    Ok((credential.access_token, Some(cursor)))
}

fn load_saved_access_credential() -> Result<(StoredCredential, CredentialStore), RemoteError> {
    let issuer = env::var("SNAPLINK_ISSUER_URL")
        .map_err(|_| RemoteError("FORGE_ACCESS_TOKEN or SNAPLINK_ISSUER_URL is required".into()))?;
    let client_id = env::var("SNAPLINK_CLIENT_ID").unwrap_or_else(|_| "forge-cli".to_owned());
    let subject = env::var("SNAPLINK_SUBJECT").ok();
    let tenant_id = env::var("SNAPLINK_TENANT_ID").ok();
    let issuer = validated_issuer(&issuer)?;
    let store = CredentialStore::from_env().map_err(RemoteError)?;
    let credential = store
        .load(
            &issuer,
            &client_id,
            OwnerSelector {
                subject: subject.as_deref(),
                tenant_id: tenant_id.as_deref(),
            },
        )
        .map_err(RemoteError)?;
    Ok((credential, store))
}

fn validate_access_token(token: String) -> Result<String, RemoteError> {
    if token.trim() != token
        || token.is_empty()
        || token.len() > 8192
        || !token.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
    {
        return Err(RemoteError("access token has an invalid format".into()));
    }
    Ok(token)
}

pub fn validated_issuer(value: &str) -> Result<String, RemoteError> {
    if value.trim() != value {
        return Err(RemoteError("SNAPLINK_ISSUER_URL is invalid".into()));
    }
    let url =
        Url::parse(value).map_err(|_| RemoteError("SNAPLINK_ISSUER_URL is invalid".into()))?;
    let local_http = url.scheme() == "http" && is_loopback_host(&url);
    if (url.scheme() != "https" && !local_http)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return Err(RemoteError(
            "SNAPLINK_ISSUER_URL must be an HTTPS origin (HTTP is allowed on loopback)".into(),
        ));
    }
    // Snaplink issuer comparison is exact; URL normalization merges identifiers.
    Ok(value.to_owned())
}

#[cfg(test)]
pub(super) fn resolve_access_token(
    explicit: Option<String>,
    load_saved: impl FnOnce() -> Result<String, RemoteError>,
) -> Result<String, RemoteError> {
    explicit.map_or_else(load_saved, validate_access_token)
}
