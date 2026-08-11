use crate::HubStoreError;

use super::super::GovernanceRecordKind;
use super::{
    GovernanceClaimConflictGroup, GovernanceSemanticListFilter, GovernanceSemanticProjection,
    GovernanceValidationJob, GovernanceValidationJobFilter,
};

pub trait GovernanceSemanticViewStore: Send + Sync {
    /// Reads one exact current semantic projection without evaluating time.
    ///
    /// # Errors
    ///
    /// Returns a store error when the projection is absent, corrupt, or unavailable.
    fn inspect_governance_semantic_projection(
        &self,
        record_kind: GovernanceRecordKind,
        aggregate_id: &str,
    ) -> Result<GovernanceSemanticProjection, HubStoreError>;

    /// Lists bounded, explicitly timed conflict candidates without choosing a winner.
    ///
    /// # Errors
    ///
    /// Returns a store error when the query or any participating projection is invalid.
    fn list_governance_claim_conflicts(
        &self,
        filter: &GovernanceSemanticListFilter,
    ) -> Result<Vec<GovernanceClaimConflictGroup>, HubStoreError>;

    /// Lists bounded validation jobs evaluated at the caller-provided time.
    ///
    /// # Errors
    ///
    /// Returns a store error when the query or any participating projection is invalid.
    fn list_governance_validation_jobs(
        &self,
        filter: &GovernanceValidationJobFilter,
    ) -> Result<Vec<GovernanceValidationJob>, HubStoreError>;

    /// Atomically rebuilds all semantic projections from the exact journal.
    ///
    /// # Errors
    ///
    /// Returns a store error when journal data is corrupt or rebuilding cannot commit.
    fn rebuild_governance_semantic_views(&self) -> Result<usize, HubStoreError>;
}
