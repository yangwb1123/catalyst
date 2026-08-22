use serde::{Deserialize, Serialize};

use super::{EffectId, EnvironmentClass, GrantScope};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityGrant {
    pub api_version: String,
    pub approval_refs: Vec<ApprovalRef>,
    pub authority_proof: AuthorityProof,
    pub bindings: GrantBindings,
    pub budget: GrantBudget,
    pub canonicalization: String,
    pub capability: CapabilityIdentity,
    pub effect_vocabulary_sha256: String,
    pub grant_id: String,
    pub grant_sha256: String,
    pub issuance_phase: IssuancePhase,
    pub kind: String,
    pub scope: GrantScope,
    pub separation_of_duty: SeparationOfDuty,
    pub subject: Principal,
    pub task_binding: GrantTaskBinding,
    pub usage_policy: UsagePolicy,
    pub validity: GrantValidity,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Principal {
    pub authority_domain: String,
    pub principal_id: String,
    pub principal_type: PrincipalType,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Issuer {
    pub authority_class: AuthorityClass,
    pub authority_domain: String,
    pub principal_id: String,
    pub principal_type: PrincipalType,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalType {
    Agent,
    Service,
    Human,
    Operator,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityClass {
    ForgeosKernel,
    ExternalOperator,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityProof {
    pub issuer: Issuer,
    pub key_id: String,
    pub proof_base64url: String,
    pub proof_profile_id: String,
    pub proof_profile_sha256: String,
    pub trust_domain: String,
    pub trust_epoch: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GrantTaskBinding {
    pub attempt_id: Option<String>,
    pub change_id: String,
    pub environment_class: EnvironmentClass,
    pub environment_id: String,
    pub node_id: String,
    pub project_id: String,
    pub role: String,
    pub run_id: String,
    pub target_id: Option<String>,
    pub task_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityIdentity {
    pub capability_contract_sha256: String,
    pub capability_id: String,
    pub capability_version: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GrantBindings {
    pub context_sha256: String,
    pub grant_request_sha256: String,
    pub impact_sha256: Option<String>,
    pub plan_sha256: Option<String>,
    pub policy_sha256: String,
    pub risk_sha256: Option<String>,
    pub source_revision: String,
    pub source_tree_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GrantBudget {
    pub max_calls: i64,
    pub max_cost_usd_micros: i64,
    pub max_input_tokens: i64,
    pub max_network_bytes: i64,
    pub max_output_bytes: i64,
    pub max_output_tokens: i64,
    pub timeout_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalRef {
    pub approval_id: String,
    pub approval_sha256: String,
    pub authority_domain: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SeparationOfDuty {
    pub requester: Principal,
    pub required_distinctions: Vec<RequiredDistinction>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RequiredDistinction {
    IssuerNotRequester,
    IssuerNotSubject,
    ApproverNotRequester,
    ApproverNotSubject,
    ApproverNotIssuer,
}

impl RequiredDistinction {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::IssuerNotRequester => "issuer_not_requester",
            Self::IssuerNotSubject => "issuer_not_subject",
            Self::ApproverNotRequester => "approver_not_requester",
            Self::ApproverNotSubject => "approver_not_subject",
            Self::ApproverNotIssuer => "approver_not_issuer",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IssuancePhase {
    BootstrapPlanning,
    PlanFinalization,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UsagePolicy {
    pub atomic_reservation_required: bool,
    pub concurrent_use: ConcurrentUse,
    pub consumption_mode: ConsumptionMode,
    pub replay: ReplayPolicy,
    pub uncertain_effect: UncertainEffectPolicy,
    pub usage_ledger_required: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConcurrentUse {
    Forbidden,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsumptionMode {
    SingleUse,
    BoundedCalls,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayPolicy {
    ReceiptOnlyNoReexecute,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UncertainEffectPolicy {
    Quarantine,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GrantValidity {
    pub expires_at_unix_ms: i64,
    pub issued_at_unix_ms: i64,
    pub not_before_unix_ms: i64,
    pub transferable: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RequestedUsage {
    pub call_count: i64,
    pub cost_usd_micros: i64,
    pub input_tokens: i64,
    pub network_bytes: i64,
    pub output_bytes: i64,
    pub output_tokens: i64,
    pub timeout_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RequestedAction {
    pub effect_id: EffectId,
    pub resources: Vec<super::ScopeResource>,
    pub usage: RequestedUsage,
}
