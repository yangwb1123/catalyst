use serde::{Deserialize, Serialize};

use crate::governance_contract::{ClaimObjectType, ClaimObjectValue, ClaimState, Integrity};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CognitiveAtomKind {
    CognitiveAtom,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AtomType {
    Assumption,
    Constraint,
    Decision,
    Fact,
    Hypothesis,
    Inference,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Hardness {
    None,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionMode {
    Shadow,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AtomMetadata {
    pub atom_id: String,
    pub context_sha256: String,
    pub policy_sha256: String,
    pub project_id: String,
    pub scope: String,
    pub source_revision: String,
    pub source_tree_sha256: String,
    pub task_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AtomSource {
    pub canonical_sha256: String,
    pub claim_aggregate_id: String,
    pub claim_record_id: String,
    pub claim_sequence: i64,
    pub closure_byte_count: i64,
    pub closure_record_count: i64,
    pub closure_sha256: String,
    pub record_kind: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Proposition {
    pub object_type: ClaimObjectType,
    pub object_value: ClaimObjectValue,
    pub predicate: String,
    pub subject: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Validity {
    pub valid_from_unix_ms: i64,
    pub valid_until_unix_ms: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CognitiveAtomSpec {
    pub atom_type: AtomType,
    pub authority_ref: Option<String>,
    pub contradicting_evidence_record_ids: Vec<String>,
    pub derived_from_claim_record_ids: Vec<String>,
    pub epistemic_state: ClaimState,
    pub hardness: Hardness,
    pub instruction_allowed: bool,
    pub projection_confidence_micros: Option<i64>,
    pub projection_mode: ProjectionMode,
    pub proposition: Proposition,
    pub supporting_evidence_record_ids: Vec<String>,
    pub validity: Validity,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CognitiveAtom {
    pub api_version: String,
    pub integrity: Integrity,
    pub kind: CognitiveAtomKind,
    pub metadata: AtomMetadata,
    pub source: AtomSource,
    pub spec: CognitiveAtomSpec,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CognitiveAtomProjection {
    pub atom_set_sha256: String,
    pub atoms: Vec<CognitiveAtom>,
    pub canonical_atom_set_json: String,
}
