package graphscheduledcontract

import (
	"reflect"
	"strings"
	"testing"

	"forgeos/forge-core/internal/graphdispatch"
)

func TestBuildInitialBindsScheduleSelectionAndEmptyReceiptBase(t *testing.T) {
	value := mustCandidate(t)
	if value.ContractScope != contractScope || value.Node.ExecutionOrdinal != 0 ||
		value.Node.NodeID != "frontend" || value.Node.TopologyWaveIndex != 0 || value.Node.Attempt != 1 {
		t.Fatalf("initial selection = %#v", value.Node)
	}
	if value.ScheduleSHA256 != readScheduleFixture(t).ScheduleSHA256 ||
		value.ScheduleID != "graph-execution-schedule-"+value.ScheduleSHA256 {
		t.Fatalf("schedule binding = %s / %s", value.ScheduleID, value.ScheduleSHA256)
	}
	request := value.Request
	if request.RequiredPredecessorNodeIDs == nil || len(request.RequiredPredecessorNodeIDs) != 0 ||
		request.PredecessorTerminalReceipts == nil || len(request.PredecessorTerminalReceipts) != 0 ||
		request.PredecessorContentIncluded || request.Tools == nil || len(request.Tools) != 0 {
		t.Fatalf("initial request evidence = %#v", request)
	}
	if strings.Contains(request.UserPrompt, "receipt") || strings.Contains(request.UserPrompt, "schedule") {
		t.Fatalf("provider Prompt disclosed local evidence: %s", request.UserPrompt)
	}
}

func TestBuildInitialFixesPassiveAuthorityFlagsAndPolicies(t *testing.T) {
	value := mustCandidate(t)
	if value.LifecycleContractAdmitted || value.ProviderRequestPresent ||
		value.ExecutionAuthorityReleased || value.DispatchAuthorityReleased ||
		value.ProgressObserved || value.SuccessorAdvanceAuthorized {
		t.Fatalf("candidate claimed an effect: %#v", value)
	}
	if value.Workspace.Mode != "none" || value.Provider.Kind != "openai_responses" ||
		value.Budgets.MaxTurns != 1 || value.Budgets.MaxToolCalls != 0 ||
		value.Result.PredecessorDataflow != "none" || value.Failure.AutomaticRetry {
		t.Fatalf("candidate policy drift: %#v", value)
	}
}

func TestV2IdentityIsSeparatedFromLegacyContractV1(t *testing.T) {
	source := readSourceFixture(t)
	snapshot, options := fixtureSnapshot(t), source.Input.ExecutionOptions.options()
	legacy, err := graphdispatch.Build(snapshot, options)
	if err != nil {
		t.Fatalf("build legacy contract: %v", err)
	}
	value, err := BuildInitial(snapshot, readScheduleFixture(t).ScheduleSHA256, options)
	if err != nil {
		t.Fatalf("build scheduled candidate: %v", err)
	}
	if legacy.V != 1 || value.V != 2 || legacy.ContractSHA256 == value.ContractSHA256 ||
		strings.HasPrefix(value.ContractID, "node-contract-") ||
		!strings.HasPrefix(value.ContractID, contractIDPrefix) ||
		!strings.HasPrefix(value.Request.RequestID, requestIDPrefix) {
		t.Fatalf("v1/v2 identity fence failed: legacy=%s candidate=%s request=%s",
			legacy.ContractID, value.ContractID, value.Request.RequestID)
	}
}

func TestBuildInitialRejectsScheduleMismatchAndSingleNodeControl(t *testing.T) {
	source := readSourceFixture(t)
	options := source.Input.ExecutionOptions.options()
	if _, err := BuildInitial(fixtureSnapshot(t), strings.Repeat("0", 64), options); err == nil {
		t.Fatal("BuildInitial accepted a different schedule digest")
	}
	snapshot := fixtureSnapshot(t)
	snapshot.Manifest.Nodes = snapshot.Manifest.Nodes[:1]
	if _, err := BuildInitial(snapshot, readScheduleFixture(t).ScheduleSHA256, options); err == nil {
		t.Fatal("BuildInitial accepted a malformed single-node control")
	}
}

func TestMarshalCandidateRejectsInitialFenceDrift(t *testing.T) {
	for _, test := range candidateDriftCases() {
		t.Run(test.name, func(t *testing.T) {
			value := mustCandidate(t)
			test.mutate(&value)
			resignCandidate(t, &value)
			if _, err := MarshalCandidate(value); err == nil {
				t.Fatal("MarshalCandidate accepted drift")
			}
		})
	}
}

type candidateDriftCase struct {
	name   string
	mutate func(*ScheduledNodeContractCandidate)
}

func candidateDriftCases() []candidateDriftCase {
	return []candidateDriftCase{
		{"scope", func(v *ScheduledNodeContractCandidate) { v.ContractScope = "generic" }},
		{"ordinal", func(v *ScheduledNodeContractCandidate) { v.Node.ExecutionOrdinal = 1 }},
		{"wave", func(v *ScheduledNodeContractCandidate) { v.Node.TopologyWaveIndex = 1 }},
		{"required predecessor", func(v *ScheduledNodeContractCandidate) {
			v.Request.RequiredPredecessorNodeIDs = []string{"frontend"}
		}},
		{"receipt", func(v *ScheduledNodeContractCandidate) {
			v.Request.PredecessorTerminalReceipts = []PredecessorTerminalReceipt{{PredecessorNodeID: "x"}}
		}},
		{"predecessor content", func(v *ScheduledNodeContractCandidate) { v.Request.PredecessorContentIncluded = true }},
		{"null predecessors", func(v *ScheduledNodeContractCandidate) { v.Request.RequiredPredecessorNodeIDs = nil }},
		{"null receipts", func(v *ScheduledNodeContractCandidate) { v.Request.PredecessorTerminalReceipts = nil }},
		{"null tools", func(v *ScheduledNodeContractCandidate) { v.Request.Tools = nil }},
		{"lifecycle authority", func(v *ScheduledNodeContractCandidate) { v.LifecycleContractAdmitted = true }},
		{"provider request", func(v *ScheduledNodeContractCandidate) { v.ProviderRequestPresent = true }},
		{"execution authority", func(v *ScheduledNodeContractCandidate) { v.ExecutionAuthorityReleased = true }},
		{"dispatch authority", func(v *ScheduledNodeContractCandidate) { v.DispatchAuthorityReleased = true }},
		{"progress", func(v *ScheduledNodeContractCandidate) { v.ProgressObserved = true }},
		{"successor", func(v *ScheduledNodeContractCandidate) { v.SuccessorAdvanceAuthorized = true }},
	}
}

func resignCandidate(t *testing.T, value *ScheduledNodeContractCandidate) {
	t.Helper()
	requestDigest, err := domainDigest(requestDigestDomain, requestPayloadFrom(value.Request))
	if err != nil {
		t.Fatalf("request digest: %v", err)
	}
	value.Request.RequestID = requestIDPrefix + requestDigest
	value.Request.RequestSHA256 = requestDigest
	contractDigest, err := domainDigest(contractDigestDomain, candidatePayloadFrom(*value))
	if err != nil {
		t.Fatalf("contract digest: %v", err)
	}
	value.ContractID = contractIDPrefix + contractDigest
	value.ContractSHA256 = contractDigest
}

func TestValidateCandidateSourceRejectsSelfConsistentSourceDrift(t *testing.T) {
	value := mustCandidate(t)
	value.Node.AgentProfile = "different-profile"
	resignCandidate(t, &value)
	if validateCandidate(value) != nil {
		t.Fatal("source drift fixture should remain intrinsically valid")
	}
	if ValidateCandidateSource(value, fixtureSnapshot(t)) == nil {
		t.Fatal("source validation accepted a self-consistent private-label substitution")
	}
	want := mustCandidate(t)
	if !reflect.DeepEqual(want, mustCandidate(t)) {
		t.Fatal("deterministic candidate construction drifted")
	}
}
