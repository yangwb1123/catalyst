use crate::GroupAgentScheduledNodeDispatchAuthorization;

use super::{
    GroupAgentNodeDestinationRegistryError, GroupAgentNodePricingQuote,
    GroupAgentNodePricingSnapshot, GroupAgentNodePricingValidationError, invalid, maximum_cost,
};

/// Resolves one scheduled authorization against an effect-free destination registry.
pub trait GroupAgentScheduledNodeDestinationRegistry: Send + Sync {
    /// Resolves one exact authorized destination and its conservative quote.
    ///
    /// Implementations must not inspect credentials, construct a provider,
    /// perform a health check, or issue network requests.
    ///
    /// # Errors
    ///
    /// Returns a controlled rejection when the destination is not registered
    /// or its pricing and scheduled authorization bindings are unacceptable.
    fn resolve(
        &self,
        authorization: &GroupAgentScheduledNodeDispatchAuthorization,
        pricing: &GroupAgentNodePricingSnapshot,
    ) -> Result<GroupAgentNodePricingQuote, GroupAgentNodeDestinationRegistryError>;
}

impl GroupAgentNodePricingSnapshot {
    /// Verifies an exact scheduled Dispatch Authorization and computes its
    /// conservative maximum cost under this immutable pricing snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid authorization, destination or pricing drift,
    /// arithmetic overflow, or a maximum cost above the authorized budget.
    pub fn verify_scheduled_authorization(
        &self,
        authorization: &GroupAgentScheduledNodeDispatchAuthorization,
    ) -> Result<GroupAgentNodePricingQuote, GroupAgentNodePricingValidationError> {
        self.validate()?;
        authorization
            .validate()
            .map_err(|_| invalid("scheduled Node Dispatch Authorization is invalid for pricing"))?;
        validate_scheduled_authorization_bindings(self, authorization)?;
        let maximum = maximum_cost(self, authorization.budgets.max_output_tokens)?;
        if maximum > authorization.budgets.max_cost_usd_micros {
            return Err(invalid(
                "authorized scheduled Node Dispatch cost budget is insufficient",
            ));
        }
        Ok(GroupAgentNodePricingQuote {
            pricing_snapshot_sha256: self.pricing_snapshot_sha256.clone(),
            destination_sha256: self.destination_sha256.clone(),
            max_input_tokens: self.max_input_tokens,
            max_output_tokens: authorization.budgets.max_output_tokens,
            max_cost_usd_micros: maximum,
        })
    }
}

fn validate_scheduled_authorization_bindings(
    snapshot: &GroupAgentNodePricingSnapshot,
    authorization: &GroupAgentScheduledNodeDispatchAuthorization,
) -> Result<(), GroupAgentNodePricingValidationError> {
    let exact = authorization.provider_kind == snapshot.provider_kind
        && authorization.endpoint == snapshot.endpoint
        && authorization.model == snapshot.model
        && authorization.destination_sha256 == snapshot.destination_sha256
        && authorization.pricing_snapshot_sha256 == snapshot.pricing_snapshot_sha256
        && authorization.budgets.pricing_snapshot_sha256 == snapshot.pricing_snapshot_sha256;
    exact.then_some(()).ok_or_else(|| {
        invalid("pricing snapshot and scheduled Node Dispatch Authorization disagree")
    })
}
