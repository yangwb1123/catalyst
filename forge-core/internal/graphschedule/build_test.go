package graphschedule

import (
	"reflect"
	"strings"
	"testing"
)

func TestBuildFreezesDiamondSerialOrderAndReceiptSlots(t *testing.T) {
	value := mustBuildSchedule(t)
	wantIDs := []string{"frontend", "backend", "sso"}
	if len(value.Nodes) != 3 || value.NodeCount != 3 || value.WaveCount != 2 {
		t.Fatalf("schedule counts = %d / %d / %d", len(value.Nodes), value.NodeCount, value.WaveCount)
	}
	for index, node := range value.Nodes {
		if node.NodeID != wantIDs[index] || node.ExecutionOrdinal != uint16(index) ||
			node.AuthoredNodeIndex != uint16(index) || node.Attempt != 1 {
			t.Fatalf("node %d = %#v", index, node)
		}
	}
	if value.Nodes[0].TopologyWaveIndex != 0 || value.Nodes[1].TopologyWaveIndex != 0 ||
		value.Nodes[2].TopologyWaveIndex != 1 ||
		!reflect.DeepEqual(value.Nodes[2].DirectPredecessorNodeIDs, []string{"frontend", "backend"}) {
		t.Fatalf("diamond topology = %#v", value.Nodes)
	}
	if !reflect.DeepEqual(value.InitialFrontier, []string{"frontend", "backend"}) ||
		value.InitialNode != "frontend" {
		t.Fatalf("initial selection = %v / %s", value.InitialFrontier, value.InitialNode)
	}
}

func TestBuildFreezesPassivePolicies(t *testing.T) {
	value := mustBuildSchedule(t)
	if value.ExecutionMode != "serial" || value.MaxInFlightNodes != 1 ||
		value.SelectionPolicy != "topology_wave_then_authored_order" ||
		value.ProgressionPolicy != "completed_contiguous_prefix" ||
		value.AttemptPolicy != "exactly_one" || value.FailurePolicy != "fail_fast_no_retry" ||
		value.PredecessorSemantics != "ordering_only" || value.PredecessorDataflow != "none" ||
		value.PartialOutputDataflow || value.ReceiptHandling != "future_verified_identity_slots" {
		t.Fatalf("schedule policy drift: %#v", value)
	}
	if !reflect.DeepEqual(value.OutcomePolicy, fixedOutcomePolicy()) ||
		value.ExecutionContractPresent || value.DispatchAuthorityReleased ||
		value.ProgressObserved || value.SuccessorAdvanced {
		t.Fatalf("schedule authority/outcome drift: %#v", value)
	}
}

func TestBuildOmitsPrivateManifestAndProviderData(t *testing.T) {
	encoded, err := MarshalSchedule(mustBuildSchedule(t))
	if err != nil {
		t.Fatalf("MarshalSchedule: %v", err)
	}
	for _, private := range []string{
		"project-frontend", "project-backend", "project-sso", "integration-manager",
		"Coordinate frontend", "Implement browser flow", "Browser uses the shared issuer",
		"gpt-5.6-sol", "api.openai.com", "member_role", "agent_profile", "project_id",
	} {
		if strings.Contains(string(encoded), private) {
			t.Fatalf("schedule disclosed private value %q: %s", private, encoded)
		}
	}
}

func TestBuildRejectsSingleNodeAndControlDrift(t *testing.T) {
	single := decodeFixtureSnapshot(t)
	single.Manifest.Nodes = single.Manifest.Nodes[:1]
	single.Manifest.Edges = single.Manifest.Edges[:0]
	resignFixtureSnapshot(t, &single)
	if _, err := Build(single); err == nil {
		t.Fatal("Build accepted a valid single-node control")
	}

	drift := decodeFixtureSnapshot(t)
	drift.SnapshotSHA256 = strings.Repeat("a", 64)
	if _, err := Build(drift); err == nil {
		t.Fatal("Build accepted digest-drifting control")
	}
}

func TestAuthoredNodeOrderChangesScheduleIdentity(t *testing.T) {
	first := mustBuildSchedule(t)
	snapshot := decodeFixtureSnapshot(t)
	snapshot.Manifest.Nodes[0], snapshot.Manifest.Nodes[1] =
		snapshot.Manifest.Nodes[1], snapshot.Manifest.Nodes[0]
	resignFixtureSnapshot(t, &snapshot)
	second, err := Build(snapshot)
	if err != nil {
		t.Fatalf("Build reordered schedule: %v", err)
	}
	if second.Nodes[0].NodeID != "backend" || second.ScheduleSHA256 == first.ScheduleSHA256 {
		t.Fatalf("authored order did not bind schedule: %#v", second)
	}
}

func TestDirectPredecessorsIgnoreCanonicalEdgeOrder(t *testing.T) {
	plan := decodeFixtureSnapshot(t).Plan
	plan.Edges[0], plan.Edges[1] = plan.Edges[1], plan.Edges[0]
	if got := directPredecessors(plan, "sso"); !reflect.DeepEqual(got, []string{"frontend", "backend"}) {
		t.Fatalf("predecessors follow edge order: %v", got)
	}
}

func TestMarshalScheduleDoesNotEscapeHTML(t *testing.T) {
	snapshot := decodeFixtureSnapshot(t)
	snapshot.GraphID = "graph<fixture"
	resignFixtureSnapshot(t, &snapshot)
	value, err := Build(snapshot)
	if err != nil {
		t.Fatalf("Build HTML fixture: %v", err)
	}
	encoded, err := MarshalSchedule(value)
	if err != nil || !strings.Contains(string(encoded), `"graph_id":"graph<fixture"`) ||
		strings.Contains(string(encoded), `\u003c`) {
		t.Fatalf("HTML-safe canonical encoding drift: err=%v %s", err, encoded)
	}
}

func TestMarshalScheduleRejectsPolicyAuthorityAndShapeDrift(t *testing.T) {
	tests := scheduleDriftCases()
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			value := mustBuildSchedule(t)
			test.mutate(&value)
			if _, err := MarshalSchedule(value); err == nil {
				t.Fatal("MarshalSchedule accepted drift")
			}
		})
	}
}

type scheduleDriftCase struct {
	name   string
	mutate func(*ExecutionSchedule)
}

func scheduleDriftCases() []scheduleDriftCase {
	return []scheduleDriftCase{
		{"selection", func(v *ExecutionSchedule) { v.SelectionPolicy = "caller_order" }},
		{"progression", func(v *ExecutionSchedule) { v.ProgressionPolicy = "any_completed" }},
		{"outcome", func(v *ExecutionSchedule) { v.OutcomePolicy.Length = "advance" }},
		{"partial dataflow", func(v *ExecutionSchedule) { v.PartialOutputDataflow = true }},
		{"receipt handling", func(v *ExecutionSchedule) { v.ReceiptHandling = "synthetic" }},
		{"authority", func(v *ExecutionSchedule) { v.DispatchAuthorityReleased = true }},
		{"progress", func(v *ExecutionSchedule) { v.ProgressObserved = true }},
		{"identifier", func(v *ExecutionSchedule) { v.Nodes[0].NodeID = "front\nend" }},
		{"bound", func(v *ExecutionSchedule) { v.NodeCount = 33 }},
		{"null nodes", func(v *ExecutionSchedule) { v.Nodes = nil }},
		{"null predecessor", func(v *ExecutionSchedule) { v.Nodes[0].DirectPredecessorNodeIDs = nil }},
		{"null frontier", func(v *ExecutionSchedule) { v.InitialFrontier = nil }},
		{"frontier", func(v *ExecutionSchedule) { v.InitialFrontier = []string{"frontend"} }},
		{"identity", func(v *ExecutionSchedule) { v.ScheduleID = "graph-execution-schedule-wrong" }},
		{"digest", func(v *ExecutionSchedule) { v.ScheduleSHA256 = strings.Repeat("0", 64) }},
	}
}
