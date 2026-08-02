// Package graphschedule builds one passive, immutable execution schedule from
// an exact v1 Group Agent Graph control snapshot. It observes no progress and
// grants no execution or dispatch authority.
package graphschedule

import "errors"

const (
	// ExecutionScheduleVersion is the canonical artifact version.
	ExecutionScheduleVersion uint16 = 1
	// ExecutionScheduleProtocolVersion fixes the static scheduler policy.
	ExecutionScheduleProtocolVersion uint16 = 1
	// MaxExecutionScheduleBytes bounds the complete public schedule artifact.
	MaxExecutionScheduleBytes = 1024 * 1024
)

const (
	scheduleDigestDomain = "forge.group-agent-graph-execution-schedule.v1\x00"
	projectLaneDomain    = "forge.group-agent-project-lane.v1\x00"
)

var errInvalidControl = errors.New("invalid Graph execution schedule control")

// ScheduledNode is one node in the exact serial execution order.
type ScheduledNode struct {
	ExecutionOrdinal         uint16   `json:"execution_ordinal"`
	NodeID                   string   `json:"node_id"`
	AuthoredNodeIndex        uint16   `json:"authored_node_index"`
	TopologyWaveIndex        uint16   `json:"topology_wave_index"`
	ProjectLaneSHA256        string   `json:"project_lane_sha256"`
	Attempt                  uint16   `json:"attempt"`
	DirectPredecessorNodeIDs []string `json:"direct_predecessor_node_ids"`
}

// OutcomePolicy fixes how a future successor protocol must interpret terminal
// classifications. It is policy only, never evidence that an outcome occurred.
type OutcomePolicy struct {
	Completed       string `json:"completed"`
	Length          string `json:"length"`
	Uncertainty     string `json:"uncertainty"`
	DispatchUnknown string `json:"dispatch_unknown"`
}

// ExecutionSchedule is Core's content-addressed, effect-free scheduling policy.
type ExecutionSchedule struct {
	V                                uint16          `json:"v"`
	SchedulerProtocolVersion         uint16          `json:"scheduler_protocol_version"`
	ExecutionScheduleProtocolVersion uint16          `json:"execution_schedule_protocol_version"`
	ControlSnapshotSHA256            string          `json:"control_snapshot_sha256"`
	ExpectedLastEventSeq             uint64          `json:"expected_last_event_seq"`
	ExpectedLastEventSHA256          string          `json:"expected_last_event_sha256"`
	GraphRunID                       string          `json:"graph_run_id"`
	GraphID                          string          `json:"graph_id"`
	SourceSnapshotSHA256             string          `json:"source_snapshot_sha256"`
	GraphManifestSHA256              string          `json:"graph_manifest_sha256"`
	CorePlanSHA256                   string          `json:"core_plan_sha256"`
	NodeCount                        uint16          `json:"node_count"`
	WaveCount                        uint16          `json:"wave_count"`
	ExecutionMode                    string          `json:"execution_mode"`
	MaxInFlightNodes                 uint16          `json:"max_in_flight_nodes"`
	SelectionPolicy                  string          `json:"selection_policy"`
	ProgressionPolicy                string          `json:"progression_policy"`
	AttemptPolicy                    string          `json:"attempt_policy"`
	FailurePolicy                    string          `json:"failure_policy"`
	OutcomePolicy                    OutcomePolicy   `json:"outcome_policy"`
	PredecessorSemantics             string          `json:"predecessor_semantics"`
	PredecessorDataflow              string          `json:"predecessor_dataflow"`
	PartialOutputDataflow            bool            `json:"partial_output_dataflow"`
	ReceiptHandling                  string          `json:"receipt_handling"`
	Nodes                            []ScheduledNode `json:"nodes"`
	InitialFrontier                  []string        `json:"initial_frontier"`
	InitialNode                      string          `json:"initial_node"`
	ExecutionContractPresent         bool            `json:"execution_contract_present"`
	DispatchAuthorityReleased        bool            `json:"dispatch_authority_released"`
	ProgressObserved                 bool            `json:"progress_observed"`
	SuccessorAdvanced                bool            `json:"successor_advanced"`
	ScheduleID                       string          `json:"schedule_id"`
	ScheduleSHA256                   string          `json:"schedule_sha256"`
}
