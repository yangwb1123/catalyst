use std::sync::{
    Arc, Barrier,
    atomic::{AtomicUsize, Ordering},
};

use forge_runtime_domain::{
    GroupAgentNodePricingSnapshot, GroupAgentScheduledNodeProviderFactoryError,
    GroupAgentScheduledNodeResolvedDispatch, GroupAgentScheduledReadyNodeDispatchAuthorization,
    GroupAgentScheduledReadyNodeProviderFactory, ModelEvent, ModelEventStream, ModelFinishReason,
    PreparedModelProvider, PreparedModelRequest, ProviderError, Usage,
};
use futures_util::stream;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[allow(dead_code)]
pub enum ProviderBehavior {
    #[default]
    Completed,
    TransportError,
}

pub(super) struct ProviderFactory {
    calls: Arc<AtomicUsize>,
    build_barrier: Option<Arc<Barrier>>,
    behavior: ProviderBehavior,
}

impl ProviderFactory {
    pub(super) fn new(
        calls: Arc<AtomicUsize>,
        build_barrier: Option<Arc<Barrier>>,
        behavior: ProviderBehavior,
    ) -> Self {
        Self {
            calls,
            build_barrier,
            behavior,
        }
    }
}

impl GroupAgentScheduledReadyNodeProviderFactory for ProviderFactory {
    fn resolve_ready(
        &self,
        authorization: &GroupAgentScheduledReadyNodeDispatchAuthorization,
        pricing: &GroupAgentNodePricingSnapshot,
    ) -> Result<GroupAgentScheduledNodeResolvedDispatch, GroupAgentScheduledNodeProviderFactoryError>
    {
        let quote = pricing
            .verify_scheduled_ready_authorization(authorization)
            .map_err(|_| factory_error())?;
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

    fn build_ready(
        &self,
        _resolved: GroupAgentScheduledNodeResolvedDispatch,
        credential: String,
    ) -> Result<Box<dyn PreparedModelProvider>, GroupAgentScheduledNodeProviderFactoryError> {
        if credential != "secret" {
            return Err(factory_error());
        }
        if let Some(barrier) = &self.build_barrier {
            barrier.wait();
        }
        Ok(Box::new(Provider {
            calls: self.calls.clone(),
            behavior: self.behavior,
        }))
    }
}

fn factory_error() -> GroupAgentScheduledNodeProviderFactoryError {
    GroupAgentScheduledNodeProviderFactoryError {
        message: "test provider rejected".into(),
    }
}

struct Provider {
    calls: Arc<AtomicUsize>,
    behavior: ProviderBehavior,
}

impl PreparedModelProvider for Provider {
    fn stream_prepared(&self, request: PreparedModelRequest) -> ModelEventStream {
        assert!(!request.body().is_empty());
        self.calls.fetch_add(1, Ordering::AcqRel);
        Box::pin(stream::iter(provider_events(self.behavior)))
    }
}

fn provider_events(behavior: ProviderBehavior) -> Vec<Result<ModelEvent, ProviderError>> {
    match behavior {
        ProviderBehavior::Completed => vec![
            Ok(ModelEvent::TextDelta {
                delta: "done".into(),
            }),
            Ok(ModelEvent::Usage {
                usage: Usage {
                    input_tokens: 1,
                    output_tokens: 1,
                },
            }),
            Ok(ModelEvent::Finished {
                reason: ModelFinishReason::Completed,
            }),
        ],
        ProviderBehavior::TransportError => vec![Err(ProviderError::new(
            "transport_error",
            "deterministic transport uncertainty",
            true,
        ))],
    }
}
