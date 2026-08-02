package graphterminal

import (
	"bytes"
	"encoding/json"
	"testing"

	"forgeos/forge-core/internal/graphpricing"
)

type terminalFixture struct {
	Control      TerminalControl
	ControlJSON  []byte
	ClaimJSON    []byte
	ArtifactJSON []byte
	Receipt      Receipt
	ReceiptJSON  []byte
}

func validTerminalFixture(t *testing.T) terminalFixture {
	t.Helper()
	base := buildReleaseFixture(t)
	claim, claimJSON := buildClaimFixture(t, base)
	artifact := buildArtifactFixture(t, claim, base)
	control := buildTerminalControlFixture(t, base, claim, artifact, claimJSON)
	encoded := mustCanonicalFixture(t, control)
	decoded, err := DecodeControl(bytes.NewReader(encoded))
	if err != nil {
		t.Fatalf("decode terminal control: %v", err)
	}
	receipt, err := BuildReceipt(decoded)
	if err != nil {
		t.Fatalf("build terminal receipt: %v", err)
	}
	receiptJSON, err := MarshalReceipt(receipt)
	if err != nil {
		t.Fatalf("marshal terminal receipt: %v", err)
	}
	return terminalFixture{
		decoded, encoded, claimJSON, mustCanonicalFixture(t, artifact), receipt, receiptJSON,
	}
}

func buildClaimFixture(t *testing.T, base releaseFixture) (Claim, []byte) {
	t.Helper()
	auth := base.Authorization
	claim := Claim{
		V: ClaimVersion, GraphRunID: auth.GraphRunID, DispatchID: "dispatch-terminal-fixture",
		AuthorizationID: auth.AuthorizationID, AuthorizationSHA256: auth.AuthorizationSHA256,
		DispatchRequestID: auth.DispatchRequestID, DispatchRequestSHA256: auth.DispatchRequestSHA256,
		LogicalRequestSHA256: auth.LogicalRequestSHA256, RequestBodySHA256: auth.RequestBodySHA256,
		RequestBodyBytes: auth.RequestBodyBytes, PricingSnapshotSHA256: auth.PricingSnapshotSHA256,
		NodeID: auth.NodeID, Attempt: auth.Attempt, MaxCostUSDMicros: auth.Budgets.MaxCostUSDMicros,
		ConsentContractVersion: auth.ReleaseRequirements.ConsentContractVersion,
		LaneOwnershipID:        "lane-owner-terminal-fixture", ProjectLaneSHA256: auth.ProjectLaneSHA256,
		ExpectedLastEventSeq:    auth.ExpectedLastEventSeq,
		ExpectedLastEventSHA256: auth.ExpectedLastEventSHA256, ReleasedAtMS: 100,
	}
	claimJSON := mustCanonicalFixture(t, eventFromClaim(claim))
	claim.ClaimEventSHA256 = rawDomainDigest(controlEventDomain, claimJSON)
	return claim, claimJSON
}

func buildArtifactFixture(t *testing.T, claim Claim, base releaseFixture) TerminalArtifact {
	t.Helper()
	cost, err := graphpricing.ActualCostUSDMicros(base.Pricing, 5, 3)
	if err != nil {
		t.Fatalf("actual fixture cost: %v", err)
	}
	value := TerminalArtifact{
		V: TerminalArtifactVersion, TerminalArtifactProtocolVersion: TerminalArtifactProtocol,
		ArtifactKind: "result", GraphRunID: claim.GraphRunID, NodeID: claim.NodeID,
		Attempt: claim.Attempt, DispatchID: claim.DispatchID,
		ClaimEventSHA256: claim.ClaimEventSHA256, AuthorizationSHA256: claim.AuthorizationSHA256,
		DispatchRequestSHA256: claim.DispatchRequestSHA256,
		LogicalRequestSHA256:  claim.LogicalRequestSHA256, RequestBodySHA256: claim.RequestBodySHA256,
		PricingSnapshotSHA256: claim.PricingSnapshotSHA256, LaneOwnershipID: claim.LaneOwnershipID,
		ProjectLaneSHA256: claim.ProjectLaneSHA256, ProviderPollStarted: true,
		TerminalSeen: true, StreamEOFSeen: true, Classification: "completed",
		OutputText: "done", OutputBytes: 4,
		OutputSHA256: rawDomainDigest(outputDigestDomain, []byte("done")), UsageObserved: true,
		InputTokens: 5, OutputTokens: 3, ActualCostCalculated: true,
		ActualCostUSDMicros: cost, RetryAuthorized: false, CreatedAtMS: 110,
	}
	return identifyArtifactFixture(t, value)
}

func buildUncertaintyArtifactFixture(
	t *testing.T,
	artifact TerminalArtifact,
) TerminalArtifact {
	t.Helper()
	value := artifact
	value.ArtifactKind = "uncertainty"
	value.Classification = "missing_usage"
	value.UsageObserved = false
	value.InputTokens, value.OutputTokens = 0, 0
	value.ActualCostCalculated = false
	value.ActualCostUSDMicros = 0
	return identifyArtifactFixture(t, value)
}

func identifyArtifactFixture(t *testing.T, value TerminalArtifact) TerminalArtifact {
	t.Helper()
	encoded := mustCanonicalFixture(t, payloadFromArtifact(value))
	value.ArtifactBytes = uint64(len(encoded))
	value.ArtifactSHA256 = rawDomainDigest(artifactDigestDomain, encoded)
	value.ArtifactID = "graph-node-terminal-artifact-" + value.ArtifactSHA256
	return value
}

func buildTerminalControlFixture(t *testing.T, base releaseFixture, claim Claim, artifact TerminalArtifact, claimJSON []byte) TerminalControl {
	t.Helper()
	run := base.Control.GraphRun
	run.V, run.Status = 4, "dispatch_unknown"
	run.DispatchAuthorityReleased, run.LastEventSeq = true, 4
	events := append([]json.RawMessage{}, base.Control.JournalEvents...)
	events = append(events, claimJSON)
	run.JournalBytes = journalBytes(events)
	value := TerminalControl{
		V: TerminalControlVersion, SchedulerProtocolVersion: 1,
		TerminalControlProtocolVersion: TerminalControlProtocol, GraphRun: run,
		Plan: base.Control.Plan, Manifest: base.Control.Manifest, JournalEvents: events,
		ContractRecord: base.Control.ContractRecord, Contract: base.Control.Contract,
		DispatchRequest:     base.Control.DispatchRequest,
		ProviderRequestJSON: base.Control.ProviderRequestJSON, Authorization: base.Authorization,
		Pricing: base.Pricing,
		ActiveLane: ActiveLane{
			V: ActiveLaneVersion, ProjectLaneSHA256: claim.ProjectLaneSHA256,
			LaneOwnershipID: claim.LaneOwnershipID, GraphRunID: claim.GraphRunID,
			NodeID: claim.NodeID, Attempt: claim.Attempt, DispatchID: claim.DispatchID,
			ClaimEventSHA256: claim.ClaimEventSHA256, ClaimedAtMS: claim.ReleasedAtMS,
		}, Claim: claim, Artifact: artifact,
	}
	return identifyControlFixture(t, value)
}

func identifyControlFixture(t *testing.T, value TerminalControl) TerminalControl {
	t.Helper()
	value.SnapshotSHA256 = mustDigestFixture(t, controlDigestDomain, controlPayload(value))
	return value
}
