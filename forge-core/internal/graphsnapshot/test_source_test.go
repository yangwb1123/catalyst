package graphsnapshot

import (
	"bytes"
	"encoding/json"
	"reflect"
	"strings"
	"testing"
)

func TestBuildTestSourceProjectsExactTopologyAndDisjointCoverage(t *testing.T) {
	graph, digest, observation := loadFixtureGraph(t)
	production, err := BuildTestSource(
		graph, digest, observation.Producer.RunID, "fixture-catalyst-go")
	if err != nil {
		t.Fatal(err)
	}
	snapshot := production.Envelope().Snapshot
	if len(snapshot.Nodes) != 11 || len(snapshot.Edges) != 14 ||
		len(snapshot.UnresolvedNodes) != 3 || len(snapshot.UnresolvedEdges) != 11 ||
		len(snapshot.ADR0062NodeCrosswalk) != 8 {
		t.Fatalf("test-source cardinalities = %d/%d/%d/%d/%d", len(snapshot.Nodes),
			len(snapshot.Edges), len(snapshot.UnresolvedNodes), len(snapshot.UnresolvedEdges),
			len(snapshot.ADR0062NodeCrosswalk))
	}
	assertTestSourceNodes(t, snapshot)
	assertTestSourceEdges(t, snapshot)
	assertTestSourceCoverage(t, snapshot)
}

func assertTestSourceNodes(t *testing.T, snapshot Snapshot) {
	t.Helper()
	p := findTypedNode(t, snapshot.Nodes, "test", "example.com/service", "internal/p", "p")
	external := findTypedNode(t, snapshot.Nodes, "test", "example.com/service", "internal/p", "p_test")
	if p.NodeID == external.NodeID || len(p.SourceLocators) != 1 || len(external.SourceLocators) != 1 {
		t.Fatalf("p/p_test source sets collapsed: %#v %#v", p, external)
	}
	if p.SourceLocators[0].Path != "service/internal/p/p_test.go" ||
		external.SourceLocators[0].Path != "service/internal/p/external_test.go" ||
		p.SourceLocators[0].Role != "test" || external.SourceLocators[0].Role != "test" {
		t.Fatalf("test-only locator sets drifted: %#v %#v", p.SourceLocators, external.SourceLocators)
	}
	for _, node := range snapshot.Nodes {
		if node.NodeType == "test" && node.IdentityProfileID !=
			"go-test-source-set-module-relative-directory-package-name-v1" {
			t.Fatalf("test identity profile = %q", node.IdentityProfileID)
		}
	}
}

func assertTestSourceEdges(t *testing.T, snapshot Snapshot) {
	t.Helper()
	nodeTypes := map[string]string{}
	for _, node := range snapshot.Nodes {
		nodeTypes[node.NodeID] = node.NodeType
	}
	testContains := 0
	for _, edge := range snapshot.Edges {
		if edge.Relation != "contains" && edge.Relation != "depends_on" {
			t.Fatalf("inferred relation %q", edge.Relation)
		}
		if edge.Relation == "contains" && nodeTypes[edge.ToNodeID] == "test" {
			testContains++
			if nodeTypes[edge.FromNodeID] != "module" || edge.SourceRole != nil ||
				edge.ImportDiscriminator != nil || edge.ParallelDiscriminator != "contains" {
				t.Fatalf("test contains edge drifted: %#v", edge)
			}
		}
		if edge.Relation == "depends_on" && nodeTypes[edge.FromNodeID] != "package" {
			t.Fatalf("legacy dependency endpoint was rewritten: %#v", edge)
		}
	}
	if testContains != 2 {
		t.Fatalf("module->test contains count = %d", testContains)
	}
}

func assertTestSourceCoverage(t *testing.T, snapshot Snapshot) {
	t.Helper()
	goSurface := findSurface(t, snapshot.Coverage, "go_module_package_lexical")
	testSurface := findSurface(t, snapshot.Coverage, "test_verification")
	if goSurface.NodeCount != 9 || goSurface.EdgeCount != 10 ||
		testSurface.NodeCount != 2 || testSurface.EdgeCount != 4 ||
		goSurface.NodeCount+testSurface.NodeCount != int64(len(snapshot.Nodes)) ||
		goSurface.EdgeCount+testSurface.EdgeCount != int64(len(snapshot.Edges)) {
		t.Fatalf("coverage partition = Go %#v Test %#v", goSurface, testSurface)
	}
	if !contains(goSurface.ReasonCodes, "go_file_diagnostic_present") ||
		contains(testSurface.ReasonCodes, "go_file_diagnostic_present") ||
		!contains(testSurface.ReasonCodes, "stdlib_candidate_dependency_present") ||
		!contains(testSurface.ReasonCodes, "nested_module_boundary_present") ||
		!contains(testSurface.ReasonCodes, "nonregular_go_entries_not_located") {
		t.Fatalf("conditional coverage partition drifted: Go %#v Test %#v",
			goSurface.ReasonCodes, testSurface.ReasonCodes)
	}
}

func TestTestSourceProfileKeepsStableIDsAndBindsFullRecords(t *testing.T) {
	graph, digest, observation := loadFixtureGraph(t)
	legacy, err := Build(graph, digest, observation.Producer.RunID, "fixture-catalyst-go")
	if err != nil {
		t.Fatal(err)
	}
	testSource, err := BuildTestSource(graph, digest, observation.Producer.RunID, "fixture-catalyst-go")
	if err != nil {
		t.Fatal(err)
	}
	left, right := legacy.Envelope().Snapshot, testSource.Envelope().Snapshot
	if !reflect.DeepEqual(left.Sources, right.Sources) ||
		left.SourceSetSHA256 != right.SourceSetSHA256 ||
		reflect.DeepEqual(left.Extractors, right.Extractors) ||
		left.Extractors[0].ExtractorSHA256 == right.Extractors[0].ExtractorSHA256 ||
		!reflect.DeepEqual(left.ADR0062NodeCrosswalk, right.ADR0062NodeCrosswalk) {
		t.Fatal("source/crosswalk stability or extractor profile binding drifted")
	}
	assertStableNodeRecords(t, left.Nodes, right.Nodes)
	assertStableEdgeRecords(t, left.Edges, right.Edges)
	assertStableUnresolvedRecords(t, left, right)
}

func assertStableNodeRecords(t *testing.T, legacy, current []Node) {
	t.Helper()
	byID := map[string]Node{}
	for _, node := range current {
		byID[node.NodeID] = node
	}
	for _, node := range legacy {
		other, exists := byID[node.NodeID]
		if !exists || node.NodeIdentitySHA256 != other.NodeIdentitySHA256 ||
			node.NodeSHA256 == other.NodeSHA256 {
			t.Fatalf("stable/profile-bound node drifted: %#v %#v", node, other)
		}
	}
}

func assertStableEdgeRecords(t *testing.T, legacy, current []Edge) {
	t.Helper()
	byID := map[string]Edge{}
	for _, edge := range current {
		byID[edge.EdgeID] = edge
	}
	for _, edge := range legacy {
		other, exists := byID[edge.EdgeID]
		if !exists || edge.EdgeIdentitySHA256 != other.EdgeIdentitySHA256 ||
			edge.EdgeSHA256 == other.EdgeSHA256 {
			t.Fatalf("stable/profile-bound edge drifted: %#v %#v", edge, other)
		}
	}
}

func assertStableUnresolvedRecords(t *testing.T, legacy, current Snapshot) {
	t.Helper()
	nodes := map[string]UnresolvedNode{}
	for _, item := range current.UnresolvedNodes {
		nodes[item.UnresolvedNodeID] = item
	}
	for _, item := range legacy.UnresolvedNodes {
		other, exists := nodes[item.UnresolvedNodeID]
		if !exists || item.UnresolvedNodeIdentitySHA256 != other.UnresolvedNodeIdentitySHA256 ||
			item.UnresolvedNodeSHA256 == other.UnresolvedNodeSHA256 {
			t.Fatalf("stable/profile-bound unresolved node drifted")
		}
	}
	edges := map[string]UnresolvedEdge{}
	for _, item := range current.UnresolvedEdges {
		edges[item.UnresolvedEdgeID] = item
	}
	for _, item := range legacy.UnresolvedEdges {
		other, exists := edges[item.UnresolvedEdgeID]
		if !exists || item.UnresolvedEdgeIdentitySHA256 != other.UnresolvedEdgeIdentitySHA256 ||
			item.UnresolvedEdgeSHA256 == other.UnresolvedEdgeSHA256 {
			t.Fatalf("stable/profile-bound unresolved edge drifted")
		}
	}
}

func TestTestSourceEmptyUnresolvedSetDigestRemainsDomainStable(t *testing.T) {
	observation := minimalObservation(".", "root.go", "compile")
	raw, digest := marshalObservation(t, observation)
	legacy, err := Build(raw, digest, observation.Producer.RunID, "fixture-root")
	if err != nil {
		t.Fatal(err)
	}
	testSource, err := BuildTestSource(raw, digest, observation.Producer.RunID, "fixture-root")
	if err != nil {
		t.Fatal(err)
	}
	left, right := legacy.Envelope().Snapshot, testSource.Envelope().Snapshot
	if left.UnresolvedNodeSetSHA256 != right.UnresolvedNodeSetSHA256 ||
		left.UnresolvedEdgeSetSHA256 != right.UnresolvedEdgeSetSHA256 {
		t.Fatal("empty unresolved set domains changed across profiles")
	}
}

func TestTestDiagnosticNeverCreatesOrAttachesTestSourceSet(t *testing.T) {
	graph, digest, observation := loadFixtureGraph(t)
	_ = graph
	_ = digest
	observation.Diagnostics[0].Path = "service/internal/broken/bad_test.go"
	raw, changedDigest := marshalObservation(t, observation)
	production, err := BuildTestSource(
		raw, changedDigest, observation.Producer.RunID, "fixture-catalyst-go")
	if err != nil {
		t.Fatal(err)
	}
	snapshot := production.Envelope().Snapshot
	testNodes := 0
	for _, node := range snapshot.Nodes {
		if node.NodeType == "test" {
			testNodes++
			for _, locator := range node.SourceLocators {
				if locator.Path == observation.Diagnostics[0].Path {
					t.Fatal("diagnostic was attached to a successful test source set")
				}
			}
		}
	}
	if testNodes != 2 {
		t.Fatalf("diagnostic changed test source-set count to %d", testNodes)
	}
	testSurface := findSurface(t, snapshot.Coverage, "test_verification")
	if !contains(testSurface.ReasonCodes, "go_file_diagnostic_present") {
		t.Fatal("test diagnostic gap was not attributed to the test surface")
	}
}

func TestTestSourceRootDotAndZeroCountSurfaceStayLiteralAndPartial(t *testing.T) {
	testOnly := minimalObservation(".", "root_test.go", "test")
	raw, digest := marshalObservation(t, testOnly)
	production, err := BuildTestSource(raw, digest, testOnly.Producer.RunID, "fixture-root")
	if err != nil {
		t.Fatal(err)
	}
	node := findTypedNode(t, production.Envelope().Snapshot.Nodes,
		"test", "example.com/root", ".", "rootpkg")
	if len(node.SourceLocators) != 1 || node.SourceLocators[0].Path != "root_test.go" {
		t.Fatalf("literal root test source set = %#v", node)
	}
	compileOnly := minimalObservation(".", "root.go", "compile")
	raw, digest = marshalObservation(t, compileOnly)
	production, err = BuildTestSource(raw, digest, compileOnly.Producer.RunID, "fixture-root")
	if err != nil {
		t.Fatal(err)
	}
	surface := findSurface(t, production.Envelope().Snapshot.Coverage, "test_verification")
	if surface.Status != "partial" || surface.NodeCount != 0 || surface.EdgeCount != 0 ||
		!contains(surface.ReasonCodes, "test_execution_not_observed") {
		t.Fatalf("zero-count test surface gained a conclusion: %#v", surface)
	}
}

func TestDecodeTestSourceIsStrictAndEndpointsDoNotFallback(t *testing.T) {
	graph, digest, observation := loadFixtureGraph(t)
	legacy, err := Build(graph, digest, observation.Producer.RunID, "fixture-catalyst-go")
	if err != nil {
		t.Fatal(err)
	}
	testSource, err := BuildTestSource(graph, digest, observation.Producer.RunID, "fixture-catalyst-go")
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := DecodeTestSource(testSource.JSON())
	if err != nil || !bytes.Equal(decoded.JSON(), testSource.JSON()) {
		t.Fatalf("test-source decode = %v", err)
	}
	if _, err := Decode(testSource.JSON()); err == nil ||
		!strings.HasPrefix(err.Error(), "unsupported_profile:") {
		t.Fatalf("legacy endpoint accepted test-source transport: %v", err)
	}
	if _, err := DecodeTestSource(legacy.JSON()); err == nil ||
		!strings.HasPrefix(err.Error(), "unsupported_profile:") {
		t.Fatalf("test-source endpoint accepted legacy transport: %v", err)
	}
}

func TestDecodeTestSourceRejectsFullyResealedSemanticAndShapeTamper(t *testing.T) {
	value := testSourceFixture(t).Envelope()
	value.Snapshot.Result += " tampered"
	value.Snapshot.SnapshotSHA256 = ""
	value.Snapshot.SnapshotSHA256, _ = digestValue(snapshotDomain, value.Snapshot)
	value.EnvelopeSHA256 = ""
	preimage, _ := canonicalJSON(value, maxEnvelopeBytes)
	value.EnvelopeSHA256 = domainDigest(testSourceEnvelopeDomain, preimage)
	raw, _ := canonicalJSON(value, maxEnvelopeBytes)
	if _, err := DecodeTestSource(raw); err == nil || !strings.Contains(err.Error(), "reconstruct") {
		t.Fatalf("fully resealed test-source tamper = %v", err)
	}
	shape := genericTestSourceEnvelope(t)
	shape["future_field"] = "future"
	raw, _ = canonicalJSON(shape, maxEnvelopeBytes)
	if _, err := DecodeTestSource(raw); err == nil || strings.HasPrefix(err.Error(), "unsupported_profile:") {
		t.Fatalf("supported-profile unknown field classified as %v", err)
	}
}

func TestTestSourceFutureProfileClassificationAndCanonicalPrecedence(t *testing.T) {
	value := genericTestSourceEnvelope(t)
	value["future_field"] = "future"
	setNestedString(value, []string{"snapshot", "profile_id"}, "future-profile/v2")
	value["snapshot"].(map[string]any)["nodes"] = make([]any, maxTestSourceNodes+1)
	raw, err := canonicalJSON(value, maxEnvelopeBytes)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := DecodeTestSource(raw); err == nil ||
		!strings.HasPrefix(err.Error(), "unsupported_profile:") {
		t.Fatalf("future test-source profile classified as %v", err)
	}
	noncanonical := bytes.Replace(raw, []byte(`"expires_at_unix_ms":1786320000123`),
		[]byte(`"expires_at_unix_ms":-0`), 1)
	if _, err := DecodeTestSource(noncanonical); err == nil ||
		strings.HasPrefix(err.Error(), "unsupported_profile:") {
		t.Fatalf("negative zero was masked by future profile: %v", err)
	}
}

func TestLegacyDecodeClassifiesNewProfileBeforeLegacyEdgeUnionBound(t *testing.T) {
	value := genericTestSourceEnvelope(t)
	value["snapshot"].(map[string]any)["edges"] = make([]any, maxEdges+1)
	raw, err := canonicalJSON(value, maxEnvelopeBytes)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := Decode(raw); err == nil ||
		!strings.HasPrefix(err.Error(), "unsupported_profile:") {
		t.Fatalf("new profile with %d edges classified as %v", maxEdges+1, err)
	}
}

func TestBuildTestSourceFutureGraphPrecedesInvalidProjectValidation(t *testing.T) {
	graph, _, observation := loadFixtureGraph(t)
	decoder := json.NewDecoder(bytes.NewReader(graph))
	decoder.UseNumber()
	var value map[string]any
	if err := decoder.Decode(&value); err != nil {
		t.Fatal(err)
	}
	value["api_version"] = "future-graph/v2"
	value["profile_id"] = "future-profile/v2"
	value["future_field"] = "future"
	raw, err := canonicalJSON(value, maxGraphBytes)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := BuildTestSource(raw, strings.Repeat("f", 64),
		observation.Producer.RunID, "INVALID PROJECT"); err == nil ||
		!strings.HasPrefix(err.Error(), "unsupported_profile:") {
		t.Fatalf("future graph with invalid project classified as %v", err)
	}
}

func TestTestSourceWalkerUsesDedicatedAggregateBounds(t *testing.T) {
	if err := validateEnvelopeJSONShapeForProfile(
		syntheticArrayEnvelope("nodes", maxTestSourceNodes), testSourceProfile); err != nil {
		t.Fatalf("node N accepted incorrectly: %v", err)
	}
	if err := validateEnvelopeJSONShapeForProfile(
		syntheticArrayEnvelope("nodes", maxTestSourceNodes+1), testSourceProfile); err == nil {
		t.Fatal("test-source node N+1 was accepted")
	}
	if err := validateEnvelopeJSONShapeForProfile(
		syntheticArrayEnvelope("edges", maxTestSourceEdges), testSourceProfile); err != nil {
		t.Fatalf("edge N accepted incorrectly: %v", err)
	}
	if err := validateEnvelopeJSONShapeForProfile(
		syntheticArrayEnvelope("edges", maxTestSourceEdges+1), testSourceProfile); err == nil {
		t.Fatal("test-source edge N+1 was accepted")
	}
	if err := validateEnvelopeJSONShapeForProfile(
		syntheticLocatorEnvelope(maxTestSourceAggregateLocators), testSourceProfile); err != nil {
		t.Fatalf("aggregate locator N rejected: %v", err)
	}
	if err := validateEnvelopeJSONShapeForProfile(
		syntheticLocatorEnvelope(maxTestSourceAggregateLocators+1), testSourceProfile); err == nil {
		t.Fatal("test-source aggregate locator N+1 was accepted")
	}
}

func genericTestSourceEnvelope(t *testing.T) map[string]any {
	t.Helper()
	production := testSourceFixture(t)
	decoder := json.NewDecoder(bytes.NewReader(production.JSON()))
	decoder.UseNumber()
	var value map[string]any
	if err := decoder.Decode(&value); err != nil {
		t.Fatal(err)
	}
	return value
}

func testSourceFixture(t *testing.T) *Production {
	t.Helper()
	graph, digest, observation := loadFixtureGraph(t)
	production, err := BuildTestSource(
		graph, digest, observation.Producer.RunID, "fixture-catalyst-go")
	if err != nil {
		t.Fatal(err)
	}
	return production
}

func findTypedNode(t *testing.T, values []Node, nodeType string, components ...string) Node {
	t.Helper()
	for _, value := range values {
		if value.NodeType == nodeType && strings.Join(value.QualifiedNameComponents, "\x00") ==
			strings.Join(components, "\x00") {
			return value
		}
	}
	t.Fatalf("%s node components %#v are absent", nodeType, components)
	return Node{}
}

func findSurface(t *testing.T, value Coverage, name string) CoverageSurface {
	t.Helper()
	for _, surface := range value.Surfaces {
		if surface.Surface == name {
			return surface
		}
	}
	t.Fatalf("coverage surface %q is absent", name)
	return CoverageSurface{}
}
