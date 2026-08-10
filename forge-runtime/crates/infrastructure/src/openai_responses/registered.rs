use crate::runtime_domain::{
    GROUP_AGENT_NODE_OFFICIAL_OPENAI_RESPONSES_ENDPOINT, GroupAgentNodeDestinationRegistry,
    GroupAgentNodeDestinationRegistryError, GroupAgentNodeDispatchAuthorization,
    GroupAgentNodeDispatchProviderFactory, GroupAgentNodeDispatchProviderFactoryError,
    GroupAgentNodePricingQuote, GroupAgentNodePricingSnapshot, GroupAgentNodeProviderKind,
    GroupAgentNodeResolvedDispatch, GroupAgentScheduledNodeDestinationRegistry,
    GroupAgentScheduledNodeDispatchAuthorization, GroupAgentScheduledNodeProviderFactory,
    GroupAgentScheduledNodeProviderFactoryError, GroupAgentScheduledNodeResolvedDispatch,
    ModelEventStream, PreparedModelProvider, PreparedModelRequest,
    group_agent_node_destination_sha256,
};

use super::{EndpointPolicy, OpenAiResponsesProvider};

#[derive(Clone, Copy, Debug, Default)]
pub struct RegisteredGroupAgentNodeProviderFactory {
    endpoint_policy: EndpointPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredGroupAgentNodeProviderReadiness {
    authorization_sha256: String,
    provider_kind: GroupAgentNodeProviderKind,
    endpoint: String,
    model: String,
    destination_sha256: String,
    pricing_snapshot_sha256: String,
    quote: GroupAgentNodePricingQuote,
}

pub struct RegisteredGroupAgentNodeProvider {
    inner: OpenAiResponsesProvider,
    readiness: RegisteredGroupAgentNodeProviderReadiness,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredGroupAgentNodeProviderFactoryError {
    pub message: String,
}

impl RegisteredGroupAgentNodeProviderFactory {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            endpoint_policy: EndpointPolicy::Official,
        }
    }

    /// Test-only factory that accepts an HTTP loopback Responses endpoint
    /// (used by live-endpoint integration tests against a local gateway).
    #[cfg(test)]
    #[must_use]
    pub fn new_insecure_for_test() -> Self {
        Self {
            endpoint_policy: EndpointPolicy::TestLoopback,
        }
    }

    /// Resolves immutable provider metadata and pricing without credentials,
    /// client construction, provider health checks, or network I/O.
    ///
    /// # Errors
    ///
    /// Returns an error unless both artifacts name the exact registered
    /// `OpenAI` Responses destination and the authorized budget covers the quote.
    pub fn resolve(
        &self,
        authorization: &GroupAgentNodeDispatchAuthorization,
        pricing: &GroupAgentNodePricingSnapshot,
    ) -> Result<
        RegisteredGroupAgentNodeProviderReadiness,
        RegisteredGroupAgentNodeProviderFactoryError,
    > {
        let quote = pricing
            .verify_authorization(authorization)
            .map_err(|_| invalid_readiness())?;
        validate_registered_destination(
            authorization.provider_kind,
            &authorization.endpoint,
            &authorization.model,
            &authorization.destination_sha256,
        )?;
        Ok(RegisteredGroupAgentNodeProviderReadiness {
            authorization_sha256: authorization.authorization_sha256.clone(),
            provider_kind: authorization.provider_kind,
            endpoint: authorization.endpoint.clone(),
            model: authorization.model.clone(),
            destination_sha256: authorization.destination_sha256.clone(),
            pricing_snapshot_sha256: pricing.pricing_snapshot_sha256.clone(),
            quote,
        })
    }

    /// Constructs the registered adapter from an explicitly supplied
    /// credential. Construction performs no provider or model request.
    ///
    /// # Errors
    ///
    /// Returns a redacted error when the credential cannot form a safe
    /// Authorization header or fixed client construction fails.
    pub fn build(
        &self,
        readiness: RegisteredGroupAgentNodeProviderReadiness,
        credential: String,
    ) -> Result<RegisteredGroupAgentNodeProvider, RegisteredGroupAgentNodeProviderFactoryError>
    {
        // The authorization endpoint names the full Responses origin
        // (`https://host/v1/responses` or a test loopback `/v1` base); the
        // adapter builds from the base URL and always serves `/v1/responses`,
        // so compare both sides with the `/responses` suffix normalized away.
        let base_url = readiness
            .endpoint
            .strip_suffix("/responses")
            .unwrap_or(&readiness.endpoint);
        let inner = OpenAiResponsesProvider::build(
            base_url,
            readiness.model.clone(),
            credential,
            self.endpoint_policy,
        )
        .map_err(|_| invalid_credential())?;
        let served_endpoint = inner
            .endpoint()
            .strip_suffix("/responses")
            .unwrap_or(inner.endpoint());
        let authorized_endpoint = readiness
            .endpoint
            .strip_suffix("/responses")
            .unwrap_or(&readiness.endpoint);
        if served_endpoint != authorized_endpoint || inner.model() != readiness.model {
            return Err(invalid_readiness());
        }
        Ok(RegisteredGroupAgentNodeProvider { inner, readiness })
    }
}

impl GroupAgentNodeDestinationRegistry for RegisteredGroupAgentNodeProviderFactory {
    fn resolve(
        &self,
        authorization: &GroupAgentNodeDispatchAuthorization,
        pricing: &GroupAgentNodePricingSnapshot,
    ) -> Result<GroupAgentNodePricingQuote, GroupAgentNodeDestinationRegistryError> {
        RegisteredGroupAgentNodeProviderFactory::resolve(self, authorization, pricing)
            .map(|readiness| readiness.quote)
            .map_err(|_| GroupAgentNodeDestinationRegistryError::Rejected)
    }
}

impl GroupAgentScheduledNodeDestinationRegistry for RegisteredGroupAgentNodeProviderFactory {
    fn resolve(
        &self,
        authorization: &GroupAgentScheduledNodeDispatchAuthorization,
        pricing: &GroupAgentNodePricingSnapshot,
    ) -> Result<GroupAgentNodePricingQuote, GroupAgentNodeDestinationRegistryError> {
        resolve_scheduled_pricing_quote(authorization, pricing)
            .map_err(|_| GroupAgentNodeDestinationRegistryError::Rejected)
    }
}

impl GroupAgentScheduledNodeProviderFactory for RegisteredGroupAgentNodeProviderFactory {
    fn resolve(
        &self,
        authorization: &GroupAgentScheduledNodeDispatchAuthorization,
        pricing: &GroupAgentNodePricingSnapshot,
    ) -> Result<GroupAgentScheduledNodeResolvedDispatch, GroupAgentScheduledNodeProviderFactoryError>
    {
        let quote = resolve_scheduled_pricing_quote(authorization, pricing)
            .map_err(|_| scheduled_factory_error())?;
        Ok(GroupAgentScheduledNodeResolvedDispatch {
            authorization_sha256: authorization.authorization_sha256.clone(),
            provider_kind: authorization.provider_kind,
            endpoint: authorization.endpoint.clone(),
            model: authorization.model.clone(),
            destination_sha256: authorization.destination_sha256.clone(),
            pricing_snapshot_sha256: authorization.pricing_snapshot_sha256.clone(),
            quote,
        })
    }

    fn build(
        &self,
        resolved: GroupAgentScheduledNodeResolvedDispatch,
        credential: String,
    ) -> Result<Box<dyn PreparedModelProvider>, GroupAgentScheduledNodeProviderFactoryError> {
        let valid = resolved.provider_kind == GroupAgentNodeProviderKind::OpenAiResponses
            && resolved.endpoint == GROUP_AGENT_NODE_OFFICIAL_OPENAI_RESPONSES_ENDPOINT
            && resolved.quote.destination_sha256 == resolved.destination_sha256
            && resolved.quote.pricing_snapshot_sha256 == resolved.pricing_snapshot_sha256
            && resolved.quote.max_output_tokens > 0;
        if !valid {
            return Err(scheduled_factory_error());
        }
        let readiness = RegisteredGroupAgentNodeProviderReadiness {
            authorization_sha256: resolved.authorization_sha256,
            provider_kind: resolved.provider_kind,
            endpoint: resolved.endpoint,
            model: resolved.model,
            destination_sha256: resolved.destination_sha256,
            pricing_snapshot_sha256: resolved.pricing_snapshot_sha256,
            quote: resolved.quote,
        };
        RegisteredGroupAgentNodeProviderFactory::build(self, readiness, credential)
            .map(|provider| Box::new(provider) as Box<dyn PreparedModelProvider>)
            .map_err(|_| scheduled_factory_error())
    }
}

impl GroupAgentNodeDispatchProviderFactory for RegisteredGroupAgentNodeProviderFactory {
    fn resolve(
        &self,
        authorization: &GroupAgentNodeDispatchAuthorization,
        pricing: &GroupAgentNodePricingSnapshot,
    ) -> Result<GroupAgentNodeResolvedDispatch, GroupAgentNodeDispatchProviderFactoryError> {
        let readiness =
            RegisteredGroupAgentNodeProviderFactory::resolve(self, authorization, pricing)
                .map_err(|_| lifecycle_factory_error())?;
        Ok(GroupAgentNodeResolvedDispatch {
            authorization_sha256: readiness.authorization_sha256,
            provider_kind: readiness.provider_kind,
            endpoint: readiness.endpoint,
            model: readiness.model,
            destination_sha256: readiness.destination_sha256,
            pricing_snapshot_sha256: readiness.pricing_snapshot_sha256,
            max_input_tokens: readiness.quote.max_input_tokens,
            max_output_tokens: readiness.quote.max_output_tokens,
            max_cost_usd_micros: readiness.quote.max_cost_usd_micros,
        })
    }

    fn build(
        &self,
        resolved: GroupAgentNodeResolvedDispatch,
        credential: String,
    ) -> Result<Box<dyn PreparedModelProvider>, GroupAgentNodeDispatchProviderFactoryError> {
        validate_resolved_dispatch(&resolved)?;
        let readiness = RegisteredGroupAgentNodeProviderReadiness {
            authorization_sha256: resolved.authorization_sha256,
            provider_kind: resolved.provider_kind,
            endpoint: resolved.endpoint,
            model: resolved.model,
            destination_sha256: resolved.destination_sha256,
            pricing_snapshot_sha256: resolved.pricing_snapshot_sha256,
            quote: GroupAgentNodePricingQuote {
                pricing_snapshot_sha256: String::new(),
                destination_sha256: String::new(),
                max_input_tokens: resolved.max_input_tokens,
                max_output_tokens: resolved.max_output_tokens,
                max_cost_usd_micros: resolved.max_cost_usd_micros,
            },
        };
        let mut readiness = readiness;
        readiness
            .quote
            .pricing_snapshot_sha256
            .clone_from(&readiness.pricing_snapshot_sha256);
        readiness
            .quote
            .destination_sha256
            .clone_from(&readiness.destination_sha256);
        RegisteredGroupAgentNodeProviderFactory::build(self, readiness, credential)
            .map(|provider| Box::new(provider) as Box<dyn PreparedModelProvider>)
            .map_err(|_| lifecycle_factory_error())
    }
}

fn validate_resolved_dispatch(
    resolved: &GroupAgentNodeResolvedDispatch,
) -> Result<(), GroupAgentNodeDispatchProviderFactoryError> {
    let destination = group_agent_node_destination_sha256(
        resolved.provider_kind,
        &resolved.endpoint,
        &resolved.model,
    );
    let valid = resolved.provider_kind == GroupAgentNodeProviderKind::OpenAiResponses
        && resolved.endpoint
            == crate::runtime_domain::GROUP_AGENT_NODE_OFFICIAL_OPENAI_RESPONSES_ENDPOINT
        && resolved.destination_sha256 == destination
        && is_digest(&resolved.authorization_sha256)
        && is_digest(&resolved.pricing_snapshot_sha256)
        && resolved.max_input_tokens > 0
        && resolved.max_output_tokens > 0
        && resolved.max_cost_usd_micros > 0;
    valid.then_some(()).ok_or_else(lifecycle_factory_error)
}

fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn lifecycle_factory_error() -> GroupAgentNodeDispatchProviderFactoryError {
    GroupAgentNodeDispatchProviderFactoryError {
        message: "registered Group Agent Node provider is unavailable".into(),
    }
}

fn validate_registered_destination(
    provider_kind: GroupAgentNodeProviderKind,
    endpoint: &str,
    model: &str,
    destination_sha256: &str,
) -> Result<(), RegisteredGroupAgentNodeProviderFactoryError> {
    let digest = group_agent_node_destination_sha256(provider_kind, endpoint, model);
    #[cfg(test)]
    let endpoint_ok = endpoint == GROUP_AGENT_NODE_OFFICIAL_OPENAI_RESPONSES_ENDPOINT
        || is_test_loopback_endpoint(endpoint);
    #[cfg(not(test))]
    let endpoint_ok = endpoint == GROUP_AGENT_NODE_OFFICIAL_OPENAI_RESPONSES_ENDPOINT;
    let registered = provider_kind == GroupAgentNodeProviderKind::OpenAiResponses
        && endpoint_ok
        && destination_sha256 == digest;
    registered.then_some(()).ok_or_else(invalid_readiness)
}

#[cfg(test)]
fn is_test_loopback_endpoint(endpoint: &str) -> bool {
    endpoint
        .parse::<url::Url>()
        .is_ok_and(|url| url.scheme() == "http" && url.host_str().is_some_and(is_loopback))
}

#[cfg(test)]
fn is_loopback(host: &str) -> bool {
    host == "localhost"
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn resolve_scheduled_pricing_quote(
    authorization: &GroupAgentScheduledNodeDispatchAuthorization,
    pricing: &GroupAgentNodePricingSnapshot,
) -> Result<GroupAgentNodePricingQuote, RegisteredGroupAgentNodeProviderFactoryError> {
    let quote = pricing
        .verify_scheduled_authorization(authorization)
        .map_err(|_| invalid_readiness())?;
    validate_registered_destination(
        authorization.provider_kind,
        &authorization.endpoint,
        &authorization.model,
        &authorization.destination_sha256,
    )?;
    Ok(quote)
}

impl RegisteredGroupAgentNodeProviderReadiness {
    #[must_use]
    pub fn authorization_sha256(&self) -> &str {
        &self.authorization_sha256
    }

    #[must_use]
    pub const fn provider_kind(&self) -> GroupAgentNodeProviderKind {
        self.provider_kind
    }

    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    #[must_use]
    pub fn destination_sha256(&self) -> &str {
        &self.destination_sha256
    }

    #[must_use]
    pub fn pricing_snapshot_sha256(&self) -> &str {
        &self.pricing_snapshot_sha256
    }

    #[must_use]
    pub const fn quote(&self) -> &GroupAgentNodePricingQuote {
        &self.quote
    }
}

impl RegisteredGroupAgentNodeProvider {
    #[must_use]
    pub const fn readiness(&self) -> &RegisteredGroupAgentNodeProviderReadiness {
        &self.readiness
    }

    #[must_use]
    pub fn endpoint(&self) -> &str {
        self.inner.endpoint()
    }

    #[must_use]
    pub fn model(&self) -> &str {
        self.inner.model()
    }
}

impl PreparedModelProvider for RegisteredGroupAgentNodeProvider {
    fn stream_prepared(&self, request: PreparedModelRequest) -> ModelEventStream {
        self.inner.stream_prepared(request)
    }
}

impl std::fmt::Display for RegisteredGroupAgentNodeProviderFactoryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RegisteredGroupAgentNodeProviderFactoryError {}

fn invalid_readiness() -> RegisteredGroupAgentNodeProviderFactoryError {
    RegisteredGroupAgentNodeProviderFactoryError {
        message: "registered Group Agent Node provider readiness is invalid".into(),
    }
}

fn invalid_credential() -> RegisteredGroupAgentNodeProviderFactoryError {
    RegisteredGroupAgentNodeProviderFactoryError {
        message: "registered Group Agent Node provider credential is invalid".into(),
    }
}

fn scheduled_factory_error() -> GroupAgentScheduledNodeProviderFactoryError {
    GroupAgentScheduledNodeProviderFactoryError {
        message: "registered scheduled Node provider is unavailable".into(),
    }
}
