package graphschedule

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
)

type schedulePayload struct {
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
}

func canonicalBytes(value any) ([]byte, error) {
	var buffer bytes.Buffer
	encoder := json.NewEncoder(&buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(value); err != nil {
		return nil, errInvalidControl
	}
	encoded := buffer.Bytes()
	if len(encoded) == 0 || encoded[len(encoded)-1] != '\n' {
		return nil, errInvalidControl
	}
	return append([]byte(nil), encoded[:len(encoded)-1]...), nil
}

func rawDomainDigest(domain string, data []byte) string {
	digest := sha256.New()
	_, _ = digest.Write([]byte(domain))
	_, _ = digest.Write(data)
	return hex.EncodeToString(digest.Sum(nil))
}

func schedulePayloadFrom(value ExecutionSchedule) schedulePayload {
	return schedulePayload{
		V: value.V, SchedulerProtocolVersion: value.SchedulerProtocolVersion,
		ExecutionScheduleProtocolVersion: value.ExecutionScheduleProtocolVersion,
		ControlSnapshotSHA256:            value.ControlSnapshotSHA256,
		ExpectedLastEventSeq:             value.ExpectedLastEventSeq,
		ExpectedLastEventSHA256:          value.ExpectedLastEventSHA256,
		GraphRunID:                       value.GraphRunID, GraphID: value.GraphID,
		SourceSnapshotSHA256: value.SourceSnapshotSHA256,
		GraphManifestSHA256:  value.GraphManifestSHA256, CorePlanSHA256: value.CorePlanSHA256,
		NodeCount: value.NodeCount, WaveCount: value.WaveCount,
		ExecutionMode: value.ExecutionMode, MaxInFlightNodes: value.MaxInFlightNodes,
		SelectionPolicy: value.SelectionPolicy, ProgressionPolicy: value.ProgressionPolicy,
		AttemptPolicy: value.AttemptPolicy,
		FailurePolicy: value.FailurePolicy, OutcomePolicy: value.OutcomePolicy,
		PredecessorSemantics:  value.PredecessorSemantics,
		PredecessorDataflow:   value.PredecessorDataflow,
		PartialOutputDataflow: value.PartialOutputDataflow,
		ReceiptHandling:       value.ReceiptHandling, Nodes: value.Nodes,
		InitialFrontier: value.InitialFrontier, InitialNode: value.InitialNode,
		ExecutionContractPresent:  value.ExecutionContractPresent,
		DispatchAuthorityReleased: value.DispatchAuthorityReleased,
		ProgressObserved:          value.ProgressObserved, SuccessorAdvanced: value.SuccessorAdvanced,
	}
}
