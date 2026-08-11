mod batch_validation;
mod budget;
mod integrity;
mod parity;
mod projection;
mod read;
mod rows;
mod stored;

use crate::runtime_domain::{
    GovernanceClaimConflictGroup, GovernanceRecordKind, GovernanceSemanticListFilter,
    GovernanceSemanticProjection, GovernanceSemanticViewStore, GovernanceValidationJob,
    GovernanceValidationJobFilter, HubStoreError,
};

use super::super::SqliteHubStore;

impl GovernanceSemanticViewStore for SqliteHubStore {
    fn inspect_governance_semantic_projection(
        &self,
        record_kind: GovernanceRecordKind,
        aggregate_id: &str,
    ) -> Result<GovernanceSemanticProjection, HubStoreError> {
        read::inspect(&mut self.connect()?, record_kind, aggregate_id)
    }

    fn list_governance_claim_conflicts(
        &self,
        filter: &GovernanceSemanticListFilter,
    ) -> Result<Vec<GovernanceClaimConflictGroup>, HubStoreError> {
        read::conflicts(&mut self.connect()?, filter)
    }

    fn list_governance_validation_jobs(
        &self,
        filter: &GovernanceValidationJobFilter,
    ) -> Result<Vec<GovernanceValidationJob>, HubStoreError> {
        read::validation_jobs(&mut self.connect()?, filter)
    }

    fn rebuild_governance_semantic_views(&self) -> Result<usize, HubStoreError> {
        projection::rebuild(&mut self.connect()?)
    }
}

pub(crate) use projection::{
    rebuild_locked, rebuild_materialized_locked, refresh_after_append,
    validate_current_for_candidates, validate_prior_projections,
};

#[cfg(test)]
pub(crate) use integrity::{install_after_snapshot_hook, scan_stats};
