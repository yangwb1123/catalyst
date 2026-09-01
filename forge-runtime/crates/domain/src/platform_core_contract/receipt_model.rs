use serde::{Deserialize, Serialize};

use super::{ActorRef, ArtifactRef, EntityRef, RecordRef, ScopeRef, wire_enum::open_wire_enum};

open_wire_enum!(WorkItemState {
    Draft => "draft",
    Planned => "planned",
    AwaitingApproval => "awaiting_approval",
    Ready => "ready",
    Dispatched => "dispatched",
    Running => "running",
    Verifying => "verifying",
    Completed => "completed",
    Blocked => "blocked",
    Failed => "failed",
    Uncertain => "uncertain",
    Cancelled => "cancelled",
});

open_wire_enum!(AttemptState {
    Requested => "requested",
    Accepted => "accepted",
    Starting => "starting",
    Running => "running",
    Interrupted => "interrupted",
    Completed => "completed",
    Failed => "failed",
    Uncertain => "uncertain",
});

open_wire_enum!(ActionState {
    Requested => "requested",
    AwaitingApproval => "awaiting_approval",
    Approved => "approved",
    Started => "started",
    Finished => "finished",
    Rejected => "rejected",
    Failed => "failed",
    Cancelled => "cancelled",
    Uncertain => "uncertain",
});

open_wire_enum!(VerificationStatus {
    Pass => "pass",
    Fail => "fail",
    Inconclusive => "inconclusive",
    NotExecuted => "not_executed",
});

open_wire_enum!(VerificationApplicability {
    Applicable => "applicable",
    NotApplicable => "not_applicable",
});

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutorDescriptor {
    pub actor_ref: ActorRef,
    pub adapter_id: String,
    pub adapter_version: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedUsage {
    pub cost_usd_micros: i64,
    pub elapsed_ms: i64,
    pub input_tokens: i64,
    pub model_calls: i64,
    pub network_bytes: i64,
    pub output_bytes: i64,
    pub output_tokens: i64,
    pub tool_calls: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EventRange {
    pub aggregate_ref: EntityRef,
    pub first_event_id: String,
    pub first_sequence: i64,
    pub last_event_id: String,
    pub last_sequence: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionReceipt {
    pub approval_ref: Option<RecordRef>,
    pub attempt_ref: EntityRef,
    pub canonicalization: String,
    pub ended_at_unix_ms: i64,
    pub event_range: Option<EventRange>,
    pub execution_receipt_version: i64,
    pub executor: ExecutorDescriptor,
    pub grant_ref: Option<RecordRef>,
    pub input_artifact_refs: Vec<ArtifactRef>,
    pub observed_usage: ObservedUsage,
    pub output_artifact_refs: Vec<ArtifactRef>,
    pub reason_codes: Vec<String>,
    pub receipt_id: String,
    pub scope_ref: ScopeRef,
    pub session_ref: EntityRef,
    pub source_snapshot_ref: EntityRef,
    pub started_at_unix_ms: i64,
    pub terminal_state: AttemptState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationCheckRequest {
    pub check_id: String,
    pub check_name: String,
    pub declared_required: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationRequest {
    pub canonicalization: String,
    pub checks: Vec<VerificationCheckRequest>,
    pub input_artifact_ref: ArtifactRef,
    pub requested_at_unix_ms: i64,
    pub requested_by: ActorRef,
    pub scope_ref: ScopeRef,
    pub verification_id: String,
    pub verification_request_version: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationCheckResult {
    pub applicability: VerificationApplicability,
    pub applicability_reason: Option<String>,
    pub check_id: String,
    pub evidence_refs: Vec<RecordRef>,
    pub reason_codes: Vec<String>,
    pub status: VerificationStatus,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationReceipt {
    pub canonicalization: String,
    pub ended_at_unix_ms: i64,
    pub input_artifact_ref: ArtifactRef,
    pub overall_status: VerificationStatus,
    pub produced_by: ActorRef,
    pub receipt_id: String,
    pub request_sha256: String,
    pub results: Vec<VerificationCheckResult>,
    pub scope_ref: ScopeRef,
    pub started_at_unix_ms: i64,
    pub verification_id: String,
    pub verification_receipt_version: i64,
}
