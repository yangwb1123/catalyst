use crate::HubStoreError;

use super::{
    AppendGovernanceRecordBatch, AppendGovernanceRecordBatchResult, GovernanceRecordInspection,
    GovernanceRecordListFilter, GovernanceStructuralHead,
};

pub trait GovernanceRecordJournalStore: Send + Sync {
    /// Atomically stores a bounded exact batch or returns its original receipt metadata.
    ///
    /// Implementations must resolve references, compare idempotency, enforce structural
    /// sequence progression, append immutable records, and update structural heads in one
    /// transaction. Exact replay changes only the result disposition; it preserves the first
    /// receipt's batch identity and append time. This operation does not attest truth,
    /// authority, freshness, or validity.
    ///
    /// # Errors
    ///
    /// Returns a structured conflict, corruption, or availability error.
    fn append_governance_record_batch(
        &self,
        request: &AppendGovernanceRecordBatch,
    ) -> Result<AppendGovernanceRecordBatchResult, HubStoreError>;

    /// Loads metadata and, only when requested, the exact canonical record bytes.
    ///
    /// # Errors
    ///
    /// Returns a structured not-found, corruption, or availability error.
    fn inspect_governance_record(
        &self,
        record_id: &str,
        include_record: bool,
    ) -> Result<GovernanceRecordInspection, HubStoreError>;

    /// Lists bounded record metadata newest-first with optional exact content reveal.
    ///
    /// # Errors
    ///
    /// Returns a structured corruption or availability error.
    fn list_governance_records(
        &self,
        filter: &GovernanceRecordListFilter,
    ) -> Result<Vec<GovernanceRecordInspection>, HubStoreError>;

    /// Loads one rebuildable structural sequence head.
    ///
    /// The result is not a current truth, authority, lifecycle, or freshness view.
    ///
    /// # Errors
    ///
    /// Returns a structured not-found, corruption, or availability error.
    fn inspect_governance_structural_head(
        &self,
        record_kind: super::GovernanceRecordKind,
        aggregate_id: &str,
    ) -> Result<GovernanceStructuralHead, HubStoreError>;

    /// Rebuilds only the structural sequence projection from immutable journal records.
    ///
    /// # Errors
    ///
    /// Returns a structured corruption or availability error without partially rebuilding.
    fn rebuild_governance_structural_heads(&self) -> Result<usize, HubStoreError>;
}
