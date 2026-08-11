use serde::{Deserialize, Serialize};

use crate::governance_contract::{ClaimType, EvidenceType};

use super::super::{GovernanceRecordJournalError, GovernanceRecordKind};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernanceTemporalState {
    Fresh,
    NotYetValid,
    ReviewOverdue,
    ValidationOverdue,
    ValidityExpired,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GovernanceSemanticHead {
    pub v: u16,
    pub record_kind: GovernanceRecordKind,
    pub aggregate_id: String,
    pub record_id: String,
    pub sequence: i64,
    pub canonical_sha256: String,
    pub project_id: String,
    pub scope: String,
    pub declared_state: String,
    pub valid_from_unix_ms: i64,
    pub valid_until_unix_ms: Option<i64>,
    pub updated_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GovernanceClaimSemanticFields {
    pub claim_type: ClaimType,
    pub subject: String,
    pub predicate: String,
    pub object_sha256: String,
    pub conflict_key_sha256: String,
    pub review_by_unix_ms: Option<i64>,
    pub validation_due_unix_ms: Option<i64>,
    pub validation_owner_id: Option<String>,
    pub validation_plan_sha256: Option<String>,
    pub required_evidence_types: Vec<EvidenceType>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GovernanceSemanticProjection {
    pub v: u16,
    pub head: GovernanceSemanticHead,
    pub claim: Option<GovernanceClaimSemanticFields>,
    pub projection_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GovernanceSemanticAssessment {
    pub v: u16,
    pub projection: GovernanceSemanticProjection,
    pub evaluated_at_unix_ms: i64,
    pub temporal_state: GovernanceTemporalState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GovernanceClaimConflictMember {
    pub aggregate_id: String,
    pub record_id: String,
    pub sequence: i64,
    pub declared_state: String,
    pub object_sha256: String,
    pub temporal_state: GovernanceTemporalState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GovernanceClaimConflictGroup {
    pub v: u16,
    pub conflict_key_sha256: String,
    pub claim_type: ClaimType,
    pub project_id: String,
    pub scope: String,
    pub subject: String,
    pub predicate: String,
    pub evaluated_at_unix_ms: i64,
    pub members: Vec<GovernanceClaimConflictMember>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GovernanceValidationJob {
    pub v: u16,
    pub job_id: String,
    pub aggregate_id: String,
    pub record_id: String,
    pub claim_type: ClaimType,
    pub due_at_unix_ms: i64,
    pub owner_id: String,
    pub required_evidence_types: Vec<EvidenceType>,
    pub validation_plan_sha256: String,
    pub declared_state: String,
    pub evaluated_at_unix_ms: i64,
    pub temporal_state: GovernanceTemporalState,
    pub due: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GovernanceSemanticListFilter {
    pub as_of_unix_ms: i64,
    pub limit: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GovernanceValidationJobFilter {
    pub as_of_unix_ms: i64,
    pub due_only: bool,
    pub limit: usize,
}

impl GovernanceSemanticProjection {
    /// Validates the complete materialized projection and its digest.
    ///
    /// # Errors
    ///
    /// Returns an error when any field, cross-binding, or digest is invalid.
    pub fn validate(&self) -> Result<(), GovernanceRecordJournalError> {
        super::validation::validate_projection(self)
    }
}

impl GovernanceSemanticAssessment {
    /// Validates an evaluated projection and its explicit evaluation time.
    ///
    /// # Errors
    ///
    /// Returns an error when the projection or derived temporal state is invalid.
    pub fn validate(&self) -> Result<(), GovernanceRecordJournalError> {
        super::validation::validate_assessment(self)
    }
}

impl GovernanceClaimConflictGroup {
    /// Validates a deterministic, non-authoritative conflict candidate group.
    ///
    /// # Errors
    ///
    /// Returns an error when identity, ordering, membership, or semantics drift.
    pub fn validate(&self) -> Result<(), GovernanceRecordJournalError> {
        super::validation::validate_conflict_group(self)
    }
}

impl GovernanceValidationJob {
    /// Validates one deterministic validation-scheduling record.
    ///
    /// # Errors
    ///
    /// Returns an error when identity, timing, evidence, or temporal fields drift.
    pub fn validate(&self) -> Result<(), GovernanceRecordJournalError> {
        super::validation::validate_validation_job(self)
    }
}

impl GovernanceSemanticListFilter {
    /// Validates an explicitly timed, publicly bounded semantic list query.
    ///
    /// # Errors
    ///
    /// Returns an error when the time is negative or the limit is out of bounds.
    pub fn validate(&self) -> Result<(), GovernanceRecordJournalError> {
        super::validation::validate_list_filter(self.as_of_unix_ms, self.limit)
    }
}

impl GovernanceValidationJobFilter {
    /// Validates an explicitly timed, publicly bounded validation-job query.
    ///
    /// # Errors
    ///
    /// Returns an error when the time is negative or the limit is out of bounds.
    pub fn validate(&self) -> Result<(), GovernanceRecordJournalError> {
        super::validation::validate_list_filter(self.as_of_unix_ms, self.limit)
    }
}
