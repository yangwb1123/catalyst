use std::{env, error::Error};

use forge_runtime_application::{
    GroupModelAnalysisDispatchProvider, GroupModelAnalysisRequestCodec,
    GroupPanelSynthesisDispatchProvider,
};
use forge_runtime_infrastructure::OpenAiResponsesProvider;

use crate::runtime_domain::{
    GroupModelAnalysisProvider, GroupPanelSynthesisProvider, ModelEventStream, ModelRequest,
    PreparedModelProvider, PreparedModelRequest, ProviderError,
};

pub(crate) const DEFAULT_OPENAI_MODEL: &str = "gpt-5.6-sol";
const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

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
            inner: OpenAiResponsesProvider::new(OPENAI_BASE_URL, model, api_key)?,
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
