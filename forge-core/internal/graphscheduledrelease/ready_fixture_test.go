package graphscheduledrelease

import (
	"bytes"
	"encoding/json"
	"testing"

	"forgeos/forge-core/internal/graphscheduledcontract"
	"forgeos/forge-core/internal/graphscheduledreconcile"
	"forgeos/forge-core/internal/scheduledterminal"
)

func validReadyInitialFixture(t *testing.T) (ReadyReleaseControl, []byte) {
	t.Helper()
	return validReadyFixtureAt(t, 0, "")
}

func validReadyZeroDirectSuccessorFixture(t *testing.T) (ReadyReleaseControl, []byte) {
	t.Helper()
	return validReadyFixtureAt(t, 1, "")
}

func validReadySuccessorFixture(
	t *testing.T,
	content string,
) (ReadyReleaseControl, []byte) {
	t.Helper()
	return validReadyFixtureAt(t, 2, content)
}

func validReadyFixtureAt(
	t *testing.T,
	target uint16,
	content string,
) (ReadyReleaseControl, []byte) {
	t.Helper()
	legacy, _ := validReleaseFixture(t)
	completed := readyCompletedReceiptsTest(t, legacy.Schedule, target)
	var artifact *scheduledterminal.Artifact
	if content != "" {
		var artifactValue scheduledterminal.Artifact
		completed[0], artifactValue = bindReadyContentArtifactTest(t, completed[0], content)
		artifact = &artifactValue
	}
	direct := readyDirectReceiptsTest(legacy.Schedule, target, completed)
	contract := readyContractTest(t, legacy, target, direct, content)
	body := providerBodyTest(t, contract)
	request := providerRecordTest(t, contract, body)
	progress, decision := readyProgressTest(t, legacy.Schedule, contract, request, completed)
	control := readyControlTest(
		t, legacy, contract, request, body, progress, decision, direct, artifact,
	)
	encoded := mustCanonicalTest(t, control)
	decoded, err := DecodeReadyReleaseControl(bytes.NewReader(encoded))
	if err != nil {
		t.Fatalf("DecodeReadyReleaseControl(valid): %v\n%s", err, encoded)
	}
	return decoded, encoded
}

func readyContractTest(
	t *testing.T,
	legacy ReleaseControl,
	target uint16,
	receipts []scheduledterminal.Receipt,
	content string,
) graphscheduledcontract.ScheduledNodeContractCandidate {
	t.Helper()
	if target == 0 {
		return mustContractTest(t, legacy.ControlSnapshot, legacy.Schedule)
	}
	value, err := graphscheduledcontract.BuildSuccessor(
		legacy.ControlSnapshot, legacy.Schedule.ScheduleSHA256, executionOptionsTest(),
		receipts, content, legacy.Schedule.Nodes[target].NodeID,
	)
	if err != nil {
		t.Fatalf("BuildSuccessor: %v", err)
	}
	return value
}

func readyControlTest(
	t *testing.T,
	legacy ReleaseControl,
	contract graphscheduledcontract.ScheduledNodeContractCandidate,
	request ScheduledNodeProviderRequestRecord,
	body []byte,
	progress graphscheduledreconcile.ProgressSnapshot,
	decision graphscheduledreconcile.Decision,
	receipts []scheduledterminal.Receipt,
	artifact *scheduledterminal.Artifact,
) ReadyReleaseControl {
	t.Helper()
	record := contractRecordTest(t, contract)
	record.PredecessorReceiptCount = uint64(len(receipts))
	value := ReadyReleaseControl{
		V: 2, SchedulerProtocolVersion: 1, ReleaseControlProtocolVersion: 2,
		GraphRun: legacy.GraphRun, JournalEvents: legacy.JournalEvents,
		ControlSnapshot: legacy.ControlSnapshot, ScheduleRecord: legacy.ScheduleRecord,
		Schedule: legacy.Schedule, ProgressSnapshot: progress, ReconcileDecision: decision,
		ScheduledContractRecord: record, ScheduledContract: contract,
		DirectPredecessorReceipts: receipts, PredecessorContentArtifact: artifact,
		ProviderRequest: request, ProviderRequestJSON: string(body),
	}
	value.SnapshotSHA256 = mustDomainDigestTest(
		t, readyReleaseControlDigestDomain, readyReleasePayload(value),
	)
	return value
}

func cloneReadyControlTest(t *testing.T, value ReadyReleaseControl) ReadyReleaseControl {
	t.Helper()
	encoded := mustCanonicalTest(t, value)
	var clone ReadyReleaseControl
	if err := json.Unmarshal(encoded, &clone); err != nil {
		t.Fatalf("clone ready control: %v", err)
	}
	return clone
}

func resignReadyControlTest(t *testing.T, value *ReadyReleaseControl) {
	t.Helper()
	value.SnapshotSHA256 = mustDomainDigestTest(
		t, readyReleaseControlDigestDomain, readyReleasePayload(*value),
	)
}
