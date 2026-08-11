mod closure;
mod error;
mod projection;
mod read;
mod rows;
pub(super) mod semantic;
mod stored;
mod write;

use crate::runtime_domain::{
    AppendGovernanceRecordBatch, AppendGovernanceRecordBatchResult, GovernanceRecordInspection,
    GovernanceRecordJournalStore, GovernanceRecordKind, GovernanceRecordListFilter,
    GovernanceStructuralHead, HubStoreError,
};

use super::SqliteHubStore;

impl GovernanceRecordJournalStore for SqliteHubStore {
    fn append_governance_record_batch(
        &self,
        request: &AppendGovernanceRecordBatch,
    ) -> Result<AppendGovernanceRecordBatchResult, HubStoreError> {
        write::append(&mut self.connect()?, request)
    }

    fn inspect_governance_record(
        &self,
        record_id: &str,
        include_record: bool,
    ) -> Result<GovernanceRecordInspection, HubStoreError> {
        read::inspect(&mut self.connect()?, record_id, include_record)
    }

    fn list_governance_records(
        &self,
        filter: &GovernanceRecordListFilter,
    ) -> Result<Vec<GovernanceRecordInspection>, HubStoreError> {
        read::list(&mut self.connect()?, filter)
    }

    fn inspect_governance_structural_head(
        &self,
        record_kind: GovernanceRecordKind,
        aggregate_id: &str,
    ) -> Result<GovernanceStructuralHead, HubStoreError> {
        projection::inspect(&mut self.connect()?, record_kind, aggregate_id)
    }

    fn rebuild_governance_structural_heads(&self) -> Result<usize, HubStoreError> {
        projection::rebuild(&mut self.connect()?)
    }
}

#[cfg(test)]
mod tests;
