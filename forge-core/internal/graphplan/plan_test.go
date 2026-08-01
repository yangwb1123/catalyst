package graphplan

import (
	"crypto/sha256"
	"encoding/hex"
	"reflect"
	"strings"
	"testing"
)

const fixtureManifestSHA = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"

func TestBuildDiamondUsesAuthoredOrderAndCanonicalEdges(t *testing.T) {
	plan, err := Build(validSpec(), "graph-fixture-v1", fixtureManifestSHA)
	if err != nil {
		t.Fatalf("Build: %v", err)
	}
	assertStrings(t, plan.AuthoredNodeIDs, []string{"frontend", "backend", "sso"})
	assertNestedStrings(t, plan.Waves, [][]string{{"frontend", "backend"}, {"sso"}})
	wantEdges := []Edge{
		{FromNodeID: "backend", ToNodeID: "sso"},
		{FromNodeID: "frontend", ToNodeID: "sso"},
	}
	if !reflect.DeepEqual(plan.Edges, wantEdges) {
		t.Fatalf("canonical edges = %#v, want %#v", plan.Edges, wantEdges)
	}
	if plan.ExecutionContractPresent || plan.DispatchAuthorityReleased {
		t.Fatal("an inert plan must not claim an execution contract or dispatch authority")
	}
	encoded, err := MarshalPlan(plan)
	if err != nil {
		t.Fatalf("MarshalPlan: %v", err)
	}
	for _, private := range []string{
		"Coordinate frontend", "Implement browser flow", "project-frontend",
	} {
		if strings.Contains(string(encoded), private) {
			t.Fatalf("topology plan disclosed authored prose/project data: %s", encoded)
		}
	}
	assertPlanDigest(t, plan)
}

func TestBuildFanoutIsOneAuthoredOrderWave(t *testing.T) {
	spec := validSpec()
	spec.Edges = []Edge{}
	plan, err := Build(spec, "fanout", fixtureManifestSHA)
	if err != nil {
		t.Fatalf("Build: %v", err)
	}
	assertNestedStrings(t, plan.Waves, [][]string{{"frontend", "backend", "sso"}})
	if plan.Edges == nil {
		t.Fatal("zero canonical edges must encode as [], not null")
	}
}

func TestBuildCanonicalIdentity(t *testing.T) {
	original := validSpec()
	reordered := validSpec()
	reordered.Edges[0], reordered.Edges[1] = reordered.Edges[1], reordered.Edges[0]
	first := mustBuild(t, original, fixtureManifestSHA, SchedulerProtocolVersion)
	second := mustBuild(t, reordered, fixtureManifestSHA, SchedulerProtocolVersion)
	if !reflect.DeepEqual(first, second) {
		t.Fatal("authored edge order changed the canonical plan")
	}

	reorderedNodes := validSpec()
	reorderedNodes.Nodes[0], reorderedNodes.Nodes[1] =
		reorderedNodes.Nodes[1], reorderedNodes.Nodes[0]
	assertDigestChanged(t, first, mustBuild(
		t, reorderedNodes, fixtureManifestSHA, SchedulerProtocolVersion,
	))

	taskOnly := validSpec()
	taskOnly.Nodes[0].Task = "A private task revision."
	if got := mustBuild(t, taskOnly, fixtureManifestSHA, SchedulerProtocolVersion); got.PlanSHA256 != first.PlanSHA256 {
		t.Fatal("task prose entered the topology-only plan digest")
	}
	assertDigestChanged(t, first, mustBuild(
		t, taskOnly, "1123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
		SchedulerProtocolVersion,
	))
	assertDigestChanged(t, first, mustBuild(t, original, fixtureManifestSHA, 2))
}

func TestBuildRejectsMalformedTopology(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(*Spec)
	}{
		{"duplicate node", func(spec *Spec) { spec.Nodes[1].NodeID = spec.Nodes[0].NodeID }},
		{"duplicate edge", func(spec *Spec) { spec.Edges[1] = spec.Edges[0] }},
		{"unknown source", func(spec *Spec) { spec.Edges[0].FromNodeID = "ghost" }},
		{"unknown destination", func(spec *Spec) { spec.Edges[0].ToNodeID = "ghost" }},
		{"self edge", func(spec *Spec) { spec.Edges[0].ToNodeID = spec.Edges[0].FromNodeID }},
		{"cycle", func(spec *Spec) {
			spec.Edges = append(spec.Edges, Edge{FromNodeID: "sso", ToNodeID: "frontend"})
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			spec := validSpec()
			test.mutate(&spec)
			if _, err := Build(spec, "graph", fixtureManifestSHA); err == nil {
				t.Fatal("Build accepted malformed topology")
			}
		})
	}
}

func TestMarshalPlanUsesFrozenOrderWithoutHTMLEscapingOrNewline(t *testing.T) {
	spec := validSpec()
	spec.Nodes[0].NodeID = "front<end"
	spec.Edges[0].FromNodeID = "front<end"
	plan, err := buildWithProtocol(
		spec, "graph<fixture", fixtureManifestSHA, SchedulerProtocolVersion,
	)
	if err != nil {
		t.Fatalf("buildWithProtocol: %v", err)
	}
	encoded, err := MarshalPlan(plan)
	if err != nil {
		t.Fatalf("MarshalPlan: %v", err)
	}
	if encoded[len(encoded)-1] == '\n' {
		t.Fatal("canonical plan has a trailing newline")
	}
	wantPrefix := `{"v":1,"scheduler_protocol_version":1,"graph_version":1,"graph_id":"graph<fixture"`
	if len(encoded) < len(wantPrefix) || string(encoded[:len(wantPrefix)]) != wantPrefix {
		t.Fatalf("canonical prefix = %s, want %s", encoded, wantPrefix)
	}
}

func mustBuild(t *testing.T, spec Spec, manifest string, protocol uint16) Plan {
	t.Helper()
	plan, err := buildWithProtocol(spec, "graph-fixture-v1", manifest, protocol)
	if err != nil {
		t.Fatalf("buildWithProtocol: %v", err)
	}
	return plan
}

func assertDigestChanged(t *testing.T, first, second Plan) {
	t.Helper()
	if first.PlanSHA256 == second.PlanSHA256 {
		t.Fatalf("plan digest did not change: %s", first.PlanSHA256)
	}
}

func assertStrings(t *testing.T, got, want []string) {
	t.Helper()
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("strings = %v, want %v", got, want)
	}
}

func assertNestedStrings(t *testing.T, got, want [][]string) {
	t.Helper()
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("nested strings = %v, want %v", got, want)
	}
}

func assertPlanDigest(t *testing.T, plan Plan) {
	t.Helper()
	payload := planPayload{
		V: plan.V, SchedulerProtocolVersion: plan.SchedulerProtocolVersion,
		GraphVersion: plan.GraphVersion, GraphID: plan.GraphID,
		GraphManifestSHA256: plan.GraphManifestSHA256,
		AuthoredNodeIDs:     plan.AuthoredNodeIDs, Edges: plan.Edges, Waves: plan.Waves,
		ExecutionContractPresent:  plan.ExecutionContractPresent,
		DispatchAuthorityReleased: plan.DispatchAuthorityReleased,
	}
	encoded, err := encodeCanonical(payload)
	if err != nil {
		t.Fatalf("encode payload: %v", err)
	}
	sum := sha256.Sum256(append([]byte(planDigestDomain), encoded...))
	if want := hex.EncodeToString(sum[:]); plan.PlanSHA256 != want {
		t.Fatalf("plan digest = %s, want %s", plan.PlanSHA256, want)
	}
}
