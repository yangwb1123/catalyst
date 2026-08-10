use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalType {
    Agent,
    Human,
    Operator,
    Service,
    Tool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Principal {
    pub authority_domain: String,
    pub principal_id: String,
    pub principal_type: PrincipalType,
    pub role: String,
    pub run_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecordMetadata {
    pub aggregate_id: String,
    pub context_sha256: String,
    pub created_at_unix_ms: i64,
    pub created_by: Principal,
    pub policy_sha256: String,
    pub project_id: String,
    pub record_id: String,
    pub scope: String,
    pub sequence: i64,
    pub source_revision: String,
    pub source_tree_sha256: String,
    pub supersedes_record_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Integrity {
    pub canonical_sha256: String,
    pub canonicalization: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum EvidenceRecordKind {
    #[serde(rename = "EvidenceRecord")]
    EvidenceRecord,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum KnowledgeClaimKind {
    #[serde(rename = "KnowledgeClaim")]
    KnowledgeClaim,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceType {
    Artifact,
    ExternalSource,
    GateResult,
    HumanAttestation,
    RepoLocator,
    RuntimeMetric,
    TestRun,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectorType {
    Human,
    Operator,
    Service,
    Tool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Collector {
    pub collector_id: String,
    pub collector_type: CollectorType,
    pub collector_version: String,
    pub parameters_sha256: String,
    pub run_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotType {
    Artifact,
    External,
    Repository,
    Runtime,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSnapshot {
    pub snapshot_id: String,
    pub snapshot_sha256: String,
    pub snapshot_type: SnapshotType,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocatorType {
    Artifact,
    Attestation,
    Command,
    External,
    Metric,
    Repo,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceLocator {
    pub content_sha256: String,
    pub exit_code: Option<i64>,
    pub line_end: Option<i64>,
    pub line_start: Option<i64>,
    pub locator_ref: String,
    pub locator_type: LocatorType,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Directness {
    Attested,
    Derived,
    Direct,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Sensitivity {
    Confidential,
    Internal,
    Public,
    Restricted,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceTrust {
    Authoritative,
    Controlled,
    Observed,
    Untrusted,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentRole {
    TrustedControl,
    UntrustedData,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceSpec {
    pub artifact_sha256: Option<String>,
    pub collector: Collector,
    pub content_role: ContentRole,
    pub directness: Directness,
    pub evidence_type: EvidenceType,
    pub locator: EvidenceLocator,
    pub observed_at_unix_ms: i64,
    pub sensitivity: Sensitivity,
    pub source_snapshot: SourceSnapshot,
    pub source_trust: SourceTrust,
    pub subjects: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceState {
    Expired,
    Invalid,
    Unavailable,
    Valid,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceStatus {
    pub reason_codes: Vec<String>,
    pub state: EvidenceState,
    pub valid_from_unix_ms: i64,
    pub valid_until_unix_ms: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRecord {
    pub api_version: String,
    pub integrity: Integrity,
    pub kind: EvidenceRecordKind,
    pub metadata: RecordMetadata,
    pub spec: EvidenceSpec,
    pub status: EvidenceStatus,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimType {
    Assumption,
    Constraint,
    Decision,
    Fact,
    Hypothesis,
    Inference,
    Lesson,
    Proposal,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimObjectType {
    ArtifactRef,
    Boolean,
    Integer,
    Null,
    String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ClaimObjectValue {
    Boolean(bool),
    Integer(i64),
    Null,
    String(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimOwner {
    pub principal_id: String,
    pub principal_type: PrincipalType,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationPlan {
    pub due_at_unix_ms: i64,
    pub impact_if_false: String,
    pub method: String,
    pub owner_id: String,
    pub required_evidence_types: Vec<EvidenceType>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionAuthority {
    pub adr_ref: String,
    pub approval_ref: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimSpec {
    pub claim_type: ClaimType,
    pub confidence_micros: Option<i64>,
    pub contradicting_evidence_record_ids: Vec<String>,
    pub decision_authority: Option<DecisionAuthority>,
    pub derived_from_claim_record_ids: Vec<String>,
    pub object_type: ClaimObjectType,
    pub object_value: ClaimObjectValue,
    pub owner: ClaimOwner,
    pub predicate: String,
    pub queue_ref: Option<String>,
    pub reasoning: String,
    pub review_by_unix_ms: Option<i64>,
    pub subject: String,
    pub supporting_evidence_record_ids: Vec<String>,
    pub validation_plan: Option<ValidationPlan>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimState {
    Accepted,
    AcceptedRisk,
    Active,
    Adopted,
    Candidate,
    Confirmed,
    Contested,
    Deprecated,
    Draft,
    Expired,
    Investigating,
    Invalidated,
    Observed,
    Open,
    Promoted,
    Proposed,
    Rejected,
    Repeated,
    Resolved,
    Retracted,
    Retired,
    Stale,
    Submitted,
    Superseded,
    Supported,
    Testing,
    Validated,
    Waived,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimStatus {
    pub reason_codes: Vec<String>,
    pub state: ClaimState,
    pub valid_from_unix_ms: i64,
    pub valid_until_unix_ms: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeClaim {
    pub api_version: String,
    pub integrity: Integrity,
    pub kind: KnowledgeClaimKind,
    pub metadata: RecordMetadata,
    pub spec: ClaimSpec,
    pub status: ClaimStatus,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum GovernanceRecord {
    Evidence(EvidenceRecord),
    Claim(KnowledgeClaim),
}
