package graphscheduledreconcile

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
)

type snapshotPayload struct {
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
}

type decisionPayload struct {
	V                       uint16  `json:"v"`
	ProgressProtocolVersion uint16  `json:"progress_protocol_version"`
	GraphRunID              string  `json:"graph_run_id"`
	ScheduleID              string  `json:"schedule_id"`
	ScheduleSHA256          string  `json:"schedule_sha256"`
	SnapshotSHA256          string  `json:"snapshot_sha256"`
	Disposition             string  `json:"disposition"`
	NextExecutionOrdinal    *uint16 `json:"next_execution_ordinal"`
	NextNodeID              *string `json:"next_node_id"`
}

func canonicalBytes(value any) ([]byte, error) {
	var buffer bytes.Buffer
	encoder := json.NewEncoder(&buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(value); err != nil {
		return nil, errInvalidSnapshot
	}
	encoded := buffer.Bytes()
	if len(encoded) == 0 || encoded[len(encoded)-1] != '\n' {
		return nil, errInvalidSnapshot
	}
	return append([]byte(nil), encoded[:len(encoded)-1]...), nil
}

func domainDigest(domain string, value any) (string, error) {
	encoded, err := canonicalBytes(value)
	if err != nil {
		return "", errInvalidSnapshot
	}
	digest := sha256.New()
	_, _ = digest.Write([]byte(domain))
	_, _ = digest.Write(encoded)
	return hex.EncodeToString(digest.Sum(nil)), nil
}

func payloadFromSnapshot(value ProgressSnapshot) snapshotPayload {
	return snapshotPayload{
		V: value.V, ProgressProtocolVersion: value.ProgressProtocolVersion,
		GraphRunID: value.GraphRunID, GraphID: value.GraphID,
		ScheduleID: value.ScheduleID, ScheduleSHA256: value.ScheduleSHA256,
		NodeCount: value.NodeCount, ExecutionMode: value.ExecutionMode,
		MaxInFlightNodes: value.MaxInFlightNodes, ProgressionPolicy: value.ProgressionPolicy,
		AttemptPolicy: value.AttemptPolicy, FailurePolicy: value.FailurePolicy, Nodes: value.Nodes,
	}
}

func payloadFromDecision(value Decision) decisionPayload {
	return decisionPayload{
		V: value.V, ProgressProtocolVersion: value.ProgressProtocolVersion,
		GraphRunID: value.GraphRunID, ScheduleID: value.ScheduleID,
		ScheduleSHA256: value.ScheduleSHA256, SnapshotSHA256: value.SnapshotSHA256,
		Disposition: value.Disposition, NextExecutionOrdinal: value.NextExecutionOrdinal,
		NextNodeID: value.NextNodeID,
	}
}
