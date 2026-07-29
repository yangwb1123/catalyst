mod endpoint;
mod output_items;
mod output_semantics;
#[cfg(test)]
#[path = "tests/phase_stream.rs"]
mod phase_stream_tests;
#[cfg(test)]
#[path = "tests/prepared_request.rs"]
mod prepared_request_tests;
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

use crate::runtime_domain::{
    Cancellation, ModelEvent, ModelEventStream, ModelProvider, ModelRequest, PreparedModelProvider,
    PreparedModelRequest, ProviderError,
};
use futures_util::{StreamExt, stream};
use reqwest::{Client, Response, Url, header};

use self::{
    endpoint::{build_client, responses_endpoint},
    request::{
        encode_request_bytes as encode_canonical_request_bytes,
        validate_exact_request_bytes as validate_exact_canonical_request_bytes,
        validate_request_bytes,
    },
    sse::SseDecoder,
};

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
        validate_api_key(&api_key)?;
        Ok(Self {
            client: build_client(endpoint_policy)?,
            endpoint: responses_endpoint(base_url, endpoint_policy)?,
            model,
            api_key,
        })
    }

    /// Encodes the exact canonical request-body bytes for the configured model.
    ///
    /// This pure adapter operation requires neither a provider instance nor an
    /// API key and performs no network I/O.
    ///
    /// # Errors
    ///
    /// Returns an error when the model is empty, provider context violates the
    /// Responses protocol, or canonical JSON encoding fails.
    pub fn encode_request_bytes(
        model: &str,
        request: &ModelRequest,
    ) -> Result<Vec<u8>, ProviderError> {
        validate_non_empty("model", model)?;
        encode_canonical_request_bytes(model, request)
    }

    /// Verifies persisted bytes against an exact expected logical request.
    ///
    /// The expected request is encoded again through the same canonical path
    /// used for dispatch, then compared byte-for-byte with `actual_bytes`.
    /// This pure adapter operation requires neither a provider instance nor an
    /// API key and performs no network I/O.
    ///
    /// # Errors
    ///
    /// Returns an error when the model or expected request is invalid, encoding
    /// fails, or any byte differs.
    pub fn validate_exact_request_bytes(
        model: &str,
        expected_request: &ModelRequest,
        actual_bytes: &[u8],
    ) -> Result<(), ProviderError> {
        validate_non_empty("model", model)?;
        validate_exact_canonical_request_bytes(model, expected_request, actual_bytes)
    }

    #[must_use]
    pub fn endpoint(&self) -> &str {
        self.endpoint.as_str()
    }

    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Encodes one request into the exact canonical bytes that will be sent.
    ///
    /// # Errors
    ///
    /// Returns an error when provider context violates the Responses protocol
    /// or the request cannot be encoded as canonical JSON.
    pub fn prepare_request(
        &self,
        request: ModelRequest,
    ) -> Result<PreparedModelRequest, ProviderError> {
        let body = Self::encode_request_bytes(&self.model, &request)?;
        Ok(PreparedModelRequest::new(body, request.cancellation))
    }
}

impl ModelProvider for OpenAiResponsesProvider {
    fn stream(&self, request: ModelRequest) -> ModelEventStream {
        match self.prepare_request(request) {
            Ok(prepared) => self.stream_prepared(prepared),
            Err(error) => error_stream(error),
        }
    }
}

impl PreparedModelProvider for OpenAiResponsesProvider {
    fn stream_prepared(&self, request: PreparedModelRequest) -> ModelEventStream {
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
    request: PreparedModelRequest,
) -> ModelEventStream {
    let (body, cancellation) = request.into_parts();
    let decode_cancellation = cancellation.clone();
    let opening = stream::once(async move {
        send_request(&client, endpoint, &model, &api_key, body, cancellation)
            .await
            .map(|response| (response, decode_cancellation, api_key))
    });
    Box::pin(opening.flat_map(|result| match result {
        Ok((response, cancellation, api_key)) => decode_response(response, cancellation, api_key),
        Err(error) => Box::pin(stream::once(async move { Err(error) })),
    }))
}

fn error_stream(error: ProviderError) -> ModelEventStream {
    Box::pin(stream::once(async move { Err(error) }))
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
    body: Vec<u8>,
    cancellation: Cancellation,
) -> Result<Response, ProviderError> {
    if cancellation.is_cancelled() {
        return Err(cancelled_error());
    }
    validate_request_bytes(model, &body)?;
    let pending = client
        .post(endpoint)
        .bearer_auth(api_key)
        .header(header::ACCEPT, "text/event-stream")
        .header(header::CONTENT_TYPE, "application/json")
        .body(body)
        .send();
    let response = tokio::select! {
        () = cancellation.cancelled() => return Err(cancelled_error()),
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

fn validate_non_empty(field: &str, value: &str) -> Result<(), ProviderError> {
    if value.trim().is_empty() {
        return Err(config_error(&format!("{field} must not be empty")));
    }
    Ok(())
}

fn validate_api_key(value: &str) -> Result<(), ProviderError> {
    validate_non_empty("api_key", value)?;
    if value.trim() != value {
        return Err(config_error(
            "api_key must not contain leading or trailing whitespace",
        ));
    }
    header::HeaderValue::from_str(&format!("Bearer {value}"))
        .map(|_| ())
        .map_err(|_| config_error("api_key cannot form an Authorization header"))
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
