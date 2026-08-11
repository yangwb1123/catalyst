use std::sync::Arc;

use thiserror::Error;

use crate::runtime_domain::{
    GovernanceClaimConflictGroup, GovernanceRecordKind, GovernanceSemanticAssessment,
    GovernanceSemanticListFilter, GovernanceSemanticViewStore, GovernanceValidationJob,
    GovernanceValidationJobFilter, HubStoreError, evaluate_governance_semantic_projection,
};

use super::validation::{
    inconsistent, invalid, validate_conflicts, validate_identifier, validate_jobs,
    validate_projection,
};

#[derive(Debug, Error)]
pub enum GovernanceSemanticViewServiceError {
    #[error("governance semantic view input is invalid: {message}")]
    InvalidInput { message: String },
    #[error("governance semantic view store returned inconsistent state")]
    InconsistentStoreResult,
    #[error("governance semantic view store failed: {0}")]
    Store(#[from] HubStoreError),
}

pub struct GovernanceSemanticViewService {
    store: Arc<dyn GovernanceSemanticViewStore>,
}

impl GovernanceSemanticViewService {
    #[must_use]
    pub fn new(store: Arc<dyn GovernanceSemanticViewStore>) -> Self {
        Self { store }
    }

    /// Validates an aggregate and explicit evaluation time before storage opens.
    ///
    /// # Errors
    ///
    /// Returns an input error when the identifier or time is invalid.
    pub fn preflight_inspect(
        aggregate_id: &str,
        as_of_unix_ms: i64,
    ) -> Result<(), GovernanceSemanticViewServiceError> {
        validate_identifier(aggregate_id)?;
        if as_of_unix_ms < 0 {
            return Err(invalid("semantic evaluation time is invalid"));
        }
        Ok(())
    }

    /// Validates a conflict-list request before storage opens.
    ///
    /// # Errors
    ///
    /// Returns an input error when the explicit time or bound is invalid.
    pub fn preflight_conflicts(
        filter: &GovernanceSemanticListFilter,
    ) -> Result<(), GovernanceSemanticViewServiceError> {
        filter
            .validate()
            .map_err(|problem| invalid(problem.message))
    }

    /// Validates a validation-job request before storage opens.
    ///
    /// # Errors
    ///
    /// Returns an input error when the explicit time or bound is invalid.
    pub fn preflight_validation_jobs(
        filter: &GovernanceValidationJobFilter,
    ) -> Result<(), GovernanceSemanticViewServiceError> {
        filter
            .validate()
            .map_err(|problem| invalid(problem.message))
    }

    /// Reads the deterministic aggregate projection and evaluates it at caller time.
    ///
    /// # Errors
    ///
    /// Returns an input, not-found, consistency, corruption, or availability error.
    pub fn inspect(
        &self,
        record_kind: GovernanceRecordKind,
        aggregate_id: &str,
        as_of_unix_ms: i64,
    ) -> Result<GovernanceSemanticAssessment, GovernanceSemanticViewServiceError> {
        Self::preflight_inspect(aggregate_id, as_of_unix_ms)?;
        let projection = self
            .store
            .inspect_governance_semantic_projection(record_kind, aggregate_id)?;
        validate_projection(&projection, record_kind, aggregate_id)?;
        evaluate_governance_semantic_projection(projection, as_of_unix_ms)
            .map_err(|_| inconsistent())
    }

    /// Lists bounded deterministic conflict candidates at caller time.
    ///
    /// # Errors
    ///
    /// Returns an input, consistency, corruption, or availability error.
    pub fn list_conflicts(
        &self,
        filter: &GovernanceSemanticListFilter,
    ) -> Result<Vec<GovernanceClaimConflictGroup>, GovernanceSemanticViewServiceError> {
        Self::preflight_conflicts(filter)?;
        let groups = self.store.list_governance_claim_conflicts(filter)?;
        validate_conflicts(&groups, filter)?;
        Ok(groups)
    }

    /// Lists bounded deterministic validation jobs at caller time.
    ///
    /// # Errors
    ///
    /// Returns an input, consistency, corruption, or availability error.
    pub fn list_validation_jobs(
        &self,
        filter: &GovernanceValidationJobFilter,
    ) -> Result<Vec<GovernanceValidationJob>, GovernanceSemanticViewServiceError> {
        Self::preflight_validation_jobs(filter)?;
        let jobs = self.store.list_governance_validation_jobs(filter)?;
        validate_jobs(&jobs, filter)?;
        Ok(jobs)
    }

    /// Atomically rebuilds the deterministic semantic projection from exact records.
    ///
    /// # Errors
    ///
    /// Returns a corruption or availability error. Rebuild grants no truth or authority.
    pub fn rebuild(&self) -> Result<usize, GovernanceSemanticViewServiceError> {
        self.store
            .rebuild_governance_semantic_views()
            .map_err(Into::into)
    }
}
