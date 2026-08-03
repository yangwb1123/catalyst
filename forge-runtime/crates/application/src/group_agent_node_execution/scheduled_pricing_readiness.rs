use std::sync::Arc;

use thiserror::Error;

use crate::runtime_domain::{
    GroupAgentGraphExecutionScheduleStore, GroupAgentGraphRunStore, GroupAgentGraphStore,
    GroupAgentNodePricingQuote, GroupAgentNodePricingSnapshot,
    GroupAgentScheduledNodeContractStore, GroupAgentScheduledNodeDestinationRegistry,
    GroupAgentScheduledNodeDispatchAuthorization, GroupAgentScheduledNodeProviderRequestStore,
};

use super::{
    GroupAgentNodeDispatchRequestCodec, GroupAgentScheduledNodeDispatchReleaseControlService,
    GroupAgentScheduledNodeDispatchReleaseControlServiceError,
    VerifiedGroupAgentScheduledNodeDispatchAuthorization,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedGroupAgentScheduledNodeDispatchReadiness {
    pub v: u16,
    pub authorization: VerifiedGroupAgentScheduledNodeDispatchAuthorization,
    pub quote: GroupAgentNodePricingQuote,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum GroupAgentScheduledNodeDispatchReadinessServiceError {
    #[error(transparent)]
    Release(#[from] GroupAgentScheduledNodeDispatchReleaseControlServiceError),
    #[error("Scheduled Node Dispatch readiness input is invalid: {message}")]
    InvalidInput { message: String },
}

pub struct GroupAgentScheduledNodeDispatchReadinessService {
    release: GroupAgentScheduledNodeDispatchReleaseControlService,
    destinations: Arc<dyn GroupAgentScheduledNodeDestinationRegistry>,
}

impl GroupAgentScheduledNodeDispatchReadinessService {
    #[must_use]
    pub fn new(
        graphs: Arc<dyn GroupAgentGraphStore>,
        runs: Arc<dyn GroupAgentGraphRunStore>,
        schedules: Arc<dyn GroupAgentGraphExecutionScheduleStore>,
        scheduled_contracts: Arc<dyn GroupAgentScheduledNodeContractStore>,
        provider_requests: Arc<dyn GroupAgentScheduledNodeProviderRequestStore>,
        codec: Arc<dyn GroupAgentNodeDispatchRequestCodec>,
        destinations: Arc<dyn GroupAgentScheduledNodeDestinationRegistry>,
    ) -> Self {
        Self {
            release: GroupAgentScheduledNodeDispatchReleaseControlService::new(
                graphs,
                runs,
                schedules,
                scheduled_contracts,
                provider_requests,
                codec,
            ),
            destinations,
        }
    }

    /// Revalidates current scheduled release state plus exact destination and pricing policy.
    ///
    /// This readiness diagnostic is effect-free. It does not obtain consent,
    /// inspect credentials, construct a provider, claim a lane, dispatch, or
    /// mutate Hub state. Its result is point-in-time metadata, not authority.
    ///
    /// # Errors
    ///
    /// Returns an error when current scheduled authorization, canonical pricing
    /// bytes, destination registration, pricing bindings, arithmetic, budget,
    /// or registry quote agreement fails.
    pub fn verify(
        &self,
        provider_request_id: &str,
        authorization_json: &str,
        pricing_json: &str,
    ) -> Result<
        VerifiedGroupAgentScheduledNodeDispatchReadiness,
        GroupAgentScheduledNodeDispatchReadinessServiceError,
    > {
        let authorization =
            GroupAgentScheduledNodeDispatchAuthorization::decode_exact(authorization_json)
                .map_err(|error| invalid(&error.message))?;
        let verified = self
            .release
            .verify(provider_request_id, authorization_json)?;
        let pricing = GroupAgentNodePricingSnapshot::decode_exact(pricing_json)
            .map_err(|error| invalid(&error.message))?;
        let quote = pricing
            .verify_scheduled_authorization(&authorization)
            .map_err(|error| invalid(&error.message))?;
        let registered_quote = self
            .destinations
            .resolve(&authorization, &pricing)
            .map_err(|_| invalid("destination registry rejected scheduled readiness"))?;
        if registered_quote != quote {
            return Err(invalid(
                "destination registry quote disagrees with scheduled pricing verification",
            ));
        }
        validate_metadata(&verified, &authorization, &quote)?;
        Ok(VerifiedGroupAgentScheduledNodeDispatchReadiness {
            v: pricing.v,
            authorization: verified,
            quote,
        })
    }
}

fn validate_metadata(
    verified: &VerifiedGroupAgentScheduledNodeDispatchAuthorization,
    authorization: &GroupAgentScheduledNodeDispatchAuthorization,
    quote: &GroupAgentNodePricingQuote,
) -> Result<(), GroupAgentScheduledNodeDispatchReadinessServiceError> {
    let exact = verified.v == authorization.v
        && verified.authorization_id == authorization.authorization_id
        && verified.authorization_sha256 == authorization.authorization_sha256
        && verified.scheduled_provider_request_id == authorization.scheduled_provider_request_id
        && verified.destination_sha256 == quote.destination_sha256
        && verified.pricing_snapshot_sha256 == quote.pricing_snapshot_sha256;
    exact
        .then_some(())
        .ok_or_else(|| invalid("scheduled readiness verification metadata disagrees"))
}

fn invalid(message: &str) -> GroupAgentScheduledNodeDispatchReadinessServiceError {
    GroupAgentScheduledNodeDispatchReadinessServiceError::InvalidInput {
        message: message.into(),
    }
}
