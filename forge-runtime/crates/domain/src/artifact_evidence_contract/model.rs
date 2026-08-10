use serde::{Deserialize, Serialize};

use crate::governance_contract::{EvidenceRecord, Sensitivity};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactProvenanceRecord {
    #[serde(rename = "_format")]
    pub format: String,
    pub agent: String,
    pub created_at: String,
    pub model: String,
    pub path: String,
    pub phase: String,
    pub prompt_sha256: String,
    pub run_id: String,
    pub sha256: String,
    pub size: i64,
    pub workflow: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactEvidenceBinding {
    pub aggregate_id: String,
    pub context_sha256: String,
    pub policy_sha256: String,
    pub project_id: String,
    pub scope: String,
    pub sensitivity: Sensitivity,
    pub sequence: i64,
    pub source_revision: String,
    pub source_tree_sha256: String,
    pub subjects: Vec<String>,
    pub supersedes_record_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactEvidenceRequest {
    pub api_version: String,
    pub artifact: ArtifactProvenanceRecord,
    pub binding: ArtifactEvidenceBinding,
    pub canonicalization: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactEvidenceAdaptation {
    pub canonical_evidence_json: String,
    pub canonical_request_json: String,
    pub evidence: EvidenceRecord,
    pub request_sha256: String,
    pub result: &'static str,
    pub source_sha256: String,
}
