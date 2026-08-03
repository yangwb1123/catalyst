package graphscheduledrelease

import (
	"encoding/json"
	"testing"

	"forgeos/forge-core/internal/graphdispatch"
	"forgeos/forge-core/internal/graphplan"
	"forgeos/forge-core/internal/graphschedule"
	"forgeos/forge-core/internal/graphscheduledcontract"
)

func controlForTest(
	t *testing.T,
	snapshot graphdispatch.ControlSnapshot,
	schedule graphschedule.ExecutionSchedule,
	contract graphscheduledcontract.ScheduledNodeContractCandidate,
	preparedJSON []byte,
	body []byte,
) ReleaseControl {
	t.Helper()
	control := ReleaseControl{
		V: 1, SchedulerProtocolVersion: 1, ReleaseControlProtocolVersion: 1,
		GraphRun:        graphRunRecordTest(t, snapshot, preparedJSON),
		JournalEvents:   []json.RawMessage{append([]byte(nil), preparedJSON...)},
		ControlSnapshot: snapshot, ScheduleRecord: scheduleRecordTest(t, schedule), Schedule: schedule,
		ScheduledContractRecord: contractRecordTest(t, contract), ScheduledContract: contract,
		ProviderRequest: providerRecordTest(t, contract, body), ProviderRequestJSON: string(body),
	}
	control.SnapshotSHA256 = mustDomainDigestTest(t, releaseControlDigestDomain, releasePayload(control))
	return control
}

func graphRunRecordTest(
	t *testing.T,
	snapshot graphdispatch.ControlSnapshot,
	preparedJSON []byte,
) GraphRunRecord {
	t.Helper()
	planJSON, err := graphplan.MarshalPlan(snapshot.Plan)
	if err != nil {
		t.Fatalf("marshal plan: %v", err)
	}
	return GraphRunRecord{
		V: 1, GraphRunID: snapshot.GraphRunID, GraphID: snapshot.GraphID,
		Status: "awaiting_execution_contract", SourceSnapshotSHA256: snapshot.SourceSnapshotSHA256,
		GraphManifestSHA256: snapshot.GraphManifestSHA256, SchedulerProtocolVersion: 1,
		PlanSHA256: snapshot.CorePlanSHA256, PlanBytes: uint64(len(planJSON)),
		NodeCount: uint64(len(snapshot.Plan.AuthoredNodeIDs)), WaveCount: uint64(len(snapshot.Plan.Waves)),
		LastEventSeq: 1, JournalBytes: uint64(len(preparedJSON)), CreatedAtMS: 73,
	}
}

func scheduleRecordTest(t *testing.T, value graphschedule.ExecutionSchedule) ExecutionScheduleRecord {
	t.Helper()
	encoded, err := graphschedule.MarshalSchedule(value)
	if err != nil {
		t.Fatalf("marshal schedule: %v", err)
	}
	return ExecutionScheduleRecord{
		V: 1, ScheduleID: value.ScheduleID, GraphRunID: value.GraphRunID, GraphID: value.GraphID,
		ControlSnapshotSHA256: value.ControlSnapshotSHA256, ScheduleSHA256: value.ScheduleSHA256,
		ScheduleBytes: uint64(len(encoded)), NodeCount: uint64(value.NodeCount), WaveCount: uint64(value.WaveCount),
		ExpectedLastEventSeq:    value.ExpectedLastEventSeq,
		ExpectedLastEventSHA256: value.ExpectedLastEventSHA256, CreatedAtMS: 74,
	}
}

func contractRecordTest(
	t *testing.T,
	value graphscheduledcontract.ScheduledNodeContractCandidate,
) ScheduledNodeContractRecord {
	t.Helper()
	encoded, err := graphscheduledcontract.MarshalCandidate(value)
	if err != nil {
		t.Fatalf("marshal contract: %v", err)
	}
	return ScheduledNodeContractRecord{
		V: 2, ContractID: value.ContractID, GraphRunID: value.GraphRunID,
		ScheduleID: value.ScheduleID, NodeID: value.Node.NodeID,
		ExecutionOrdinal: uint64(value.Node.ExecutionOrdinal), Attempt: value.Node.Attempt,
		ControlSnapshotSHA256: value.ControlSnapshotSHA256, ScheduleSHA256: value.ScheduleSHA256,
		ContractSHA256: value.ContractSHA256, ContractBytes: uint64(len(encoded)),
		RequestID: value.Request.RequestID, RequestSHA256: value.Request.RequestSHA256,
		ProjectLaneSHA256:       value.Node.ProjectLaneSHA256,
		ExpectedLastEventSeq:    value.ExpectedLastEventSeq,
		ExpectedLastEventSHA256: value.ExpectedLastEventSHA256, CreatedAtMS: 75,
	}
}
