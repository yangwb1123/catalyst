use serde::{Deserialize, Serialize};

use crate::governance_contract::GovernanceRecord;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum GovernanceRecordKind {
    #[serde(rename = "EvidenceRecord")]
    EvidenceRecord,
    #[serde(rename = "KnowledgeClaim")]
    KnowledgeClaim,
}

impl GovernanceRecordKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EvidenceRecord => "EvidenceRecord",
            Self::KnowledgeClaim => "KnowledgeClaim",
        }
    }
}

impl From<&GovernanceRecord> for GovernanceRecordKind {
    fn from(record: &GovernanceRecord) -> Self {
        match record {
            GovernanceRecord::Evidence(_) => Self::EvidenceRecord,
            GovernanceRecord::Claim(_) => Self::KnowledgeClaim,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppendGovernanceRecordBatch {
    pub v: u16,
    pub batch_id: String,
    pub request_sha256: String,
    pub record_set_sha256: String,
    pub canonical_record_set_json: String,
    pub idempotency_key: String,
    pub appended_at_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernanceRecordAppendDisposition {
    Stored,
    ExactReplay,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GovernanceRecordAppendReceipt {
    pub v: u16,
    pub batch_id: String,
    pub request_sha256: String,
    pub record_set_sha256: String,
    pub record_count: usize,
    pub record_ids: Vec<String>,
    pub appended_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AppendGovernanceRecordBatchResult {
    pub v: u16,
    pub disposition: GovernanceRecordAppendDisposition,
    pub receipt: GovernanceRecordAppendReceipt,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GovernanceRecordMetadata {
    pub v: u16,
    pub batch_id: String,
    pub batch_ordinal: usize,
    pub record_id: String,
    pub record_kind: GovernanceRecordKind,
    pub aggregate_id: String,
    pub sequence: i64,
    pub canonical_sha256: String,
    pub canonical_record_bytes: usize,
    pub created_at_unix_ms: i64,
    pub appended_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GovernanceRecordInspection {
    pub v: u16,
    pub metadata: GovernanceRecordMetadata,
    pub canonical_record_json: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GovernanceStructuralHead {
    pub v: u16,
    pub record_kind: GovernanceRecordKind,
    pub aggregate_id: String,
    pub record_id: String,
    pub sequence: i64,
    pub canonical_sha256: String,
    pub updated_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GovernanceRecordListFilter {
    pub record_kind: Option<GovernanceRecordKind>,
    pub aggregate_id: Option<String>,
    pub limit: usize,
    pub include_record: bool,
}
