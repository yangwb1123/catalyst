use serde::{Deserialize, Serialize};

use crate::governance_contract::{EvidenceRecord, Sensitivity};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedCommand {
    pub argv: Vec<String>,
    pub cwd: String,
    pub environment_sha256: String,
    pub stdin_bytes: i64,
    pub stdin_sha256: String,
    pub timeout_ms: Option<i64>,
    pub tool_snapshot_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandEvidenceType {
    GateResult,
    TestRun,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandProducerType {
    Service,
    Tool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommandProducer {
    pub producer_id: String,
    pub producer_type: CommandProducerType,
    pub producer_version: String,
    pub run_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSource {
    pub source_revision: String,
    pub source_tree_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedStream {
    pub bytes: i64,
    pub retained_bytes: i64,
    pub retained_sha256: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedStreams {
    pub combined: ObservedStream,
    pub stderr: ObservedStream,
    pub stdout: ObservedStream,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandTerminationKind {
    Cancelled,
    Exited,
    TimedOut,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommandTermination {
    pub exit_code: Option<i64>,
    pub kind: CommandTerminationKind,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommandObservation {
    pub api_version: String,
    pub canonicalization: String,
    pub command: ObservedCommand,
    pub ended_at_unix_ms: i64,
    pub evidence_type: CommandEvidenceType,
    pub producer: CommandProducer,
    pub source: CommandSource,
    pub started_at_unix_ms: i64,
    pub streams: ObservedStreams,
    pub termination: CommandTermination,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommandEvidenceBinding {
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
pub struct CommandObservationEvidenceRequest {
    pub api_version: String,
    pub binding: CommandEvidenceBinding,
    pub canonicalization: String,
    pub observation: CommandObservation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandObservationEvidenceAdaptation {
    pub canonical_command_json: String,
    pub canonical_evidence_json: String,
    pub canonical_observation_json: String,
    pub canonical_request_json: String,
    pub command_sha256: String,
    pub evidence: EvidenceRecord,
    pub request_sha256: String,
    pub result: &'static str,
    pub source_snapshot_sha256: String,
}
