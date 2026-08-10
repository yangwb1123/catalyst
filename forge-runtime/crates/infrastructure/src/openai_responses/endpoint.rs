use std::net::IpAddr;

use reqwest::{Client, Url};

use super::{
    CONNECT_TIMEOUT, EndpointPolicy, OpenAiResponsesProvider, READ_TIMEOUT, REQUEST_TIMEOUT,
    config_error,
};
use crate::runtime_domain::ProviderError;

impl OpenAiResponsesProvider {
    /// Resolves the exact Responses endpoint for an official `/v1` base URL
    /// without constructing a client or reading credentials.
    ///
    /// # Errors
    ///
    /// Returns an error when `base_url` is not the official `OpenAI` `/v1` URL.
    pub fn resolve_official_endpoint(base_url: &str) -> Result<String, ProviderError> {
        responses_endpoint(base_url, EndpointPolicy::Official).map(|url| url.to_string())
    }

    /// Resolves the exact Responses endpoint for an explicitly selected
    /// self-hosted `/v1` base URL without constructing a client or reading
    /// credentials.
    ///
    /// # Errors
    ///
    /// Returns an error when `base_url` violates the self-hosted endpoint
    /// policy.
    pub fn resolve_self_hosted_endpoint(base_url: &str) -> Result<String, ProviderError> {
        responses_endpoint(base_url, EndpointPolicy::SelfHosted).map(|url| url.to_string())
    }
}

pub(super) fn responses_endpoint(
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
        // Self-hosted gateways (LiteLLM/Ollama/one-api behind an explicit
        // OPENAI_BASE_URL opt-in): HTTPS anywhere, or HTTP restricted to
        // loopback — a plaintext non-loopback endpoint stays refused.
        EndpointPolicy::SelfHosted => validate_self_hosted_base_url(&url)?,
        #[cfg(test)]
        EndpointPolicy::TestLoopback => validate_test_base_url(&url)?,
    }
    url.set_path("/v1/responses");
    Ok(url)
}

fn validate_self_hosted_base_url(url: &Url) -> Result<(), ProviderError> {
    let loopback = is_loopback_host(url.host_str().unwrap_or(""));
    if url.scheme() == "https" {
        if url.port().is_some() {
            return Err(config_error(
                "self-hosted base_url must not carry an explicit port with https",
            ));
        }
    } else if url.scheme() == "http" {
        if !loopback {
            return Err(config_error(
                "self-hosted http base_url must be a loopback host",
            ));
        }
    } else {
        return Err(config_error(
            "self-hosted base_url must be http(s) (loopback http allowed, https anywhere)",
        ));
    }
    if !matches!(url.path(), "/v1" | "/v1/") {
        return Err(config_error("self-hosted base_url must end in /v1"));
    }
    Ok(())
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
fn validate_test_base_url(url: &Url) -> Result<(), ProviderError> {
    if url.scheme() != "http"
        || !url.host_str().is_some_and(is_loopback_host)
        || !matches!(url.path(), "/v1" | "/v1/")
    {
        return Err(config_error(
            "test base_url must be an HTTP loopback URL ending in /v1",
        ));
    }
    Ok(())
}

fn is_loopback_host(host: &str) -> bool {
    host == "localhost"
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

pub(super) fn build_client(endpoint_policy: EndpointPolicy) -> Result<Client, ProviderError> {
    Client::builder()
        .no_proxy()
        .https_only(matches!(endpoint_policy, EndpointPolicy::Official))
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .build()
        .map_err(|_| config_error("failed to construct the HTTP client"))
}

#[cfg(test)]
mod self_hosted_policy_tests {
    use super::{EndpointPolicy, responses_endpoint};

    fn accepts(base_url: &str) -> bool {
        responses_endpoint(base_url, EndpointPolicy::SelfHosted).is_ok()
    }

    #[test]
    fn loopback_http_v1_is_accepted() {
        assert!(accepts("http://127.0.0.1:4001/v1"));
        assert!(accepts("http://localhost:11434/v1"));
    }

    #[test]
    fn arbitrary_https_v1_is_accepted() {
        assert!(accepts("https://llm.internal.example/v1"));
    }

    #[test]
    fn plaintext_non_loopback_is_refused() {
        assert!(!accepts("http://llm.internal.example/v1"));
    }

    #[test]
    fn wrong_path_is_refused() {
        assert!(!accepts("http://127.0.0.1:4001"));
        assert!(!accepts("https://llm.internal.example/v1/extra"));
    }

    #[test]
    fn official_policy_still_refuses_self_hosted_urls() {
        assert!(responses_endpoint("http://127.0.0.1:4001/v1", EndpointPolicy::Official).is_err());
        assert!(
            responses_endpoint("https://llm.internal.example/v1", EndpointPolicy::Official,)
                .is_err()
        );
    }
}

#[cfg(test)]
mod ambient_proxy_contract_tests {
    #[test]
    fn production_client_builder_disables_ambient_proxy_discovery() {
        let source = include_str!("endpoint.rs");
        let marker = [".no_", "proxy()"].concat();
        let (_, after_signature) = source
            .split_once("pub(super) fn build_client")
            .expect("client builder source");
        let (builder, _) = after_signature
            .split_once(".build()")
            .expect("client builder terminator");

        assert!(builder.contains(&marker));
    }
}
