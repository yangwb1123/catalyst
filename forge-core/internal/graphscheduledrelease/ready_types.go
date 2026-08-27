package graphscheduledrelease

import (
	"encoding/json"

	"forgeos/forge-core/internal/graphdispatch"
	"forgeos/forge-core/internal/graphschedule"
	"forgeos/forge-core/internal/graphscheduledcontract"
	"forgeos/forge-core/internal/graphscheduledreconcile"
	"forgeos/forge-core/internal/scheduledterminal"
)

const (
	ReadyReleaseControlVersion  uint16 = 2
	ReadyReleaseControlProtocol uint16 = 2
	ReadyAuthorizationVersion   uint16 = 2
	ReadyAuthorizationProtocol  uint16 = 2
	MaxReadyReleaseControlBytes        = 64 * 1024 * 1024
	MaxReadyAuthorizationBytes         = 1024 * 1024
)

const (
	readyReleaseControlDigestDomain = "forge.group-agent-scheduled-ready-node-dispatch-release-control.v2\x00"
	readyAuthorizationDigestDomain  = "forge.group-agent-scheduled-ready-node-dispatch-authorization.v2\x00"
	readyAuthorizationIDPrefix      = "scheduled-ready-node-dispatch-authorization-"
)

// ReadyReleaseControl binds one exact Core-selected scheduled ready node and
// its source evidence. It carries no consent, claim, send, or current authority.
type ReadyReleaseControl struct {
	V                             uint16                                                `json:"v"`
	SchedulerProtocolVersion      uint16                                                `json:"scheduler_protocol_version"`
	ReleaseControlProtocolVersion uint16                                                `json:"release_control_protocol_version"`
	GraphRun                      GraphRunRecord                                        `json:"graph_run"`
	JournalEvents                 []json.RawMessage                                     `json:"journal_events"`
	ControlSnapshot               graphdispatch.ControlSnapshot                         `json:"control_snapshot"`
	ScheduleRecord                ExecutionScheduleRecord                               `json:"schedule_record"`
	Schedule                      graphschedule.ExecutionSchedule                       `json:"schedule"`
	ProgressSnapshot              graphscheduledreconcile.ProgressSnapshot              `json:"progress_snapshot"`
	ReconcileDecision             graphscheduledreconcile.Decision                      `json:"reconcile_decision"`
	ScheduledContractRecord       ScheduledNodeContractRecord                           `json:"scheduled_contract_record"`
	ScheduledContract             graphscheduledcontract.ScheduledNodeContractCandidate `json:"scheduled_contract"`
	DirectPredecessorReceipts     []scheduledterminal.Receipt                           `json:"direct_predecessor_receipts"`
	PredecessorContentArtifact    *scheduledterminal.Artifact                           `json:"predecessor_content_artifact"`
	ProviderRequest               ScheduledNodeProviderRequestRecord                    `json:"provider_request"`
	ProviderRequestJSON           string                                                `json:"provider_request_json"`
	SnapshotSHA256                string                                                `json:"snapshot_sha256"`
}

type readyReleaseControlPayload struct {
	V                             uint16                                                `json:"v"`
	SchedulerProtocolVersion      uint16                                                `json:"scheduler_protocol_version"`
	ReleaseControlProtocolVersion uint16                                                `json:"release_control_protocol_version"`
	GraphRun                      GraphRunRecord                                        `json:"graph_run"`
	JournalEvents                 []json.RawMessage                                     `json:"journal_events"`
	ControlSnapshot               graphdispatch.ControlSnapshot                         `json:"control_snapshot"`
	ScheduleRecord                ExecutionScheduleRecord                               `json:"schedule_record"`
	Schedule                      graphschedule.ExecutionSchedule                       `json:"schedule"`
	ProgressSnapshot              graphscheduledreconcile.ProgressSnapshot              `json:"progress_snapshot"`
	ReconcileDecision             graphscheduledreconcile.Decision                      `json:"reconcile_decision"`
	ScheduledContractRecord       ScheduledNodeContractRecord                           `json:"scheduled_contract_record"`
	ScheduledContract             graphscheduledcontract.ScheduledNodeContractCandidate `json:"scheduled_contract"`
	DirectPredecessorReceipts     []scheduledterminal.Receipt                           `json:"direct_predecessor_receipts"`
	PredecessorContentArtifact    *scheduledterminal.Artifact                           `json:"predecessor_content_artifact"`
	ProviderRequest               ScheduledNodeProviderRequestRecord                    `json:"provider_request"`
	ProviderRequestJSON           string                                                `json:"provider_request_json"`
}
