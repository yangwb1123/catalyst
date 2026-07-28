mod output_items;
mod output_semantics;
#[cfg(test)]
#[path = "tests/phase_stream.rs"]
mod phase_stream_tests;
#[cfg(test)]
#[path = "tests/protocol.rs"]
mod protocol_tests;
#[cfg(test)]
#[path = "tests/reasoning_loop.rs"]
mod reasoning_loop_tests;
mod redaction;
mod request;
#[cfg(test)]
#[path = "tests/response_fixtures.rs"]
mod response_fixtures;
#[cfg(test)]
#[path = "tests/secret_redaction.rs"]
mod secret_redaction_tests;
mod sse;
mod sse_wire;
#[cfg(test)]
#[path = "tests/terminal_semantics.rs"]
mod terminal_semantics_tests;
#[cfg(test)]
#[path = "tests/transport.rs"]
mod tests;

use std::time::Duration;

#[cfg(test)]
use std::net::IpAddr;

use crate::runtime_domain::{
    Cancellation, ModelEvent, ModelEventStream, ModelProvider, ModelRequest, ProviderError,
};
use futures_util::{StreamExt, stream};
use reqwest::{Client, Response, Url, header};

use self::{request::encode_request, sse::SseDecoder};

const ERROR_BODY_LIMIT: usize = 64 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(600);
const READ_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone, Copy)]
enum EndpointPolicy {
    Official,
    #[cfg(test)]
    TestLoopback,
}

#[derive(Clone)]
pub struct OpenAiResponsesProvider {
    client: Client,
    endpoint: Url,
    model: String,
    api_key: String,
}

impl OpenAiResponsesProvider {
    /// Constructs a provider without performing a network request.
    ///
    /// # Errors
    ///
    /// Returns an error when the model or API key is empty, the base URL is
    /// not the official `https://api.openai.com/v1` origin and path, or the
    /// bounded HTTP client cannot be constructed.
    pub fn new(
        base_url: impl AsRef<str>,
        model: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Result<Self, ProviderError> {
        Self::build(
            base_url.as_ref(),
            model.into(),
            api_key.into(),
            EndpointPolicy::Official,
        )
    }

    #[cfg(test)]
    fn new_insecure_for_test(
        base_url: impl AsRef<str>,
        model: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Result<Self, ProviderError> {
        Self::build(
            base_url.as_ref(),
            model.into(),
            api_key.into(),
            EndpointPolicy::TestLoopback,
        )
    }

    fn build(
        base_url: &str,
        model: String,
        api_key: String,
        endpoint_policy: EndpointPolicy,
    ) -> Result<Self, ProviderError> {
        validate_non_empty("model", &model)?;
        validate_non_empty("api_key", &api_key)?;
        Ok(Self {
            client: build_client(endpoint_policy)?,
            endpoint: responses_endpoint(base_url, endpoint_policy)?,
            model,
            api_key,
        })
    }
}

impl ModelProvider for OpenAiResponsesProvider {
    fn stream(&self, request: ModelRequest) -> ModelEventStream {
        response_stream(
            self.client.clone(),
            self.endpoint.clone(),
            self.model.clone(),
            self.api_key.clone(),
            request,
        )
    }
}

fn response_stream(
    client: Client,
    endpoint: Url,
    model: String,
    api_key: String,
    request: ModelRequest,
) -> ModelEventStream {
    let opening = stream::once(async move {
        let cancellation = request.cancellation.clone();
        send_request(&client, endpoint, &model, &api_key, &request)
            .await
            .map(|response| (response, cancellation, api_key))
    });
    Box::pin(opening.flat_map(|result| match result {
        Ok((response, cancellation, api_key)) => decode_response(response, cancellation, api_key),
        Err(error) => Box::pin(stream::once(async move { Err(error) })),
    }))
}

fn decode_response(
    mut response: Response,
    cancellation: Cancellation,
    api_key: String,
) -> ModelEventStream {
    Box::pin(async_stream::stream! {
        let mut decoder = SseDecoder::new(&api_key);
        loop {
            let next = tokio::select! {
                () = cancellation.cancelled() => Err(cancelled_error()),
                chunk = response.chunk() => {
                    chunk.map_err(|_| transport_error(!decoder.has_decoded_event()))
                },
            };
            let bytes = match next {
                Ok(Some(bytes)) => bytes,
                Ok(None) => {
                    let (events, error) = finish_decoder(&mut decoder);
                    for event in events {
                        yield Ok(event);
                    }
                    if let Some(error) = error {
                        yield Err(error);
                    }
                    return;
                }
                Err(error) => {
                    yield Err(error);
                    return;
                }
            };
            match decoder.push(&bytes) {
                Ok(events) => {
                    for event in events {
                        yield Ok(event);
                    }
                    if decoder.is_terminal() {
                        return;
                    }
                }
                Err(error) => {
                    yield Err(error);
                    return;
                }
            }
        }
    })
}

fn finish_decoder(decoder: &mut SseDecoder) -> (Vec<ModelEvent>, Option<ProviderError>) {
    match decoder.finish() {
        Err(error) => (Vec::new(), Some(error)),
        Ok(events) => {
            let error =
                (!decoder.is_terminal()).then(|| stream_ended_error(!decoder.has_decoded_event()));
            (events, error)
        }
    }
}

async fn send_request(
    client: &Client,
    endpoint: Url,
    model: &str,
    api_key: &str,
    request: &ModelRequest,
) -> Result<Response, ProviderError> {
    if request.cancellation.is_cancelled() {
        return Err(cancelled_error());
    }
    let body = encode_request(model, request)?;
    let pending = client
        .post(endpoint)
        .bearer_auth(api_key)
        .header(header::ACCEPT, "text/event-stream")
        .json(&body)
        .send();
    let response = tokio::select! {
        () = request.cancellation.cancelled() => return Err(cancelled_error()),
        result = pending => result.map_err(|error| transport_error(error.is_connect() || error.is_timeout()))?,
    };
    if response.status().is_success() && has_event_stream_content_type(&response) {
        return Ok(response);
    }
    if response.status().is_success() {
        return Err(provider_protocol_error(
            "successful provider response was not text/event-stream",
        ));
    }
    Err(http_error(response, api_key).await)
}

fn has_event_stream_content_type(response: &Response) -> bool {
    let mut values = response.headers().get_all(header::CONTENT_TYPE).iter();
    let Some(value) = values.next() else {
        return false;
    };
    if values.next().is_some() {
        return false;
    }
    value
        .to_str()
        .ok()
        .is_some_and(is_event_stream_content_type)
}

fn is_event_stream_content_type(value: &str) -> bool {
    if value.contains(',') {
        return false;
    }
    let mut parts = value.split(';');
    if !parts
        .next()
        .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case("text/event-stream"))
    {
        return false;
    }
    let mut saw_charset = false;
    for parameter in parts {
        let Some((name, value)) = parameter.trim().split_once('=') else {
            return false;
        };
        if saw_charset || !name.trim().eq_ignore_ascii_case("charset") {
            return false;
        }
        let value = value.trim();
        let value = if let Some(quoted) = value.strip_prefix('"') {
            let Some(quoted) = quoted.strip_suffix('"') else {
                return false;
            };
            if quoted.contains('"') {
                return false;
            }
            quoted
        } else {
            if value.contains('"') {
                return false;
            }
            value
        };
        if !value.eq_ignore_ascii_case("utf-8") {
            return false;
        }
        saw_charset = true;
    }
    true
}

async fn http_error(response: Response, api_key: &str) -> ProviderError {
    let status = response.status();
    let body = bounded_body(response).await;
    let api_error = serde_json::from_slice::<ErrorEnvelope>(&body)
        .ok()
        .map(|envelope| envelope.error);
    let code = api_error
        .as_ref()
        .and_then(|error| error.code.clone())
        .unwrap_or_else(|| format!("http_{}", status.as_u16()));
    let code = redact(&code, api_key);
    let message = api_error.map_or_else(
        || format!("provider returned HTTP {}", status.as_u16()),
        |error| redact(&error.message, api_key),
    );
    ProviderError::new(
        code,
        message,
        status.as_u16() == 429 || status.is_server_error(),
    )
}

async fn bounded_body(mut response: Response) -> Vec<u8> {
    let mut body = Vec::new();
    while let Ok(Some(chunk)) = response.chunk().await {
        let remaining = ERROR_BODY_LIMIT.saturating_sub(body.len());
        body.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
        if body.len() == ERROR_BODY_LIMIT {
            break;
        }
    }
    body
}

#[derive(serde::Deserialize)]
struct ErrorEnvelope {
    error: ApiError,
}

#[derive(serde::Deserialize)]
struct ApiError {
    code: Option<String>,
    message: String,
}

fn responses_endpoint(
    base_url: &str,
    endpoint_policy: EndpointPolicy,
) -> Result<Url, ProviderError> {
    let mut url = Url::parse(base_url)
        .map_err(|_| config_error("base_url must be an absolute HTTP(S) URL"))?;
    if url.cannot_be_a_base()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(config_error(
            "base_url must not contain credentials, a query, or a fragment",
        ));
    }
    match endpoint_policy {
        EndpointPolicy::Official => validate_official_base_url(&url)?,
        #[cfg(test)]
        EndpointPolicy::TestLoopback => {
            if url.scheme() != "http"
                || !url.host_str().is_some_and(is_loopback_host)
                || !matches!(url.path(), "/v1" | "/v1/")
            {
                return Err(config_error(
                    "test base_url must be an HTTP loopback URL ending in /v1",
                ));
            }
        }
    }
    url.set_path("/v1/responses");
    Ok(url)
}

fn validate_official_base_url(url: &Url) -> Result<(), ProviderError> {
    if url.scheme() != "https"
        || url.host_str() != Some("api.openai.com")
        || url.port().is_some()
        || !matches!(url.path(), "/v1" | "/v1/")
    {
        return Err(config_error(
            "base_url must be exactly https://api.openai.com/v1",
        ));
    }
    Ok(())
}

#[cfg(test)]
fn is_loopback_host(host: &str) -> bool {
    host == "localhost"
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn build_client(endpoint_policy: EndpointPolicy) -> Result<Client, ProviderError> {
    Client::builder()
        .https_only(matches!(endpoint_policy, EndpointPolicy::Official))
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .build()
        .map_err(|_| config_error("failed to construct the HTTP client"))
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), ProviderError> {
    if value.trim().is_empty() {
        return Err(config_error(&format!("{field} must not be empty")));
    }
    Ok(())
}

fn redact(message: &str, api_key: &str) -> String {
    message.replace(api_key, "[REDACTED]")
}

fn config_error(message: &str) -> ProviderError {
    ProviderError::new("invalid_provider_config", message, false)
}

fn cancelled_error() -> ProviderError {
    ProviderError::new("cancelled", "model request was cancelled", false)
}

fn transport_error(retryable: bool) -> ProviderError {
    ProviderError::new(
        "transport_error",
        "OpenAI Responses transport failed",
        retryable,
    )
}

fn provider_protocol_error(message: &str) -> ProviderError {
    ProviderError::new("provider_protocol", message, false)
}

fn stream_ended_error(retryable: bool) -> ProviderError {
    ProviderError::new(
        "stream_ended",
        "provider stream ended without a terminal event",
        retryable,
    )
}
