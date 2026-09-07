use std::{env, error::Error, sync::Arc};

use forge_runtime_infrastructure::{OpenAiResponsesProvider, ReadThenAnswerProvider};

use crate::runtime_domain::{ModelProvider, RunExecution, RunProvider};

const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
const OPENAI_BASE_URL_ENV: &str = "OPENAI_BASE_URL";
pub const DEFAULT_MODEL: &str = "gpt-5.6-sol";

pub fn endpoint() -> String {
    env::var(OPENAI_BASE_URL_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| OPENAI_BASE_URL.to_owned())
}

pub fn for_execution(execution: &RunExecution) -> Result<Arc<dyn ModelProvider>, Box<dyn Error>> {
    match &execution.provider {
        RunProvider::DeterministicRead { path } => Ok(Arc::new(ReadThenAnswerProvider::new(path))),
        RunProvider::OpenAiResponses { endpoint, model }
        | RunProvider::OpenAiAgent {
            endpoint, model, ..
        } => live_provider(endpoint, model),
    }
}

pub fn preflight_agent(model: Option<&str>) -> Result<(), Box<dyn Error>> {
    let endpoint = endpoint();
    let model = model.unwrap_or(DEFAULT_MODEL);
    let api_key = api_key()?;
    if endpoint == OPENAI_BASE_URL {
        OpenAiResponsesProvider::validate_official_configuration(&endpoint, model, &api_key)?;
    } else {
        OpenAiResponsesProvider::validate_self_hosted_configuration(&endpoint, model, &api_key)?;
    }
    Ok(())
}

fn live_provider(endpoint: &str, model: &str) -> Result<Arc<dyn ModelProvider>, Box<dyn Error>> {
    let api_key = api_key()?;
    let provider = if endpoint == OPENAI_BASE_URL {
        OpenAiResponsesProvider::new(endpoint, model, api_key)?
    } else {
        OpenAiResponsesProvider::new_self_hosted(endpoint, model, api_key)?
    };
    Ok(Arc::new(provider))
}

fn api_key() -> Result<String, Box<dyn Error>> {
    let api_key = env::var("OPENAI_API_KEY")
        .map_err(|_| "OPENAI_API_KEY is required for explicit model execution")?;
    if api_key.trim().is_empty() {
        return Err("OPENAI_API_KEY must not be empty for model execution".into());
    }
    Ok(api_key)
}
