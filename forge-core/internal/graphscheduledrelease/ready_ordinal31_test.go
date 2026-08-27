package graphscheduledrelease

import (
	"bytes"
	"fmt"
	"os"
	"testing"

	"forgeos/forge-core/internal/graphdispatch"
	"forgeos/forge-core/internal/graphplan"
	"forgeos/forge-core/internal/graphschedule"
	"forgeos/forge-core/internal/graphscheduledcontract"
	"forgeos/forge-core/internal/graphscheduledreconcile"
	"forgeos/forge-core/internal/scheduledterminal"
)

const (
	readyControlOutputEnv = "FORGE_TEST_READY_CONTROL_OUTPUT"
	readyControlShapeEnv  = "FORGE_TEST_READY_CONTROL_SHAPE"
)

type readyManifestDigestViewTest struct {
	Edges   []graphplan.Edge              `json:"edges"`
	Manager graphplan.Manager             `json:"manager"`
	Nodes   []readyManifestNodeDigestTest `json:"nodes"`
	Source  readyManifestSourceDigestTest `json:"source"`
	V       uint16                        `json:"v"`
	Waves   [][]string                    `json:"waves"`
}

type readyManifestNodeDigestTest struct {
	Acceptance   string `json:"acceptance"`
	AgentProfile string `json:"agent_profile"`
	MemberRole   string `json:"member_role"`
	NodeID       string `json:"node_id"`
	ProjectID    string `json:"project_id"`
	Task         string `json:"task"`
}

type readyManifestSourceDigestTest struct {
	ContextSliceSHA256 string `json:"context_slice_sha256"`
	ContextVersion     uint16 `json:"context_version"`
	GroupID            string `json:"group_id"`
	GroupRunID         string `json:"group_run_id"`
	GroupRunVersion    uint16 `json:"group_run_version"`
	SnapshotBytes      uint64 `json:"snapshot_bytes"`
	SnapshotSHA256     string `json:"snapshot_sha256"`
}

func TestReadyReleaseSupportsOrdinal31AndMaximumDirectClosure(t *testing.T) {
	control, encoded := validReadyOrdinal31Fixture(t)
	value, err := BuildReadyAuthorization(control)
	if err != nil {
		t.Fatalf("BuildReadyAuthorization ordinal31: %v", err)
	}
	authorization, err := MarshalReadyAuthorization(value)
	if err != nil {
		t.Fatalf("MarshalReadyAuthorization ordinal31: %v", err)
	}
	progress := mustCanonicalTest(t, control.ProgressSnapshot)
	if value.ExecutionOrdinal != 31 || len(control.DirectPredecessorReceipts) != 31 ||
		len(progress) > graphscheduledreconcile.MaxProgressSnapshotBytes ||
		len(encoded) > MaxReadyReleaseControlBytes || len(authorization) > MaxReadyAuthorizationBytes {
		t.Fatal("ordinal31 identity, closure, or byte bound disagrees")
	}
}

// TestExportReadyControlForCrossLanguage writes only to an explicit
// test-owned temporary path so Rust can validate compiled Go fixtures.
func TestExportReadyControlForCrossLanguage(t *testing.T) {
	path := os.Getenv(readyControlOutputEnv)
	if path == "" {
		t.Skip("cross-language fixture output was not requested")
	}
	var encoded []byte
	switch os.Getenv(readyControlShapeEnv) {
	case "zero-direct-successor":
		_, encoded = validReadyZeroDirectSuccessorFixture(t)
	case "ordinal31":
		_, encoded = validReadyOrdinal31Fixture(t)
	default:
		t.Fatal("unsupported cross-language ready-control shape")
	}
	if err := os.WriteFile(path, encoded, 0o600); err != nil {
		t.Fatalf("write cross-language ready-control fixture: %v", err)
	}
}

func validReadyOrdinal31Fixture(t *testing.T) (ReadyReleaseControl, []byte) {
	t.Helper()
	snapshot, preparedJSON := readyOrdinal31SnapshotTest(t)
	schedule := mustScheduleTest(t, snapshot)
	completed := readyCompletedReceiptsTest(t, schedule, 31)
	direct := readyDirectReceiptsTest(schedule, 31, completed)
	contract := readyOrdinal31ContractTest(t, snapshot, schedule, direct)
	body := providerBodyTest(t, contract)
	request := providerRecordTest(t, contract, body)
	progress, decision := readyProgressTest(t, schedule, contract, request, completed)
	legacy := controlForTest(t, snapshot, schedule, contract, preparedJSON, body)
	control := readyControlTest(
		t, legacy, contract, request, body, progress, decision, direct, nil,
	)
	encoded := mustCanonicalTest(t, control)
	decoded, err := DecodeReadyReleaseControl(bytes.NewReader(encoded))
	if err != nil {
		t.Fatalf("DecodeReadyReleaseControl ordinal31: %v", err)
	}
	return decoded, encoded
}

func readyOrdinal31SnapshotTest(t *testing.T) (graphdispatch.ControlSnapshot, []byte) {
	t.Helper()
	snapshot := fixtureSnapshot(t)
	snapshot.Manifest.Nodes, snapshot.Manifest.Edges, snapshot.Manifest.Waves = readyMaximumGraphTest()
	manifestSHA := readyManifestDigestTest(t, snapshot.Manifest)
	plan, err := graphplan.Build(graphplan.Spec{
		V: snapshot.Manifest.V, Manager: snapshot.Manifest.Manager,
		Nodes: snapshot.Manifest.Nodes, Edges: snapshot.Manifest.Edges,
	}, snapshot.GraphID, manifestSHA)
	if err != nil {
		t.Fatalf("Build maximum plan: %v", err)
	}
	snapshot.GraphManifestSHA256 = manifestSHA
	snapshot.Manifest.Edges, snapshot.Manifest.Waves = plan.Edges, plan.Waves
	snapshot.Plan, snapshot.CorePlanSHA256 = plan, plan.PlanSHA256
	prepared := preparedEvent{
		V: 1, GraphRunID: snapshot.GraphRunID, Seq: 1, Type: "graph_run_prepared",
		GraphID: snapshot.GraphID, GraphManifestSHA256: manifestSHA,
		PlanSHA256: plan.PlanSHA256, SchedulerProtocolVersion: 1, PreparedAtMS: 73,
	}
	preparedJSON := mustCanonicalTest(t, prepared)
	snapshot.LastEventSHA256 = rawDomainDigest(preparedEventDigestDomain, preparedJSON)
	snapshot.SnapshotSHA256 = snapshotDigestTest(t, snapshot)
	if graphdispatch.ValidateControlSnapshot(snapshot) != nil {
		t.Fatal("maximum source snapshot is invalid")
	}
	return snapshot, preparedJSON
}

func readyMaximumGraphTest() ([]graphplan.Node, []graphplan.Edge, [][]string) {
	nodes := make([]graphplan.Node, 32)
	firstWave := make([]string, 31)
	edges := make([]graphplan.Edge, 31)
	for index := range nodes {
		nodeID := fmt.Sprintf("node-%02d", index)
		nodes[index] = graphplan.Node{
			NodeID: nodeID, ProjectID: fmt.Sprintf("project-%02d", index),
			MemberRole: "implementer", AgentProfile: "implementer",
			Task: fmt.Sprintf("Implement scheduled node %02d.", index), Acceptance: "Result is exact.",
		}
		if index < 31 {
			firstWave[index] = nodeID
			edges[index] = graphplan.Edge{FromNodeID: nodeID, ToNodeID: "node-31"}
		}
	}
	return nodes, edges, [][]string{firstWave, {"node-31"}}
}

func readyManifestDigestTest(t *testing.T, manifest graphdispatch.GraphManifest) string {
	t.Helper()
	view := readyManifestDigestViewTest{
		Edges: manifest.Edges, Manager: manifest.Manager, Nodes: readyManifestNodesTest(manifest.Nodes),
		Source: readyManifestSourceTest(manifest.Source), V: manifest.V, Waves: manifest.Waves,
	}
	return mustDomainDigestTest(t, "forge.group-agent-graph-manifest.v1\x00", view)
}

func readyManifestNodesTest(nodes []graphplan.Node) []readyManifestNodeDigestTest {
	result := make([]readyManifestNodeDigestTest, len(nodes))
	for index, node := range nodes {
		result[index] = readyManifestNodeDigestTest{
			Acceptance: node.Acceptance, AgentProfile: node.AgentProfile,
			MemberRole: node.MemberRole, NodeID: node.NodeID,
			ProjectID: node.ProjectID, Task: node.Task,
		}
	}
	return result
}

func readyManifestSourceTest(source graphdispatch.GraphSource) readyManifestSourceDigestTest {
	return readyManifestSourceDigestTest{
		ContextSliceSHA256: source.ContextSliceSHA256, ContextVersion: source.ContextVersion,
		GroupID: source.GroupID, GroupRunID: source.GroupRunID,
		GroupRunVersion: source.GroupRunVersion, SnapshotBytes: source.SnapshotBytes,
		SnapshotSHA256: source.SnapshotSHA256,
	}
}

func readyOrdinal31ContractTest(
	t *testing.T,
	snapshot graphdispatch.ControlSnapshot,
	schedule graphschedule.ExecutionSchedule,
	receipts []scheduledterminal.Receipt,
) graphscheduledcontract.ScheduledNodeContractCandidate {
	t.Helper()
	value, err := graphscheduledcontract.BuildSuccessor(
		snapshot, schedule.ScheduleSHA256, executionOptionsTest(), receipts, "", "node-31",
	)
	if err != nil {
		t.Fatalf("BuildSuccessor ordinal31: %v", err)
	}
	return value
}
