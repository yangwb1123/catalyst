use serde::{Deserialize, Serialize};

use crate::governance_contract::{EvidenceRecord, Sensitivity};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvolveRepoContent {
    pub bytes: i64,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvolveRepoLocator {
    pub detail: String,
    pub line: i64,
    pub path: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvolveProducerType {
    Service,
    Tool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvolveProducer {
    pub parameters_sha256: String,
    pub producer_id: String,
    pub producer_type: EvolveProducerType,
    pub producer_version: String,
    pub run_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvolveScanDepth {
    Advisory,
    Opportunistic,
    Standard,
    Thorough,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvolveDimension {
    ArchitectureDrift,
    Code,
    Dependencies,
    Performance,
    Security,
    TestCoverage,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvolveLocatorRelation {
    Clear,
    Finding,
    Opportunity,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvolveScanContext {
    pub contract: String,
    pub depth: EvolveScanDepth,
    pub dimension: EvolveDimension,
    pub opportunity_id: Option<String>,
    pub relation: EvolveLocatorRelation,
    pub report_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvolveRepoSource {
    pub source_revision: String,
    pub source_tree_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvolveRepoLocatorObservation {
    pub api_version: String,
    pub canonicalization: String,
    pub content: EvolveRepoContent,
    pub locator: EvolveRepoLocator,
    pub observed_at_unix_ms: i64,
    pub producer: EvolveProducer,
    pub scan_context: EvolveScanContext,
    pub source: EvolveRepoSource,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvolveRepoLocatorEvidenceBinding {
    pub aggregate_id: String,
    pub context_sha256: String,
    pub policy_sha256: String,
    pub project_id: String,
    pub scope: String,
    pub sensitivity: Sensitivity,
    pub sequence: i64,
    pub subjects: Vec<String>,
    pub supersedes_record_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvolveRepoLocatorEvidenceRequest {
    pub api_version: String,
    pub binding: EvolveRepoLocatorEvidenceBinding,
    pub canonicalization: String,
    pub observation: EvolveRepoLocatorObservation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvolveRepoLocatorEvidenceAdaptation {
    pub canonical_evidence_json: String,
    pub canonical_locator_json: String,
    pub canonical_observation_json: String,
    pub canonical_request_json: String,
    pub evidence: EvidenceRecord,
    pub locator_sha256: String,
    pub request_sha256: String,
    pub result: &'static str,
    pub source_snapshot_sha256: String,
}
