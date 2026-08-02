package graphschedule

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/graphdispatch"
	"forgeos/forge-core/internal/graphplan"
)

type sharedScheduleFixture struct {
	V                              uint16 `json:"v"`
	ControlFixture                 string `json:"control_fixture"`
	CanonicalSchedulePayloadJSON   string `json:"canonical_schedule_payload_json"`
	ScheduleSHA256                 string `json:"schedule_sha256"`
	ScheduleID                     string `json:"schedule_id"`
	CanonicalExecutionScheduleJSON string `json:"canonical_execution_schedule_json"`
}

type existingControlFixture struct {
	V     uint16 `json:"v"`
	Input struct {
		CanonicalControlSnapshotJSON string `json:"canonical_control_snapshot_json"`
	} `json:"input"`
}

func readExistingControlFixture(t *testing.T) existingControlFixture {
	t.Helper()
	path := fixturePath("group-agent-node-execution-contract-v1.json")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read control fixture: %v", err)
	}
	var fixture existingControlFixture
	if err := json.Unmarshal(data, &fixture); err != nil || fixture.V != 1 {
		t.Fatalf("decode control fixture: v=%d err=%v", fixture.V, err)
	}
	return fixture
}

func readScheduleFixture(t *testing.T) sharedScheduleFixture {
	t.Helper()
	data, err := os.ReadFile(fixturePath("group-agent-graph-execution-schedule-v1.json"))
	if err != nil {
		t.Fatalf("read schedule fixture: %v", err)
	}
	var fixture sharedScheduleFixture
	if err := json.Unmarshal(data, &fixture); err != nil || fixture.V != 1 {
		t.Fatalf("decode schedule fixture: v=%d err=%v", fixture.V, err)
	}
	return fixture
}

func fixturePath(name string) string {
	return filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", name)
}

func decodeFixtureSnapshot(t *testing.T) graphdispatch.ControlSnapshot {
	t.Helper()
	fixture := readExistingControlFixture(t)
	snapshot, err := graphdispatch.DecodeControl(strings.NewReader(
		fixture.Input.CanonicalControlSnapshotJSON,
	))
	if err != nil {
		t.Fatalf("decode fixture control: %v", err)
	}
	return snapshot
}

func mustBuildSchedule(t *testing.T) ExecutionSchedule {
	t.Helper()
	value, err := Build(decodeFixtureSnapshot(t))
	if err != nil {
		t.Fatalf("Build schedule: %v", err)
	}
	return value
}

type manifestDigestViewFixture struct {
	Edges   []graphplan.Edge            `json:"edges"`
	Manager graphplan.Manager           `json:"manager"`
	Nodes   []manifestNodeDigestFixture `json:"nodes"`
	Source  manifestSourceDigestFixture `json:"source"`
	V       uint16                      `json:"v"`
	Waves   [][]string                  `json:"waves"`
}

type manifestNodeDigestFixture struct {
	Acceptance   string `json:"acceptance"`
	AgentProfile string `json:"agent_profile"`
	MemberRole   string `json:"member_role"`
	NodeID       string `json:"node_id"`
	ProjectID    string `json:"project_id"`
	Task         string `json:"task"`
}

type manifestSourceDigestFixture struct {
	ContextSliceSHA256 string `json:"context_slice_sha256"`
	ContextVersion     uint16 `json:"context_version"`
	GroupID            string `json:"group_id"`
	GroupRunID         string `json:"group_run_id"`
	GroupRunVersion    uint16 `json:"group_run_version"`
	SnapshotBytes      uint64 `json:"snapshot_bytes"`
	SnapshotSHA256     string `json:"snapshot_sha256"`
}

type controlPayloadFixture struct {
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

func resignFixtureSnapshot(t *testing.T, snapshot *graphdispatch.ControlSnapshot) {
	t.Helper()
	canonicalizeFixtureTopology(t, snapshot)
	manifestSHA := fixtureManifestDigest(t, snapshot.Manifest)
	spec := graphplan.Spec{
		V: snapshot.Manifest.V, Manager: snapshot.Manifest.Manager,
		Nodes: snapshot.Manifest.Nodes, Edges: snapshot.Manifest.Edges,
	}
	plan, err := graphplan.Build(spec, snapshot.GraphID, manifestSHA)
	if err != nil {
		t.Fatalf("rebuild fixture plan: %v", err)
	}
	snapshot.GraphManifestSHA256 = manifestSHA
	snapshot.Plan, snapshot.CorePlanSHA256 = plan, plan.PlanSHA256
	snapshot.SnapshotSHA256 = fixtureSnapshotDigest(t, *snapshot)
	if err := graphdispatch.ValidateControlSnapshot(*snapshot); err != nil {
		t.Fatalf("validate rebuilt fixture: %v", err)
	}
}

func canonicalizeFixtureTopology(t *testing.T, snapshot *graphdispatch.ControlSnapshot) {
	t.Helper()
	spec := graphplan.Spec{
		V: snapshot.Manifest.V, Manager: snapshot.Manifest.Manager,
		Nodes: snapshot.Manifest.Nodes, Edges: snapshot.Manifest.Edges,
	}
	plan, err := graphplan.Build(spec, snapshot.GraphID, strings.Repeat("0", 64))
	if err != nil {
		t.Fatalf("canonicalize fixture topology: %v", err)
	}
	snapshot.Manifest.Edges, snapshot.Manifest.Waves = plan.Edges, plan.Waves
}

func fixtureManifestDigest(t *testing.T, manifest graphdispatch.GraphManifest) string {
	t.Helper()
	nodes := make([]manifestNodeDigestFixture, len(manifest.Nodes))
	for index, node := range manifest.Nodes {
		nodes[index] = manifestNodeDigestFixture{
			Acceptance: node.Acceptance, AgentProfile: node.AgentProfile,
			MemberRole: node.MemberRole, NodeID: node.NodeID,
			ProjectID: node.ProjectID, Task: node.Task,
		}
	}
	source := manifest.Source
	view := manifestDigestViewFixture{
		Edges: manifest.Edges, Manager: manifest.Manager, Nodes: nodes,
		Source: manifestSourceDigestFixture{
			ContextSliceSHA256: source.ContextSliceSHA256, ContextVersion: source.ContextVersion,
			GroupID: source.GroupID, GroupRunID: source.GroupRunID,
			GroupRunVersion: source.GroupRunVersion, SnapshotBytes: source.SnapshotBytes,
			SnapshotSHA256: source.SnapshotSHA256,
		}, V: manifest.V, Waves: manifest.Waves,
	}
	return fixtureDomainDigest(t, "forge.group-agent-graph-manifest.v1\x00", view)
}

func fixtureSnapshotDigest(t *testing.T, snapshot graphdispatch.ControlSnapshot) string {
	t.Helper()
	payload := controlPayloadFixture{
		V: snapshot.V, SchedulerProtocolVersion: snapshot.SchedulerProtocolVersion,
		GraphRunVersion: snapshot.GraphRunVersion, GraphRunID: snapshot.GraphRunID,
		GraphID: snapshot.GraphID, SourceSnapshotSHA256: snapshot.SourceSnapshotSHA256,
		GraphManifestSHA256: snapshot.GraphManifestSHA256, CorePlanSHA256: snapshot.CorePlanSHA256,
		LastEventSeq: snapshot.LastEventSeq, LastEventSHA256: snapshot.LastEventSHA256,
		ExecutionContractPresent:  snapshot.ExecutionContractPresent,
		DispatchAuthorityReleased: snapshot.DispatchAuthorityReleased,
		Plan:                      snapshot.Plan, Manifest: snapshot.Manifest,
	}
	return fixtureDomainDigest(t, "forge.group-agent-graph-control-snapshot.v1\x00", payload)
}

func fixtureDomainDigest(t *testing.T, domain string, value any) string {
	t.Helper()
	encoded, err := canonicalBytes(value)
	if err != nil {
		t.Fatalf("canonical fixture digest: %v", err)
	}
	return rawDomainDigest(domain, encoded)
}
