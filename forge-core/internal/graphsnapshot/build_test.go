package graphsnapshot

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"
)

func TestBuildProjectsRichGraphWithExactPartialUnknownSemantics(t *testing.T) {
	production := buildFixture(t)
	value := production.Envelope()
	if len(value.Snapshot.Nodes) != 9 || len(value.Snapshot.Edges) != 12 ||
		len(value.Snapshot.UnresolvedNodes) != 3 || len(value.Snapshot.UnresolvedEdges) != 11 ||
		len(value.Snapshot.ADR0062NodeCrosswalk) != 8 {
		t.Fatalf("projection cardinalities = nodes %d edges %d unresolved %d/%d crosswalk %d",
			len(value.Snapshot.Nodes), len(value.Snapshot.Edges), len(value.Snapshot.UnresolvedNodes),
			len(value.Snapshot.UnresolvedEdges), len(value.Snapshot.ADR0062NodeCrosswalk))
	}
	if value.Snapshot.Coverage.Status != "partial" ||
		value.Snapshot.SystemKnowledgeStatus != "unknown" ||
		value.Snapshot.Freshness.Status != "unknown" || len(value.Snapshot.Coverage.Surfaces) != 11 {
		t.Fatalf("partial/unknown contract drifted: %#v", value.Snapshot.Coverage)
	}
	assertGoCoverage(t, value.Snapshot)
	assertParallelEdges(t, value.Snapshot.Edges)
}

func assertGoCoverage(t *testing.T, value Snapshot) {
	t.Helper()
	for _, surface := range value.Coverage.Surfaces {
		if surface.Surface == "go_module_package_lexical" {
			if surface.NodeCount != int64(len(value.Nodes)) || surface.EdgeCount != int64(len(value.Edges)) ||
				surface.Status != "partial" {
				t.Fatalf("Go coverage is not exact: %#v", surface)
			}
			return
		}
		if surface.Status != "not_observed" || surface.NodeCount != 0 || surface.EdgeCount != 0 {
			t.Fatalf("unobserved surface gained facts: %#v", surface)
		}
	}
	t.Fatal("Go coverage surface is absent")
}

func assertParallelEdges(t *testing.T, values []Edge) {
	t.Helper()
	byEndpoints := map[string][]Edge{}
	for _, edge := range values {
		if edge.Relation == "depends_on" {
			key := edge.FromNodeID + "\x00" + edge.ToNodeID
			byEndpoints[key] = append(byEndpoints[key], edge)
		}
	}
	for _, edges := range byEndpoints {
		if len(edges) == 2 && edges[0].SourceRole != nil && edges[1].SourceRole != nil &&
			*edges[0].SourceRole != *edges[1].SourceRole {
			if edges[0].EdgeID == edges[1].EdgeID {
				t.Fatal("compile/test parallel edges share an identity")
			}
			return
		}
	}
	t.Fatal("fixture compile/test parallel edge pair is absent")
}

func TestProjectScopedSemanticIdentityAndRootDot(t *testing.T) {
	observation := minimalObservation(".", "root.go", "compile")
	raw, digest := marshalObservation(t, observation)
	first, err := Build(raw, digest, observation.Producer.RunID, "0project")
	if err != nil {
		t.Fatal(err)
	}
	node := findNodeByComponents(t, first.Envelope().Snapshot.Nodes,
		"example.com/root", ".", "rootpkg")
	if node.NodeType != "package" {
		t.Fatalf("root node = %#v", node)
	}
	second, err := Build(raw, digest, observation.Producer.RunID, "1project")
	if err != nil {
		t.Fatal(err)
	}
	other := findNodeByComponents(t, second.Envelope().Snapshot.Nodes,
		"example.com/root", ".", "rootpkg")
	if node.NodeID == other.NodeID {
		t.Fatal("caller-declared project namespace did not scope semantic node ID")
	}
}

func TestStableNodeIDExcludesLocatorContentButRecordBindsIt(t *testing.T) {
	graph, digest, observation := loadFixtureGraph(t)
	first, err := Build(graph, digest, observation.Producer.RunID, "fixture-catalyst-go")
	if err != nil {
		t.Fatal(err)
	}
	observation.Files[0].ContentSHA256 = strings.Repeat("f", 64)
	changed, changedDigest := marshalObservation(t, observation)
	second, err := Build(changed, changedDigest, observation.Producer.RunID, "fixture-catalyst-go")
	if err != nil {
		t.Fatal(err)
	}
	components := []string{"example.com/service", "cmd/app", "main"}
	left := findNodeByComponents(t, first.Envelope().Snapshot.Nodes, components...)
	right := findNodeByComponents(t, second.Envelope().Snapshot.Nodes, components...)
	if left.NodeID != right.NodeID || left.NodeSHA256 == right.NodeSHA256 {
		t.Fatalf("stable/full identity split failed: %q/%q %q/%q",
			left.NodeID, right.NodeID, left.NodeSHA256, right.NodeSHA256)
	}
}

func TestStructuredSetDigestAndDefensiveCopies(t *testing.T) {
	production := buildFixture(t)
	value := production.Envelope()
	want, err := setDigest(nodeSetDomain, value.Snapshot.Nodes)
	if err != nil || want != value.Snapshot.NodeSetSHA256 {
		t.Fatalf("structured set digest = %q, %v", want, err)
	}
	bare, _ := json.Marshal(value.Snapshot.Nodes)
	if domainDigest(nodeSetDomain, bare) == want {
		t.Fatal("bare array incorrectly aliases structured set preimage")
	}
	jsonCopy := production.JSON()
	jsonCopy[0] = '['
	envelopeCopy := production.Envelope()
	envelopeCopy.Snapshot.Nodes[0].QualifiedNameComponents[0] = "mutated"
	if bytes.Equal(jsonCopy, production.JSON()) ||
		production.Envelope().Snapshot.Nodes[0].QualifiedNameComponents[0] == "mutated" {
		t.Fatal("Production exposed mutable internal storage")
	}
}

func TestSnapshotIdentityHasNoSelfDigestCycle(t *testing.T) {
	value := buildFixture(t).Envelope().Snapshot
	identity := snapshotIdentity{
		CoverageSHA256: value.CoverageSHA256, CrosswalkSetSHA256: value.CrosswalkSetSHA256,
		EdgeSetSHA256: value.EdgeSetSHA256, ExtractorSetSHA256: value.ExtractorSetSHA256,
		NodeSetSHA256: value.NodeSetSHA256, ProfileID: value.ProfileID,
		ProjectID: value.ProjectID, RequestSHA256: value.RequestSHA256,
		SourceSetSHA256:         value.SourceSetSHA256,
		UnresolvedEdgeSetSHA256: value.UnresolvedEdgeSetSHA256,
		UnresolvedNodeSetSHA256: value.UnresolvedNodeSetSHA256,
	}
	digest, err := digestValue(snapshotIdentityDomain, identity)
	if err != nil || digest != value.SnapshotIdentitySHA256 ||
		value.SnapshotID != "graph-snapshot-"+digest {
		t.Fatalf("snapshot independent identity = %q, %v", digest, err)
	}
	original := value.SnapshotIdentitySHA256
	value.SnapshotSHA256 = strings.Repeat("0", 64)
	digest, err = digestValue(snapshotIdentityDomain, identity)
	if err != nil || digest != original {
		t.Fatal("snapshot self digest leaked into independent identity")
	}
}

func TestUnknownGraphProfileClassifiesBeforeFutureShapeDecode(t *testing.T) {
	graph, _, observation := loadFixtureGraph(t)
	var value map[string]any
	decoder := json.NewDecoder(bytes.NewReader(graph))
	decoder.UseNumber()
	if err := decoder.Decode(&value); err != nil {
		t.Fatal(err)
	}
	value["api_version"] = "future-graph/v2"
	value["future_field"] = "future"
	raw, err := canonicalJSON(value, maxGraphBytes)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := Build(raw, strings.Repeat("f", 64), observation.Producer.RunID,
		"future-project"); err == nil || !strings.HasPrefix(err.Error(), "unsupported_profile:") {
		t.Fatalf("future graph classified as %v", err)
	}
	if _, err := Build(raw, strings.Repeat("f", 64), observation.Producer.RunID,
		"INVALID PROJECT"); err == nil || !strings.HasPrefix(err.Error(), "unsupported_profile:") {
		t.Fatalf("future graph with invalid project classified as %v", err)
	}
	noncanonical := bytes.Replace(raw, []byte(`"observed_at_unix_ms":1786320000123`),
		[]byte(`"observed_at_unix_ms":-0`), 1)
	if _, err := Build(noncanonical, strings.Repeat("f", 64), observation.Producer.RunID,
		"future-project"); err == nil || strings.HasPrefix(err.Error(), "unsupported_profile:") {
		t.Fatalf("future graph negative zero classified as %v", err)
	}
}

func TestCrosswalkIdentityCollisionsFailClosed(t *testing.T) {
	value := Crosswalk{ADR0062NodeID: "go-package-node-a", ADR0062NodeSHA256: "a",
		GraphNodeID: "graph-node-a"}
	cases := [][]Crosswalk{
		{value, {ADR0062NodeID: value.ADR0062NodeID, ADR0062NodeSHA256: "b", GraphNodeID: "graph-node-b"}},
		{value, {ADR0062NodeID: "go-package-node-b", ADR0062NodeSHA256: value.ADR0062NodeSHA256, GraphNodeID: "graph-node-b"}},
		{value, {ADR0062NodeID: "go-package-node-b", ADR0062NodeSHA256: "b", GraphNodeID: value.GraphNodeID}},
	}
	for _, candidate := range cases {
		if err := validateCrosswalkIdentities(candidate); err == nil {
			t.Fatal("crosswalk collision was accepted")
		}
	}
}
