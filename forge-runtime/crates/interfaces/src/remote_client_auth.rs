use std::{
    env,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use reqwest::{Client, Url, redirect::Policy};

use super::credentials::{
    ChangeCursorStore, CredentialStore, OwnerSelector, StoredCredential, credential_from_token,
};
use super::login::{TokenReply, refresh_token_request};
use super::{RemoteError, is_loopback_host};

pub(super) fn access_token_from_env(
    coordinator: &str,
) -> Result<
    (
        String,
        Option<ChangeCursorStore>,
        Option<Arc<SavedTokenProvider>>,
    ),
    RemoteError,
> {
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
        return Ok((validate_access_token(token)?, None, None));
    }
    let (credential, store) = load_saved_access_credential()?;
    let cursor = store.change_cursor_store(coordinator, &credential);
    let provider = SavedTokenProvider::new(credential.clone(), store)?;
    Ok((
        credential.access_token,
        Some(cursor),
        Some(Arc::new(provider)),
    ))
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

pub(super) struct SavedTokenProvider {
    credential: tokio::sync::Mutex<StoredCredential>,
    store: CredentialStore,
    issuer: Url,
    client_id: String,
    http: Client,
}

impl SavedTokenProvider {
    fn new(credential: StoredCredential, store: CredentialStore) -> Result<Self, RemoteError> {
        let issuer = Url::parse(&credential.issuer)
            .map_err(|_| RemoteError("SNAPLINK_ISSUER_URL is invalid".into()))?;
        let http = Client::builder()
            .redirect(Policy::none())
            .https_only(issuer.scheme() == "https")
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|_| RemoteError("could not configure the Snaplink OAuth client".into()))?;
        Ok(Self {
            client_id: credential.client_id.clone(),
            credential: tokio::sync::Mutex::new(credential),
            store,
            issuer,
            http,
        })
    }

    pub(super) async fn access_token(&self) -> Result<String, RemoteError> {
        let mut credential = self.credential.lock().await;
        let now = unix_now()?;
        if credential.expires_at_unix > now.saturating_add(60) {
            return Ok(credential.access_token.clone());
        }
        let store = self.store.clone();
        let binding = credential.clone();
        let refresh_token = tokio::task::spawn_blocking(move || store.load_refresh_token(&binding))
            .await
            .map_err(|_| RemoteError("OS credential-store operation failed".into()))?
            .map_err(RemoteError)?;
        let Some(refresh_token) = refresh_token else {
            if credential.expires_at_unix > now {
                return Ok(credential.access_token.clone());
            }
            return Err(RemoteError(
                "saved Forge access token expired and no secure refresh token is available; run remote login".into(),
            ));
        };
        let reply =
            refresh_token_request(&self.http, &self.issuer, &self.client_id, &refresh_token)
                .await
                .map_err(RemoteError)?;
        self.apply_refresh_reply(&mut credential, reply).await
    }

    async fn apply_refresh_reply(
        &self,
        current: &mut StoredCredential,
        reply: TokenReply,
    ) -> Result<String, RemoteError> {
        if reply.error.as_deref() == Some("invalid_grant") {
            let store = self.store.clone();
            let binding = current.clone();
            tokio::task::spawn_blocking(move || store.clear_refresh_token(&binding))
                .await
                .map_err(|_| RemoteError("OS credential-store operation failed".into()))?
                .map_err(RemoteError)?;
            return Err(RemoteError(
                "Snaplink refresh credential expired or was revoked; run remote login".into(),
            ));
        }
        let (Some(access_token), Some(token_type), Some(expires_in), Some(refresh_token)) = (
            reply.access_token,
            reply.token_type,
            reply.expires_in,
            reply.refresh_token,
        ) else {
            return Err(RemoteError(
                "Snaplink returned an invalid refresh response".into(),
            ));
        };
        if !token_type.eq_ignore_ascii_case("bearer") || expires_in == 0 || expires_in > 31_536_000
        {
            return Err(RemoteError(
                "Snaplink returned an invalid refresh response".into(),
            ));
        }
        let expires_at = unix_now()?
            .checked_add(expires_in)
            .ok_or_else(|| RemoteError("Snaplink token expiry is invalid".into()))?;
        let next = credential_from_token(
            &current.issuer,
            &current.client_id,
            access_token,
            expires_at,
        )
        .map_err(RemoteError)?;
        if next.subject != current.subject || next.tenant_id != current.tenant_id {
            return Err(RemoteError(
                "Snaplink refresh token changed the authenticated Forge owner".into(),
            ));
        }

        let store = self.store.clone();
        let binding = next.clone();
        let persisted_refresh = refresh_token.clone();
        tokio::task::spawn_blocking(move || store.save_refresh_token(&binding, &persisted_refresh))
            .await
            .map_err(|_| RemoteError("OS credential-store operation failed".into()))?
            .map_err(RemoteError)?;

        let store = self.store.clone();
        let saved = next.clone();
        tokio::task::spawn_blocking(move || store.save(&saved))
            .await
            .map_err(|_| RemoteError("credential-file operation failed".into()))?
            .map_err(RemoteError)?;
        *current = next.clone();
        Ok(next.access_token)
    }
}

fn unix_now() -> Result<u64, RemoteError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| RemoteError("system clock is before the Unix epoch".into()))
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
