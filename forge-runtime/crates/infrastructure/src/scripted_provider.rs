use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use forge_runtime_domain::{
    ModelEvent, ModelEventStream, ModelProvider, ModelRequest, ProviderError,
};
use futures_util::stream;

pub type ScriptedTurn = Vec<Result<ModelEvent, ProviderError>>;

#[derive(Clone, Default)]
pub struct ScriptedProvider {
    turns: Arc<Mutex<VecDeque<ScriptedTurn>>>,
}

impl ScriptedProvider {
    #[must_use]
    pub fn new(turns: Vec<ScriptedTurn>) -> Self {
        Self {
            turns: Arc::new(Mutex::new(turns.into())),
        }
    }
}

impl ModelProvider for ScriptedProvider {
    fn stream(&self, _request: ModelRequest) -> ModelEventStream {
        let turn = self
            .turns
            .lock()
            .map_err(|_| ProviderError::new("fixture_poisoned", "fixture lock was poisoned", false))
            .and_then(|mut turns| {
                turns.pop_front().ok_or_else(|| {
                    ProviderError::new(
                        "fixture_exhausted",
                        "scripted provider has no remaining turn",
                        false,
                    )
                })
            });
        match turn {
            Ok(events) => Box::pin(stream::iter(events)),
            Err(error) => Box::pin(stream::once(async move { Err(error) })),
        }
    }
}
