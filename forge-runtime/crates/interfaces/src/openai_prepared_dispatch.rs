use std::{env, error::Error};

use forge_runtime_application::{
    GroupAgentNodeDispatchRequestCodec, GroupModelAnalysisDispatchProvider,
    GroupModelAnalysisRequestCodec, GroupPanelSynthesisDispatchProvider,
};
use forge_runtime_infrastructure::OpenAiResponsesProvider;

use crate::runtime_domain::{
    GroupModelAnalysisProvider, GroupPanelSynthesisProvider, ModelEventStream, ModelRequest,
    PreparedModelProvider, PreparedModelRequest, ProviderError,
};

pub(crate) const DEFAULT_OPENAI_MODEL: &str = "gpt-5.6-sol";
const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
const OPENAI_BASE_URL_ENV: &str = "OPENAI_BASE_URL";

/// `effective_openai_base_url` honours an explicit `OPENAI_BASE_URL` opt-in for a
/// self-hosted `/v1` gateway (`LiteLLM`/`Ollama`); the official endpoint is the
/// default. Provider-side endpoint policy validates the value (https
/// anywhere, http loopback-only).
pub(crate) fn effective_openai_base_url() -> String {
    env::var(OPENAI_BASE_URL_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| OPENAI_BASE_URL.to_owned())
}

pub(crate) struct OpenAiPreparedProvider {
    inner: OpenAiResponsesProvider,
}

impl OpenAiPreparedProvider {
    pub(crate) fn from_environment(model: &str) -> Result<Self, Box<dyn Error>> {
        let api_key = env::var("OPENAI_API_KEY")
            .map_err(|_| "OPENAI_API_KEY is required after explicit off-machine consent")?;
        if api_key.trim().is_empty() {
            return Err(
                "OPENAI_API_KEY must not be empty after explicit off-machine consent".into(),
            );
        }
        Ok(Self {
            inner: if effective_openai_base_url() == OPENAI_BASE_URL {
                OpenAiResponsesProvider::new(OPENAI_BASE_URL, model, api_key)?
            } else {
                OpenAiResponsesProvider::new_self_hosted(
                    effective_openai_base_url(),
                    model,
                    api_key,
                )?
            },
        })
    }

    pub(crate) fn endpoint(&self) -> &str {
        self.inner.endpoint()
    }

    pub(crate) fn model(&self) -> &str {
        self.inner.model()
    }
}

impl PreparedModelProvider for OpenAiPreparedProvider {
    fn stream_prepared(&self, request: PreparedModelRequest) -> ModelEventStream {
        self.inner.stream_prepared(request)
    }
}

impl GroupModelAnalysisDispatchProvider for OpenAiPreparedProvider {
    fn analysis_provider(&self) -> GroupModelAnalysisProvider {
        GroupModelAnalysisProvider::OpenAiResponses
    }

    fn endpoint(&self) -> &str {
        self.inner.endpoint()
    }

    fn model(&self) -> &str {
        self.inner.model()
    }
}

impl GroupPanelSynthesisDispatchProvider for OpenAiPreparedProvider {
    fn synthesis_provider(&self) -> GroupPanelSynthesisProvider {
        GroupPanelSynthesisProvider::OpenAiResponses
    }

    fn endpoint(&self) -> &str {
        self.inner.endpoint()
    }

    fn model(&self) -> &str {
        self.inner.model()
    }
}

pub(crate) struct OpenAiRequestCodec;

impl GroupAgentNodeDispatchRequestCodec for OpenAiRequestCodec {
    fn encode_request(
        &self,
        model: &str,
        request: &ModelRequest,
    ) -> Result<Vec<u8>, ProviderError> {
        OpenAiResponsesProvider::encode_request_bytes(model, request)
    }

    fn validate_exact_request(
        &self,
        model: &str,
        expected: &ModelRequest,
        actual: &[u8],
    ) -> Result<(), ProviderError> {
        OpenAiResponsesProvider::validate_exact_request_bytes(model, expected, actual)
    }
}

impl GroupModelAnalysisRequestCodec for OpenAiRequestCodec {
    fn encode_request(
        &self,
        model: &str,
        request: &ModelRequest,
    ) -> Result<Vec<u8>, ProviderError> {
        OpenAiResponsesProvider::encode_request_bytes(model, request)
    }

    fn validate_exact_request(
        &self,
        model: &str,
        expected: &ModelRequest,
        actual: &[u8],
    ) -> Result<(), ProviderError> {
        OpenAiResponsesProvider::validate_exact_request_bytes(model, expected, actual)
    }
}
