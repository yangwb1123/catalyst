package graphscheduledcontract

import (
	"bytes"
	"strings"
	"testing"

	"forgeos/forge-core/internal/graphdispatch"
)

func TestSharedScheduledNodeContractGolden(t *testing.T) {
	fixture := readCandidateFixture(t)
	if fixture.ControlFixture != "group-agent-node-execution-contract-v1.json" ||
		fixture.ScheduleFixture != "group-agent-graph-execution-schedule-v1.json" {
		t.Fatalf("unknown fixture sources: %q / %q", fixture.ControlFixture, fixture.ScheduleFixture)
	}
	snapshot, err := graphdispatch.DecodeControl(strings.NewReader(fixture.Input.CanonicalControlSnapshotJSON))
	if err != nil {
		t.Fatalf("decode fixture control: %v", err)
	}
	value, err := BuildInitial(
		snapshot, fixture.Input.ScheduleSHA256, fixture.Input.ExecutionOptions.options(),
	)
	if err != nil {
		t.Fatalf("BuildInitial golden: %v", err)
	}
	assertRequestGolden(t, value, fixture.Expected)
	assertCandidateGolden(t, value, fixture.Expected)
}

func assertRequestGolden(
	t *testing.T,
	value ScheduledNodeContractCandidate,
	want candidateGolden,
) {
	t.Helper()
	payload, err := canonicalBytes(requestPayloadFrom(value.Request))
	encoded, encodeErr := canonicalBytes(value.Request)
	valid := err == nil && encodeErr == nil && value.Node.NodeID == want.SelectedNodeID &&
		value.Request.UserPrompt == want.CanonicalUserPromptJSON &&
		string(payload) == want.CanonicalRequestPayloadJSON &&
		value.Request.RequestSHA256 == want.RequestSHA256 && value.Request.RequestID == want.RequestID &&
		string(encoded) == want.CanonicalRequestJSON
	if !valid {
		t.Fatalf("request golden differs: payload_err=%v encode_err=%v\n%s\n%s", err, encodeErr, payload, encoded)
	}
}

func assertCandidateGolden(
	t *testing.T,
	value ScheduledNodeContractCandidate,
	want candidateGolden,
) {
	t.Helper()
	payload, err := canonicalBytes(candidatePayloadFrom(value))
	encoded, encodeErr := MarshalCandidate(value)
	valid := err == nil && encodeErr == nil && string(payload) == want.CanonicalContractPayloadJSON &&
		value.ContractSHA256 == want.ContractSHA256 && value.ContractID == want.ContractID &&
		string(encoded) == want.CanonicalContractJSON && !bytes.HasSuffix(encoded, []byte{'\n'})
	if !valid {
		t.Fatalf("candidate golden differs: payload_err=%v encode_err=%v\n%s\n%s", err, encodeErr, payload, encoded)
	}
}
