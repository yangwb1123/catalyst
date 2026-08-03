package graphscheduledrelease

import (
	"encoding/json"

	"forgeos/forge-core/internal/graphdispatch"
	"forgeos/forge-core/internal/graphplan"
)

type preparedEvent struct {
	V                        uint16 `json:"v"`
	GraphRunID               string `json:"graph_run_id"`
	Seq                      uint64 `json:"seq"`
	Type                     string `json:"type"`
	GraphID                  string `json:"graph_id"`
	GraphManifestSHA256      string `json:"graph_manifest_sha256"`
	PlanSHA256               string `json:"plan_sha256"`
	SchedulerProtocolVersion uint16 `json:"scheduler_protocol_version"`
	PreparedAtMS             uint64 `json:"prepared_at_ms"`
}

type preparedEventFacts struct {
	Event  preparedEvent
	SHA256 string
	Bytes  uint64
}

func validateGraphSource(value ReleaseControl) error {
	snapshot := value.ControlSnapshot
	if graphdispatch.ValidateControlSnapshot(snapshot) != nil {
		return errInvalidControl
	}
	planJSON, err := graphplan.MarshalPlan(snapshot.Plan)
	if err != nil || len(planJSON) == 0 || len(planJSON) > graphplan.MaxSpecBytes {
		return errInvalidControl
	}
	facts, err := decodePreparedEvent(value.JournalEvents)
	if err != nil || !validGraphRun(value, facts, planJSON) ||
		!validPreparedEvent(value, facts) || !validControlBindings(value, facts) {
		return errInvalidControl
	}
	return nil
}

func decodePreparedEvent(events []json.RawMessage) (preparedEventFacts, error) {
	if len(events) != 1 || len(events[0]) == 0 || len(events[0]) > maxGraphEventBytes {
		return preparedEventFacts{}, errInvalidControl
	}
	event, err := decodeExact[preparedEvent](events[0])
	if err != nil {
		return preparedEventFacts{}, errInvalidControl
	}
	return preparedEventFacts{
		Event: event, SHA256: rawDomainDigest(preparedEventDigestDomain, events[0]),
		Bytes: uint64(len(events[0])),
	}, nil
}

func validGraphRun(value ReleaseControl, facts preparedEventFacts, planJSON []byte) bool {
	run, snapshot := value.GraphRun, value.ControlSnapshot
	return run.V == 1 && run.Status == "awaiting_execution_contract" &&
		validText(run.GraphRunID, 128) && validText(run.GraphID, 128) &&
		isLowerHexDigest(run.SourceSnapshotSHA256) && isLowerHexDigest(run.GraphManifestSHA256) &&
		run.SchedulerProtocolVersion == value.SchedulerProtocolVersion &&
		isLowerHexDigest(run.PlanSHA256) && run.PlanBytes == uint64(len(planJSON)) &&
		run.NodeCount == uint64(len(snapshot.Plan.AuthoredNodeIDs)) &&
		run.WaveCount == uint64(len(snapshot.Plan.Waves)) && !run.ExecutionContractPresent &&
		!run.DispatchRequestPresent && !run.DispatchAuthorityReleased && run.LastEventSeq == 1 &&
		run.JournalBytes == facts.Bytes && validSignedTime(run.CreatedAtMS)
}

func validPreparedEvent(value ReleaseControl, facts preparedEventFacts) bool {
	run, event := value.GraphRun, facts.Event
	return event.V == 1 && event.GraphRunID == run.GraphRunID && event.Seq == 1 &&
		event.Type == "graph_run_prepared" && event.GraphID == run.GraphID &&
		event.GraphManifestSHA256 == run.GraphManifestSHA256 &&
		event.PlanSHA256 == run.PlanSHA256 &&
		event.SchedulerProtocolVersion == run.SchedulerProtocolVersion &&
		event.PreparedAtMS == run.CreatedAtMS && validSignedTime(event.PreparedAtMS)
}

func validControlBindings(value ReleaseControl, facts preparedEventFacts) bool {
	run, snapshot := value.GraphRun, value.ControlSnapshot
	return snapshot.SchedulerProtocolVersion == value.SchedulerProtocolVersion &&
		snapshot.GraphRunVersion == run.V && snapshot.GraphRunID == run.GraphRunID &&
		snapshot.GraphID == run.GraphID && snapshot.SourceSnapshotSHA256 == run.SourceSnapshotSHA256 &&
		snapshot.GraphManifestSHA256 == run.GraphManifestSHA256 &&
		snapshot.CorePlanSHA256 == run.PlanSHA256 && snapshot.LastEventSeq == run.LastEventSeq &&
		snapshot.LastEventSHA256 == facts.SHA256 && !snapshot.ExecutionContractPresent &&
		!snapshot.DispatchAuthorityReleased && snapshot.Plan.GraphID == run.GraphID &&
		snapshot.Plan.PlanSHA256 == run.PlanSHA256 &&
		snapshot.Manifest.Source.SnapshotSHA256 == run.SourceSnapshotSHA256
}
