//! Pure Attempt lifecycle values over the Platform Core state graph.
//!
//! Requests are caller declarations. A reduced value does not prove current
//! durable state, authenticate an observation, or authorize execution.

use crate::platform_core_contract::{
    AttemptState, PlatformCoreContractError, validate_attempt_transition,
};

/// An immutable lifecycle value with no identity, version, or transition authority.
///
/// Construction begins at `requested`; durable restoration belongs to FR-04.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttemptLifecycle {
    state: AttemptState,
}

/// A closed declaration requesting one canonical Attempt transition.
///
/// Observations describe the caller's asserted outcome; this value neither
/// authenticates the caller nor retains observation evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttemptTransitionRequest {
    /// Declare acceptance of the request without authorizing an effect.
    Accept,
    /// Declare that starting has begun.
    BeginStarting,
    /// Declare observed execution in progress.
    ObserveRunning,
    /// Declare a known interruption, not an unproved effect outcome.
    ObserveInterrupted,
    /// Declare a known completed outcome.
    ObserveCompleted,
    /// Declare a known failed outcome, not an unproved effect outcome.
    ObserveFailed,
    /// Declare that an effect may have started and its outcome is unproved.
    ///
    /// Missing journal, projection, or history alone does not justify this
    /// declaration. FR-04 must retain actual evidence before durable use.
    ObserveEffectOutcomeUncertain,
}

impl AttemptLifecycle {
    /// Creates the initial value without creating a durable Attempt aggregate.
    #[must_use]
    pub const fn requested() -> Self {
        Self {
            state: AttemptState::Requested,
        }
    }

    /// Returns the state of this value, not a current-state observation.
    #[must_use]
    pub const fn state(&self) -> &AttemptState {
        &self.state
    }

    /// Returns a new value after checking the canonical Platform Core edge.
    ///
    /// The current value remains unchanged on both success and rejection.
    /// Terminal values have no outgoing edges; same-state requests are rejected.
    ///
    /// # Errors
    /// Returns `pc_transition_invalid` for every undeclared edge.
    pub fn reduce(
        &self,
        request: &AttemptTransitionRequest,
    ) -> Result<Self, PlatformCoreContractError> {
        let target = request.target_state();
        validate_attempt_transition(&self.state, &target)?;
        Ok(Self { state: target })
    }
}

impl AttemptTransitionRequest {
    const fn target_state(self) -> AttemptState {
        match self {
            Self::Accept => AttemptState::Accepted,
            Self::BeginStarting => AttemptState::Starting,
            Self::ObserveRunning => AttemptState::Running,
            Self::ObserveInterrupted => AttemptState::Interrupted,
            Self::ObserveCompleted => AttemptState::Completed,
            Self::ObserveFailed => AttemptState::Failed,
            Self::ObserveEffectOutcomeUncertain => AttemptState::Uncertain,
        }
    }
}
