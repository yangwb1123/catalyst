use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextPackageBuildRequest {
    pub api_version: String,
    pub budget: ContextBudget,
    pub canonicalization: String,
    pub redactions: Vec<SourceRedaction>,
    pub source_binding: SourceBinding,
    pub sources: Vec<ContextSource>,
    pub task_binding: TaskBinding,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskBinding {
    pub change_id: String,
    pub node_id: String,
    pub phase: String,
    pub project_id: String,
    pub role: String,
    pub run_id: String,
    pub task_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceBinding {
    pub as_of_unix_ms: i64,
    pub policy_sha256: String,
    pub routes_sha256: String,
    pub source_revision: String,
    pub source_tree_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextBudget {
    pub max_content_bytes: u64,
    pub max_snippets: u64,
    pub max_tokens: u64,
    pub tokenizer_id: String,
    pub tokenizer_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceAvailability {
    Available,
    Missing,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextCategory {
    Task,
    Requirement,
    Acceptance,
    HardConstraint,
    Permission,
    Prohibition,
    Fact,
    Decision,
    Assumption,
    Unknown,
    Adr,
    Impact,
    ApiContract,
    DataContract,
    DeploymentContract,
    Code,
    Test,
    Debt,
    Finding,
    RuntimeEvidence,
    History,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclaredLane {
    Instruction,
    TrustedContext,
    UntrustedData,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclaredTrust {
    SystemPolicy,
    UserAuthorized,
    ProjectGovernance,
    GovernanceRecord,
    Untrusted,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceDisposition {
    Allow,
    Deny,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceFreshness {
    Fresh,
    Stale,
    Contested,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InjectionRisk {
    None,
    Suspected,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceClass {
    SystemPolicy,
    UserInstruction,
    Repository,
    Web,
    Log,
    Issue,
    ToolOutput,
    GovernanceRecord,
    Artifact,
    Other,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceTruncationPolicy {
    Forbidden,
    Utf8Prefix,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextSource {
    pub availability: SourceAvailability,
    pub category: ContextCategory,
    pub content: Option<String>,
    pub content_sha256: Option<String>,
    pub declared_lane: DeclaredLane,
    pub declared_trust: DeclaredTrust,
    pub disposition: SourceDisposition,
    pub expires_at_unix_ms: Option<i64>,
    pub freshness: SourceFreshness,
    pub injection_risk: InjectionRisk,
    pub max_bytes: u64,
    pub priority: u64,
    pub required: bool,
    pub source_class: SourceClass,
    pub source_id: String,
    pub source_ref: String,
    pub source_revision: String,
    pub truncation: SourceTruncationPolicy,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRedaction {
    pub ranges: Vec<RedactionRange>,
    pub source_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RedactionRange {
    pub end_byte: u64,
    pub rule_id: String,
    pub start_byte: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectedLane {
    InstructionCandidates,
    TrustedContext,
    UntrustedData,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionReason {
    RequiredSource,
    PrioritySelection,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextSnippet {
    pub category: ContextCategory,
    pub content: String,
    pub declared_lane: DeclaredLane,
    pub declared_trust: DeclaredTrust,
    pub delimiter: String,
    pub instruction_allowed: bool,
    pub lane: ProjectedLane,
    pub normalization: String,
    pub projected_content_sha256: String,
    pub required: bool,
    pub selection_reason: SelectionReason,
    pub snippet_sha256: String,
    pub source_class: SourceClass,
    pub source_content_sha256: String,
    pub source_id: String,
    pub source_ref: String,
    pub source_revision: String,
    pub truncation: Option<SnippetTruncation>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SnippetTruncation {
    pub original_redacted_bytes: u64,
    pub reason: String,
    pub retained_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextLanes {
    pub instruction_candidates: Vec<ContextSnippet>,
    pub trusted_context: Vec<ContextSnippet>,
    pub untrusted_data: Vec<ContextSnippet>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OmissionReason {
    Missing,
    Denied,
    Stale,
    Contested,
    UnknownFreshness,
    Expired,
    QuarantinedPromptInjection,
    SourceLimitExceeded,
    SnippetBudgetExceeded,
    ContentBudgetExceeded,
    TokenBudgetExceeded,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextOmission {
    pub reason: OmissionReason,
    pub source_id: String,
    pub source_ref: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RedactionReceipt {
    pub ranges: Vec<RedactionRange>,
    pub source_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextAccounting {
    pub actual_tokens: u64,
    pub candidate_count: u64,
    pub content_bytes: u64,
    pub omitted_source_count: u64,
    pub redacted_range_count: u64,
    pub selected_snippet_count: u64,
    pub truncated_snippet_count: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextFreshness {
    pub evaluated_at_unix_ms: i64,
    pub expires_at_unix_ms: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextPackage {
    pub accounting: ContextAccounting,
    pub api_version: String,
    pub assembly_mode: String,
    pub budget: ContextBudget,
    pub cache_key_sha256: String,
    pub canonicalization: String,
    pub context_sha256: String,
    pub freshness: ContextFreshness,
    pub lanes: ContextLanes,
    pub omissions: Vec<ContextOmission>,
    pub projection_sha256: String,
    pub redaction_receipts: Vec<RedactionReceipt>,
    pub request_sha256: String,
    pub result: String,
    pub source_binding: SourceBinding,
    pub task_binding: TaskBinding,
}
