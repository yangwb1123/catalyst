use crate::runtime_domain::{
    GROUP_AGENT_NODE_OFFICIAL_OPENAI_RESPONSES_ENDPOINT, GroupAgentNodeDestinationRegistry,
    GroupAgentNodeDestinationRegistryError, GroupAgentNodeDispatchAuthorization,
    GroupAgentNodePricingQuote, GroupAgentNodePricingSnapshot, GroupAgentNodeProviderKind,
    ModelEventStream, PreparedModelProvider, PreparedModelRequest,
    group_agent_node_destination_sha256,
};

use super::OpenAiResponsesProvider;

const OFFICIAL_OPENAI_RESPONSES_BASE_URL: &str = "https://api.openai.com/v1";

#[derive(Clone, Copy, Debug, Default)]
pub struct RegisteredGroupAgentNodeProviderFactory;

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
        Self
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
        validate_registered_destination(authorization)?;
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
        let inner = OpenAiResponsesProvider::new(
            OFFICIAL_OPENAI_RESPONSES_BASE_URL,
            readiness.model.clone(),
            credential,
        )
        .map_err(|_| invalid_credential())?;
        if inner.endpoint() != readiness.endpoint || inner.model() != readiness.model {
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

fn validate_registered_destination(
    authorization: &GroupAgentNodeDispatchAuthorization,
) -> Result<(), RegisteredGroupAgentNodeProviderFactoryError> {
    let digest = group_agent_node_destination_sha256(
        authorization.provider_kind,
        &authorization.endpoint,
        &authorization.model,
    );
    let registered = authorization.provider_kind == GroupAgentNodeProviderKind::OpenAiResponses
        && authorization.endpoint == GROUP_AGENT_NODE_OFFICIAL_OPENAI_RESPONSES_ENDPOINT
        && authorization.destination_sha256 == digest;
    registered.then_some(()).ok_or_else(invalid_readiness)
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
