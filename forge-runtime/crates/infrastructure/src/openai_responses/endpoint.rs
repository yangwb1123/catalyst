#[cfg(test)]
use std::net::IpAddr;

use reqwest::{Client, Url};

use super::{CONNECT_TIMEOUT, EndpointPolicy, READ_TIMEOUT, REQUEST_TIMEOUT, config_error};
use crate::runtime_domain::ProviderError;

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
        #[cfg(test)]
        EndpointPolicy::TestLoopback => validate_test_base_url(&url)?,
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

#[cfg(test)]
fn is_loopback_host(host: &str) -> bool {
    host == "localhost"
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

pub(super) fn build_client(endpoint_policy: EndpointPolicy) -> Result<Client, ProviderError> {
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
