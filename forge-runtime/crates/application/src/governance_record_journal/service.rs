use std::sync::Arc;

use crate::runtime_domain::{
    AppendGovernanceRecordBatch, AppendGovernanceRecordBatchResult, GovernanceRecordInspection,
    GovernanceRecordJournalStore, GovernanceRecordKind, GovernanceRecordListFilter,
    GovernanceStructuralHead, HubStoreError, validate_governance_record_idempotency_key,
};
use thiserror::Error;

use super::validation::{
    invalid, validate_append_result, validate_head, validate_identifier, validate_inspection,
    validate_list,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppendGovernanceRecordBatchInput {
    pub canonical_record_set_json: String,
    pub idempotency_key: String,
    pub appended_at_ms: u64,
}

#[derive(Debug, Error)]
pub enum GovernanceRecordJournalServiceError {
    #[error("governance record journal input is invalid: {message}")]
    InvalidInput { message: String },
    #[error("governance record journal store returned inconsistent state")]
    InconsistentStoreResult,
    #[error("governance record journal store failed: {0}")]
    Store(#[from] HubStoreError),
}

pub struct GovernanceRecordJournalService {
    store: Arc<dyn GovernanceRecordJournalStore>,
}

impl GovernanceRecordJournalService {
    #[must_use]
    pub fn new(store: Arc<dyn GovernanceRecordJournalStore>) -> Self {
        Self { store }
    }

    /// Validates an append replay key before a potentially blocking input read.
    ///
    /// # Errors
    ///
    /// Returns an input error for an invalid caller-owned idempotency key.
    pub fn preflight_append_key(
        idempotency_key: &str,
    ) -> Result<(), GovernanceRecordJournalServiceError> {
        validate_governance_record_idempotency_key(idempotency_key)
            .map_err(|error| invalid(error.message))
    }

    /// Builds and fully validates an append request before storage is opened.
    ///
    /// # Errors
    ///
    /// Returns an input error for an invalid key, time, record, or canonical record set.
    pub fn prepare_append_request(
        input: AppendGovernanceRecordBatchInput,
    ) -> Result<AppendGovernanceRecordBatch, GovernanceRecordJournalServiceError> {
        AppendGovernanceRecordBatch::from_canonical_record_set(
            input.canonical_record_set_json,
            input.idempotency_key,
            input.appended_at_ms,
        )
        .map_err(|error| invalid(error.message))
    }

    /// Validates an inspection identifier before any read-only store is opened.
    ///
    /// # Errors
    ///
    /// Returns an input error for an invalid record identifier.
    pub fn preflight_inspect(record_id: &str) -> Result<(), GovernanceRecordJournalServiceError> {
        validate_identifier(record_id)
    }

    /// Validates a list filter before any read-only store is opened.
    ///
    /// # Errors
    ///
    /// Returns an input error for an invalid filter or bound.
    pub fn preflight_list(
        filter: &GovernanceRecordListFilter,
    ) -> Result<(), GovernanceRecordJournalServiceError> {
        filter.validate().map_err(|error| invalid(error.message))
    }

    /// Validates a structural-head aggregate before any read-only store is opened.
    ///
    /// # Errors
    ///
    /// Returns an input error for an invalid aggregate identifier.
    pub fn preflight_head(aggregate_id: &str) -> Result<(), GovernanceRecordJournalServiceError> {
        validate_identifier(aggregate_id)
    }

    /// Atomically appends a preflighted exact batch or returns its original replay receipt.
    ///
    /// # Errors
    ///
    /// Returns an input, consistency, conflict, corruption, or availability error.
    pub fn append_prepared(
        &self,
        request: &AppendGovernanceRecordBatch,
    ) -> Result<AppendGovernanceRecordBatchResult, GovernanceRecordJournalServiceError> {
        request.validate().map_err(|error| invalid(error.message))?;
        let result = self.store.append_governance_record_batch(request)?;
        validate_append_result(request, &result)?;
        Ok(result)
    }

    /// Loads minimized metadata and reveals canonical content only on explicit request.
    ///
    /// # Errors
    ///
    /// Returns an input, not-found, consistency, corruption, or availability error.
    pub fn inspect(
        &self,
        record_id: &str,
        include_record: bool,
    ) -> Result<GovernanceRecordInspection, GovernanceRecordJournalServiceError> {
        Self::preflight_inspect(record_id)?;
        let inspection = self
            .store
            .inspect_governance_record(record_id, include_record)?;
        validate_inspection(&inspection, record_id, include_record)?;
        Ok(inspection)
    }

    /// Lists bounded newest-first metadata with optional exact content reveal.
    ///
    /// # Errors
    ///
    /// Returns an input, consistency, corruption, or availability error.
    pub fn list(
        &self,
        filter: &GovernanceRecordListFilter,
    ) -> Result<Vec<GovernanceRecordInspection>, GovernanceRecordJournalServiceError> {
        Self::preflight_list(filter)?;
        let records = self.store.list_governance_records(filter)?;
        validate_list(&records, filter)?;
        Ok(records)
    }

    /// Loads one structural sequence-only aggregate head.
    ///
    /// # Errors
    ///
    /// Returns an input, not-found, consistency, corruption, or availability error.
    pub fn inspect_structural_head(
        &self,
        record_kind: GovernanceRecordKind,
        aggregate_id: &str,
    ) -> Result<GovernanceStructuralHead, GovernanceRecordJournalServiceError> {
        Self::preflight_head(aggregate_id)?;
        let head = self
            .store
            .inspect_governance_structural_head(record_kind, aggregate_id)?;
        validate_head(&head, record_kind, aggregate_id)?;
        Ok(head)
    }

    /// Rebuilds only the deterministic structural sequence projection.
    ///
    /// # Errors
    ///
    /// Returns a corruption or availability error without claiming epistemic currentness.
    pub fn rebuild_structural_heads(&self) -> Result<usize, GovernanceRecordJournalServiceError> {
        self.store
            .rebuild_governance_structural_heads()
            .map_err(Into::into)
    }
}
