use std::{env, error::Error, fmt, time::Duration};

use reqwest::{Client, Response, Url, redirect::Policy};
use serde::Deserialize;

use super::credentials::{CredentialStore, credential_from_token};

const DEFAULT_CLIENT_ID: &str = "forge-cli";
const RESOURCE: &str = "forge-api";
const SCOPES: &str = "forge:conversations:read forge:conversations:write";
const MAX_BODY_BYTES: usize = 64 * 1024;
const MAX_URL_BYTES: usize = 2048;
const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";

#[derive(Debug)]
struct LoginError(String);

impl fmt::Display for LoginError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for LoginError {}

struct IssuerConfig {
    endpoint: Url,
    identifier: String,
}

struct DeviceClient {
    http: Client,
    issuer: Url,
    issuer_id: String,
    client_id: String,
}

#[derive(Deserialize)]
struct DeviceAuthorization {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    #[serde(default = "default_interval")]
    interval: u64,
}

#[derive(Deserialize)]
pub(super) struct TokenReply {
    pub(super) access_token: Option<String>,
    pub(super) token_type: Option<String>,
    pub(super) expires_in: Option<u64>,
    pub(super) refresh_token: Option<String>,
    pub(super) error: Option<String>,
}

pub(super) async fn run() -> Result<(), Box<dyn Error>> {
    let client = DeviceClient::from_env()?;
    let authorization = client.request_device_code().await?;
    println!(
        "Open {} and enter code {}",
        authorization.verification_uri, authorization.user_code
    );
    let token = client.poll_until_approved(&authorization).await?;
    let expires_at = unix_now()?
        .checked_add(token.expires_in)
        .ok_or_else(|| LoginError("Snaplink token expiry is invalid".into()))?;
    let credential = credential_from_token(
        &client.issuer_id,
        &client.client_id,
        token.access_token,
        expires_at,
    )
    .map_err(LoginError)?;
    let store = CredentialStore::from_env().map_err(LoginError)?;
    tokio::task::spawn_blocking(move || {
        store.save_login(&credential, token.refresh_token.as_deref())
    })
    .await
    .map_err(|_| LoginError("OS credential-store operation failed".into()))?
    .map_err(LoginError)?;
    println!(
        "Forge CLI login completed; credentials were stored in the available protected stores."
    );
    Ok(())
}

impl DeviceClient {
    fn from_env() -> Result<Self, LoginError> {
        let raw_issuer = env::var("SNAPLINK_ISSUER_URL")
            .map_err(|_| LoginError("SNAPLINK_ISSUER_URL is required for remote login".into()))?;
        let issuer = parse_issuer(&raw_issuer)?;
        let client_id = match env::var("SNAPLINK_CLIENT_ID") {
            Ok(value) => value,
            Err(env::VarError::NotPresent) => DEFAULT_CLIENT_ID.into(),
            Err(_) => return Err(LoginError("SNAPLINK_CLIENT_ID is not valid Unicode".into())),
        };
        validate_client_id(&client_id)?;
        let http = Client::builder()
            .redirect(Policy::none())
            .https_only(issuer.endpoint.scheme() == "https")
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|_| LoginError("could not configure the Snaplink OAuth client".into()))?;
        Ok(Self {
            http,
            issuer_id: issuer.identifier,
            issuer: issuer.endpoint,
            client_id,
        })
    }

    async fn request_device_code(&self) -> Result<DeviceAuthorization, LoginError> {
        let endpoint = self.endpoint("/device/code")?;
        let response = self
            .http
            .post(endpoint)
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("scope", SCOPES),
                ("resource", RESOURCE),
            ])
            .send()
            .await
            .map_err(|_| LoginError("Snaplink device authorization request failed".into()))?;
        let authorization: DeviceAuthorization = decode_success(response).await?;
        if authorization.device_code.is_empty()
            || authorization.device_code.len() > 8192
            || authorization.user_code.trim().is_empty()
            || authorization.user_code.len() > 128
            || authorization.user_code.chars().any(char::is_control)
            || authorization.expires_in == 0
            || authorization.expires_in > 3600
            || authorization.interval > 3600
        {
            return Err(LoginError(
                "Snaplink returned invalid device-flow parameters".into(),
            ));
        }
        let verification_uri =
            validate_verification_uri(&authorization.verification_uri, &self.issuer)?;
        Ok(DeviceAuthorization {
            verification_uri: verification_uri.to_string(),
            interval: if authorization.interval == 0 {
                default_interval()
            } else {
                authorization.interval
            },
            ..authorization
        })
    }

    async fn poll_until_approved(
        &self,
        authorization: &DeviceAuthorization,
    ) -> Result<ApprovedToken, LoginError> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(authorization.expires_in);
        let mut interval = authorization.interval;
        loop {
            tokio::time::sleep(Duration::from_secs(interval)).await;
            if tokio::time::Instant::now() >= deadline {
                return Err(LoginError("Snaplink device authorization expired".into()));
            }
            match self.poll_token(&authorization.device_code).await? {
                PollResult::Pending => {}
                PollResult::SlowDown => interval = increase_interval(interval),
                PollResult::Approved(token) => return Ok(token),
            }
        }
    }

    async fn poll_token(&self, device_code: &str) -> Result<PollResult, LoginError> {
        let response = self
            .http
            .post(self.endpoint("/token")?)
            .form(&[
                ("grant_type", DEVICE_GRANT),
                ("device_code", device_code),
                ("client_id", self.client_id.as_str()),
            ])
            .send()
            .await
            .map_err(|_| LoginError("Snaplink token poll failed".into()))?;
        interpret_token_reply(decode_token_reply(response).await?)
    }

    fn endpoint(&self, path: &str) -> Result<Url, LoginError> {
        self.issuer
            .join(path)
            .map_err(|_| LoginError("Snaplink OAuth endpoint is invalid".into()))
    }
}

struct ApprovedToken {
    access_token: String,
    expires_in: u64,
    refresh_token: Option<String>,
}

enum PollResult {
    Pending,
    SlowDown,
    Approved(ApprovedToken),
}

fn interpret_token_reply(reply: TokenReply) -> Result<PollResult, LoginError> {
    match reply {
        TokenReply {
            access_token: Some(access_token),
            token_type: Some(token_type),
            expires_in: Some(expires_in),
            error: None,
            refresh_token,
            ..
        } if token_type.eq_ignore_ascii_case("bearer") && expires_in > 0 => {
            Ok(PollResult::Approved(ApprovedToken {
                access_token,
                expires_in,
                refresh_token,
            }))
        }
        TokenReply {
            error: Some(error), ..
        } => match error.as_str() {
            "authorization_pending" => Ok(PollResult::Pending),
            "slow_down" => Ok(PollResult::SlowDown),
            "access_denied" => Err(LoginError(
                "Snaplink device authorization was denied".into(),
            )),
            "expired_token" => Err(LoginError("Snaplink device authorization expired".into())),
            _ => Err(LoginError("Snaplink token request was rejected".into())),
        },
        _ => Err(LoginError(
            "Snaplink returned an invalid token response".into(),
        )),
    }
}

async fn decode_success<T: for<'de> Deserialize<'de>>(response: Response) -> Result<T, LoginError> {
    if !response.status().is_success() {
        return Err(LoginError(
            "Snaplink device authorization was rejected".into(),
        ));
    }
    let bytes = bounded_body(response).await?;
    serde_json::from_slice(&bytes)
        .map_err(|_| LoginError("Snaplink returned an invalid device-flow response".into()))
}

pub(super) async fn refresh_token_request(
    http: &Client,
    issuer: &Url,
    client_id: &str,
    refresh_token: &str,
) -> Result<TokenReply, String> {
    let endpoint = issuer
        .join("/token")
        .map_err(|_| "Snaplink OAuth endpoint is invalid".to_owned())?;
    let response = http
        .post(endpoint)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", client_id),
        ])
        .send()
        .await
        .map_err(|_| "Snaplink refresh request failed".to_owned())?;
    decode_token_reply(response)
        .await
        .map_err(|error| error.to_string())
}

async fn decode_token_reply(response: Response) -> Result<TokenReply, LoginError> {
    let status_ok = response.status().is_success();
    let bytes = bounded_body(response).await?;
    let reply: TokenReply = serde_json::from_slice(&bytes)
        .map_err(|_| LoginError("Snaplink returned an invalid token response".into()))?;
    if !status_ok && reply.error.is_none() {
        return Err(LoginError("Snaplink token request was rejected".into()));
    }
    Ok(reply)
}

async fn bounded_body(mut response: Response) -> Result<Vec<u8>, LoginError> {
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| LoginError("Snaplink response could not be read".into()))?
    {
        if body.len().saturating_add(chunk.len()) > MAX_BODY_BYTES {
            return Err(LoginError(
                "Snaplink response exceeds the size limit".into(),
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn parse_issuer(value: &str) -> Result<IssuerConfig, LoginError> {
    if value.len() > MAX_URL_BYTES || value.trim() != value {
        return Err(LoginError(
            "SNAPLINK_ISSUER_URL is invalid or exceeds the size limit".into(),
        ));
    }
    let issuer =
        Url::parse(value).map_err(|_| LoginError("SNAPLINK_ISSUER_URL is invalid".into()))?;
    if !secure_url(&issuer)
        || !issuer.username().is_empty()
        || issuer.password().is_some()
        || issuer.query().is_some()
        || issuer.fragment().is_some()
        || !matches!(issuer.path(), "" | "/")
    {
        return Err(LoginError(
            "SNAPLINK_ISSUER_URL must be an HTTPS origin (HTTP is allowed on loopback)".into(),
        ));
    }
    Ok(IssuerConfig {
        endpoint: issuer,
        // JWT iss is an identifier string, not a URL-equivalence class.
        // Preserve the configured spelling for exact claim checks and lookup.
        identifier: value.to_owned(),
    })
}

fn validate_verification_uri(value: &str, issuer: &Url) -> Result<Url, LoginError> {
    if value.len() > MAX_URL_BYTES {
        return Err(LoginError(
            "Snaplink verification URI exceeds the size limit".into(),
        ));
    }
    let uri =
        Url::parse(value).map_err(|_| LoginError("Snaplink verification URI is invalid".into()))?;
    let secure_web_origin = uri.scheme() == "https";
    let loopback_development_origin = uri.scheme() == "http"
        && issuer.scheme() == "http"
        && loopback_host(&uri)
        && loopback_host(issuer)
        && uri.origin() == issuer.origin();
    if (!secure_web_origin && !loopback_development_origin)
        || !uri.username().is_empty()
        || uri.password().is_some()
    {
        return Err(LoginError(
            "Snaplink verification URI must use a secure origin without embedded credentials"
                .into(),
        ));
    }
    Ok(uri)
}

fn secure_url(url: &Url) -> bool {
    url.scheme() == "https" || (url.scheme() == "http" && loopback_host(url))
}

fn loopback_host(url: &Url) -> bool {
    url.host_str().is_some_and(|host| {
        let host = host.trim_start_matches('[').trim_end_matches(']');
        host.eq_ignore_ascii_case("localhost")
            || host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|address| address.is_loopback())
    })
}

fn validate_client_id(value: &str) -> Result<(), LoginError> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(LoginError("SNAPLINK_CLIENT_ID is invalid".into()));
    }
    Ok(())
}

fn default_interval() -> u64 {
    5
}

fn increase_interval(interval: u64) -> u64 {
    interval.saturating_add(5)
}

fn unix_now() -> Result<u64, LoginError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| LoginError("system clock is before the Unix epoch".into()))
}

#[cfg(test)]
#[path = "remote_login_tests.rs"]
mod tests;
