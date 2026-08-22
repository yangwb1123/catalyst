use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum EffectId {
    #[serde(rename = "repo.read")]
    RepoRead,
    #[serde(rename = "repo.write")]
    RepoWrite,
    #[serde(rename = "process.exec")]
    ProcessExec,
    #[serde(rename = "network.read")]
    NetworkRead,
    #[serde(rename = "network.write")]
    NetworkWrite,
    #[serde(rename = "secrets.read")]
    SecretsRead,
    #[serde(rename = "migration.generate")]
    MigrationGenerate,
    #[serde(rename = "migration.apply")]
    MigrationApply,
    #[serde(rename = "release.plan")]
    ReleasePlan,
    #[serde(rename = "release.execute")]
    ReleaseExecute,
    #[serde(rename = "approval.request")]
    ApprovalRequest,
    #[serde(rename = "approval.decide")]
    ApprovalDecide,
    #[serde(rename = "knowledge.propose")]
    KnowledgePropose,
    #[serde(rename = "knowledge.apply")]
    KnowledgeApply,
    #[serde(rename = "policy.propose")]
    PolicyPropose,
    #[serde(rename = "policy.write")]
    PolicyWrite,
    #[serde(rename = "placement.plan")]
    PlacementPlan,
    #[serde(rename = "target.inventory")]
    TargetInventory,
    #[serde(rename = "target.probe")]
    TargetProbe,
    #[serde(rename = "target.reserve")]
    TargetReserve,
    #[serde(rename = "target.execute")]
    TargetExecute,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GrantScope {
    pub allow: Vec<ScopeClause>,
    pub deny: Vec<ScopeResource>,
    pub effect_id: EffectId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeClause {
    pub resources: Vec<ScopeResource>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "scope_kind", rename_all = "snake_case")]
pub enum ScopeResource {
    Artifact {
        artifact_kind: String,
        artifact_ref: String,
        artifact_sha256: String,
    },
    Command {
        argv: Vec<String>,
        cwd: String,
        environment_sha256: String,
        stdin_bytes: i64,
        stdin_sha256: String,
        timeout_ms: i64,
        tool_snapshot_sha256: String,
    },
    Environment {
        environment_class: EnvironmentClass,
        environment_id: String,
        environment_sha256: String,
    },
    GovernanceObject {
        object_kind: GovernanceObjectKind,
        object_ref: String,
        object_scope_sha256: String,
    },
    NetworkOrigin {
        host: String,
        host_kind: HostKind,
        port: i64,
        scheme: NetworkScheme,
    },
    RepoPath {
        #[serde(rename = "match")]
        path_match: PathMatch,
        path: String,
    },
    SecretRef {
        broker_id: String,
        secret_ref: String,
        version_ref: String,
    },
    Target {
        target_attestation_sha256: String,
        target_id: String,
    },
    TargetQuery {
        query_ref: String,
        query_sha256: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentClass {
    Local,
    Development,
    Test,
    Staging,
    Production,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostKind {
    Dns,
    Ipv4,
    Ipv6,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkScheme {
    Http,
    Https,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernanceObjectKind {
    Approval,
    Knowledge,
    Policy,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PathMatch {
    Exact,
    Subtree,
}
