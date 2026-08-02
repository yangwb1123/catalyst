use std::sync::Arc;

use thiserror::Error;

use crate::runtime_domain::{
    GroupAgentGraphStore, GroupAgentNodeDestinationRegistry, GroupAgentNodeDispatchAuthorization,
    GroupAgentNodeDispatchRequestStore, GroupAgentNodePricingQuote, GroupAgentNodePricingSnapshot,
    MAX_GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_BYTES,
};

use super::{
    GroupAgentNodeDispatchReleaseControlService, GroupAgentNodeDispatchReleaseControlServiceError,
    GroupAgentNodeDispatchRequestCodec, VerifiedGroupAgentNodeDispatchAuthorization,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedGroupAgentNodeDispatchReadiness {
    pub v: u16,
    pub authorization: VerifiedGroupAgentNodeDispatchAuthorization,
    pub quote: GroupAgentNodePricingQuote,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum GroupAgentNodeDispatchReadinessServiceError {
    #[error(transparent)]
    Release(#[from] GroupAgentNodeDispatchReleaseControlServiceError),
    #[error("Node Dispatch readiness input is invalid: {message}")]
    InvalidInput { message: String },
}

pub struct GroupAgentNodeDispatchReadinessService {
    release: GroupAgentNodeDispatchReleaseControlService,
    destinations: Arc<dyn GroupAgentNodeDestinationRegistry>,
}

impl GroupAgentNodeDispatchReadinessService {
    #[must_use]
    pub fn new(
        graphs: Arc<dyn GroupAgentGraphStore>,
        requests: Arc<dyn GroupAgentNodeDispatchRequestStore>,
        codec: Arc<dyn GroupAgentNodeDispatchRequestCodec>,
        destinations: Arc<dyn GroupAgentNodeDestinationRegistry>,
    ) -> Self {
        Self {
            release: GroupAgentNodeDispatchReleaseControlService::new(graphs, requests, codec),
            destinations,
        }
    }

    /// Revalidates current v3 state plus exact destination and pricing policy.
    ///
    /// This is an effect-free readiness diagnostic. It does not obtain consent,
    /// inspect credentials, construct a provider, claim a lane, or dispatch.
    ///
    /// # Errors
    ///
    /// Returns an error when current authorization, canonical pricing bytes,
    /// destination registration, pricing bindings, arithmetic, or budget fails.
    pub fn verify(
        &self,
        graph_run_id: &str,
        authorization_json: &str,
        pricing_json: &str,
    ) -> Result<VerifiedGroupAgentNodeDispatchReadiness, GroupAgentNodeDispatchReadinessServiceError>
    {
        let authorization = decode_authorization(authorization_json)?;
        let verified = self.release.verify(graph_run_id, authorization_json)?;
        let pricing = GroupAgentNodePricingSnapshot::decode_exact(pricing_json)
            .map_err(|error| invalid(&error.message))?;
        let quote = self
            .destinations
            .resolve(&authorization, &pricing)
            .map_err(|_| invalid("destination registry rejected readiness"))?;
        validate_metadata(&verified, &authorization, &quote)?;
        Ok(VerifiedGroupAgentNodeDispatchReadiness {
            v: pricing.v,
            authorization: verified,
            quote,
        })
    }
}

fn decode_authorization(
    json: &str,
) -> Result<GroupAgentNodeDispatchAuthorization, GroupAgentNodeDispatchReadinessServiceError> {
    if !(1..=MAX_GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_BYTES).contains(&json.len()) {
        return Err(invalid("authorization JSON byte bound is invalid"));
    }
    let authorization: GroupAgentNodeDispatchAuthorization = serde_json::from_str(json)
        .map_err(|_| invalid("authorization JSON is malformed or has unknown fields"))?;
    authorization
        .validate()
        .map_err(|_| invalid("authorization JSON is invalid"))?;
    if authorization.canonical_json().as_deref() != Ok(json) {
        return Err(invalid(
            "authorization JSON is not its exact canonical encoding",
        ));
    }
    Ok(authorization)
}

fn validate_metadata(
    verified: &VerifiedGroupAgentNodeDispatchAuthorization,
    authorization: &GroupAgentNodeDispatchAuthorization,
    quote: &GroupAgentNodePricingQuote,
) -> Result<(), GroupAgentNodeDispatchReadinessServiceError> {
    let exact = verified.authorization_sha256 == authorization.authorization_sha256
        && verified.destination_sha256 == quote.destination_sha256
        && verified.pricing_snapshot_sha256 == quote.pricing_snapshot_sha256;
    exact
        .then_some(())
        .ok_or_else(|| invalid("readiness verification metadata disagrees"))
}

fn invalid(message: &str) -> GroupAgentNodeDispatchReadinessServiceError {
    GroupAgentNodeDispatchReadinessServiceError::InvalidInput {
        message: message.into(),
    }
}
