package graphscheduledrelease

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/graphdispatch"
	"forgeos/forge-core/internal/graphplan"
	"forgeos/forge-core/internal/graphschedule"
	"forgeos/forge-core/internal/graphscheduledcontract"
)

type sourceFixture struct {
	Input struct {
		CanonicalControlSnapshotJSON string `json:"canonical_control_snapshot_json"`
	} `json:"input"`
}

type controlSnapshotPayloadTest struct {
	V                         uint16                      `json:"v"`
	SchedulerProtocolVersion  uint16                      `json:"scheduler_protocol_version"`
	GraphRunVersion           uint16                      `json:"graph_run_version"`
	GraphRunID                string                      `json:"graph_run_id"`
	GraphID                   string                      `json:"graph_id"`
	SourceSnapshotSHA256      string                      `json:"source_snapshot_sha256"`
	GraphManifestSHA256       string                      `json:"graph_manifest_sha256"`
	CorePlanSHA256            string                      `json:"core_plan_sha256"`
	LastEventSeq              uint64                      `json:"last_event_seq"`
	LastEventSHA256           string                      `json:"last_event_sha256"`
	ExecutionContractPresent  bool                        `json:"execution_contract_present"`
	DispatchAuthorityReleased bool                        `json:"dispatch_authority_released"`
	Plan                      graphplan.Plan              `json:"plan"`
	Manifest                  graphdispatch.GraphManifest `json:"manifest"`
}

func validReleaseFixture(t *testing.T) (ReleaseControl, []byte) {
	t.Helper()
	snapshot := fixtureSnapshot(t)
	prepared := preparedEvent{
		V: 1, GraphRunID: snapshot.GraphRunID, Seq: 1, Type: "graph_run_prepared",
		GraphID: snapshot.GraphID, GraphManifestSHA256: snapshot.GraphManifestSHA256,
		PlanSHA256: snapshot.CorePlanSHA256, SchedulerProtocolVersion: 1, PreparedAtMS: 73,
	}
	preparedJSON := mustCanonicalTest(t, prepared)
	snapshot.LastEventSHA256 = rawDomainDigest(preparedEventDigestDomain, preparedJSON)
	snapshot.SnapshotSHA256 = snapshotDigestTest(t, snapshot)
	schedule := mustScheduleTest(t, snapshot)
	contract := mustContractTest(t, snapshot, schedule)
	body := providerBodyTest(t, contract)
	control := controlForTest(t, snapshot, schedule, contract, preparedJSON, body)
	encoded := mustCanonicalTest(t, control)
	decoded, err := DecodeControl(bytes.NewReader(encoded))
	if err != nil {
		t.Fatalf("DecodeControl(valid): %v\n%s", err, encoded)
	}
	return decoded, encoded
}

func fixtureSnapshot(t *testing.T) graphdispatch.ControlSnapshot {
	t.Helper()
	path := filepath.Join("..", "..", "..", "docs", "contracts", "fixtures",
		"group-agent-node-execution-contract-v1.json")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read source fixture: %v", err)
	}
	var fixture sourceFixture
	if err := json.Unmarshal(data, &fixture); err != nil {
		t.Fatalf("decode source fixture: %v", err)
	}
	snapshot, err := graphdispatch.DecodeControl(strings.NewReader(fixture.Input.CanonicalControlSnapshotJSON))
	if err != nil {
		t.Fatalf("decode source control: %v", err)
	}
	return snapshot
}

func snapshotDigestTest(t *testing.T, value graphdispatch.ControlSnapshot) string {
	t.Helper()
	payload := controlSnapshotPayloadTest{
		V: value.V, SchedulerProtocolVersion: value.SchedulerProtocolVersion,
		GraphRunVersion: value.GraphRunVersion, GraphRunID: value.GraphRunID, GraphID: value.GraphID,
		SourceSnapshotSHA256: value.SourceSnapshotSHA256,
		GraphManifestSHA256:  value.GraphManifestSHA256, CorePlanSHA256: value.CorePlanSHA256,
		LastEventSeq: value.LastEventSeq, LastEventSHA256: value.LastEventSHA256,
		ExecutionContractPresent:  value.ExecutionContractPresent,
		DispatchAuthorityReleased: value.DispatchAuthorityReleased,
		Plan:                      value.Plan, Manifest: value.Manifest,
	}
	return mustDomainDigestTest(t, "forge.group-agent-graph-control-snapshot.v1\x00", payload)
}

func executionOptionsTest() graphdispatch.ExecutionOptions {
	return graphdispatch.ExecutionOptions{
		Endpoint: "https://api.openai.com/v1/responses", Model: "gpt-5.6-sol",
		MaxOutputTokens: 4096, MaxModelOutputBytes: 65_536, MaxModelEvents: 4096,
		TimeoutMilliseconds: 60_000, MaxCostUSDMicros: 1_000_000,
		PricingSnapshotSHA256: strings.Repeat("4", 64), MaxResultBytes: 512 * 1024,
	}
}

func mustScheduleTest(t *testing.T, snapshot graphdispatch.ControlSnapshot) graphschedule.ExecutionSchedule {
	t.Helper()
	value, err := graphschedule.Build(snapshot)
	if err != nil {
		t.Fatalf("Build schedule: %v", err)
	}
	return value
}

func mustContractTest(
	t *testing.T,
	snapshot graphdispatch.ControlSnapshot,
	schedule graphschedule.ExecutionSchedule,
) graphscheduledcontract.ScheduledNodeContractCandidate {
	t.Helper()
	value, err := graphscheduledcontract.BuildInitial(snapshot, schedule.ScheduleSHA256, executionOptionsTest())
	if err != nil {
		t.Fatalf("BuildInitial: %v", err)
	}
	return value
}

func mustCanonicalTest(t *testing.T, value any) []byte {
	t.Helper()
	encoded, err := canonicalBytes(value)
	if err != nil {
		t.Fatalf("canonical JSON: %v", err)
	}
	return encoded
}

func mustDomainDigestTest(t *testing.T, domain string, value any) string {
	t.Helper()
	digest, err := domainDigest(domain, value)
	if err != nil {
		t.Fatalf("domain digest: %v", err)
	}
	return digest
}
