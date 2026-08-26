// Package graphscheduledreconcile validates one durable, content-free serial
// Graph progress snapshot and emits a passive reconciliation decision. It
// performs no persistence, provider, workspace, consent, or dispatch effect.
package graphscheduledreconcile

import "errors"

const (
	// ProgressProtocolVersion is the exact snapshot and decision protocol.
	ProgressProtocolVersion uint16 = 1
	// MaxProgressSnapshotBytes bounds the complete private snapshot.
	MaxProgressSnapshotBytes = 64 * 1024
)

const (
	snapshotDigestDomain = "forge.scheduled-graph-progress-snapshot.v1\x00"
	decisionDigestDomain = "forge.scheduled-graph-reconcile-decision.v1\x00"
	scheduleIDPrefix     = "graph-execution-schedule-"
	candidateIDPrefix    = "scheduled-node-contract-"
	providerIDPrefix     = "scheduled-node-provider-request-"
)

var errInvalidSnapshot = errors.New("invalid scheduled Graph progress snapshot")

// ProgressNode is one schedule ordinal and its content-free durable evidence.
type ProgressNode struct {
	ExecutionOrdinal      uint16  `json:"execution_ordinal"`
	NodeID                string  `json:"node_id"`
	Attempt               uint16  `json:"attempt"`
	CandidateID           *string `json:"candidate_id"`
	CandidateSHA256       *string `json:"candidate_sha256"`
	ProviderRequestID     *string `json:"provider_request_id"`
	PreparedRequestSHA256 *string `json:"prepared_request_sha256"`
	LifecycleStatus       *string `json:"lifecycle_status"`
	TerminalOutcome       *string `json:"terminal_outcome"`
	TerminalReceiptSHA256 *string `json:"terminal_receipt_sha256"`
}

// ProgressSnapshot is Rust's exact, content-free, single-SQLite-snapshot view.
type ProgressSnapshot struct {
	V                       uint16         `json:"v"`
	ProgressProtocolVersion uint16         `json:"progress_protocol_version"`
	GraphRunID              string         `json:"graph_run_id"`
	GraphID                 string         `json:"graph_id"`
	ScheduleID              string         `json:"schedule_id"`
	ScheduleSHA256          string         `json:"schedule_sha256"`
	NodeCount               uint16         `json:"node_count"`
	ExecutionMode           string         `json:"execution_mode"`
	MaxInFlightNodes        uint16         `json:"max_in_flight_nodes"`
	ProgressionPolicy       string         `json:"progression_policy"`
	AttemptPolicy           string         `json:"attempt_policy"`
	FailurePolicy           string         `json:"failure_policy"`
	Nodes                   []ProgressNode `json:"nodes"`
	SnapshotSHA256          string         `json:"snapshot_sha256"`
}

// Decision is Core's content-addressed, zero-effect serial classification.
type Decision struct {
	V                       uint16  `json:"v"`
	ProgressProtocolVersion uint16  `json:"progress_protocol_version"`
	GraphRunID              string  `json:"graph_run_id"`
	ScheduleID              string  `json:"schedule_id"`
	ScheduleSHA256          string  `json:"schedule_sha256"`
	SnapshotSHA256          string  `json:"snapshot_sha256"`
	Disposition             string  `json:"disposition"`
	NextExecutionOrdinal    *uint16 `json:"next_execution_ordinal"`
	NextNodeID              *string `json:"next_node_id"`
	DecisionSHA256          string  `json:"decision_sha256"`
}
