package graphsnapshot

import (
	"bytes"
	"encoding/json"
	"os"
	"testing"
)

type snapshotGolden struct {
	Expected struct {
		CanonicalEnvelopeJSON   string `json:"canonical_envelope_json"`
		CoverageSHA256          string `json:"coverage_sha256"`
		CrosswalkSetSHA256      string `json:"crosswalk_set_sha256"`
		EdgeSetSHA256           string `json:"edge_set_sha256"`
		EnvelopeSHA256          string `json:"envelope_sha256"`
		ExtractorSetSHA256      string `json:"extractor_set_sha256"`
		NodeSetSHA256           string `json:"node_set_sha256"`
		RequestSHA256           string `json:"request_sha256"`
		SnapshotID              string `json:"snapshot_id"`
		SnapshotIdentitySHA256  string `json:"snapshot_identity_sha256"`
		SnapshotSHA256          string `json:"snapshot_sha256"`
		SourceSetSHA256         string `json:"source_set_sha256"`
		UnresolvedEdgeSetSHA256 string `json:"unresolved_edge_set_sha256"`
		UnresolvedNodeSetSHA256 string `json:"unresolved_node_set_sha256"`
	} `json:"expected"`
	Input struct {
		CanonicalGraphObservationJSON string `json:"canonical_graph_observation_json"`
		GraphObservationSHA256        string `json:"graph_observation_sha256"`
		ProjectID                     string `json:"project_id"`
		RunID                         string `json:"run_id"`
	} `json:"input"`
}

func TestPythonGoGoldenEnvelopeAndEverySnapshotDigestMatch(t *testing.T) {
	raw, err := os.ReadFile("../../../docs/contracts/fixtures/graph-snapshot-v1.json")
	if err != nil {
		t.Fatal(err)
	}
	var fixture snapshotGolden
	if err := json.Unmarshal(raw, &fixture); err != nil {
		t.Fatal(err)
	}
	production, err := Build([]byte(fixture.Input.CanonicalGraphObservationJSON),
		fixture.Input.GraphObservationSHA256, fixture.Input.RunID, fixture.Input.ProjectID)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(production.JSON(), []byte(fixture.Expected.CanonicalEnvelopeJSON)) {
		t.Fatal("Go canonical envelope differs from the Python golden")
	}
	value := production.Envelope()
	actual := []string{
		value.Snapshot.CoverageSHA256, value.Snapshot.CrosswalkSetSHA256,
		value.Snapshot.EdgeSetSHA256, value.EnvelopeSHA256,
		value.Snapshot.ExtractorSetSHA256, value.Snapshot.NodeSetSHA256,
		value.Request.RequestSHA256, value.Snapshot.SnapshotID,
		value.Snapshot.SnapshotIdentitySHA256, value.Snapshot.SnapshotSHA256,
		value.Snapshot.SourceSetSHA256, value.Snapshot.UnresolvedEdgeSetSHA256,
		value.Snapshot.UnresolvedNodeSetSHA256,
	}
	want := []string{
		fixture.Expected.CoverageSHA256, fixture.Expected.CrosswalkSetSHA256,
		fixture.Expected.EdgeSetSHA256, fixture.Expected.EnvelopeSHA256,
		fixture.Expected.ExtractorSetSHA256, fixture.Expected.NodeSetSHA256,
		fixture.Expected.RequestSHA256, fixture.Expected.SnapshotID,
		fixture.Expected.SnapshotIdentitySHA256, fixture.Expected.SnapshotSHA256,
		fixture.Expected.SourceSetSHA256, fixture.Expected.UnresolvedEdgeSetSHA256,
		fixture.Expected.UnresolvedNodeSetSHA256,
	}
	for index := range want {
		if actual[index] != want[index] {
			t.Fatalf("golden digest %d = %q, want %q", index, actual[index], want[index])
		}
	}
}

func TestPythonGoTestSourceGoldenEnvelopeAndEverySnapshotDigestMatch(t *testing.T) {
	raw, err := os.ReadFile("../../../docs/contracts/fixtures/graph-snapshot-go-test-source-v1.json")
	if err != nil {
		t.Fatal(err)
	}
	var fixture snapshotGolden
	if err := json.Unmarshal(raw, &fixture); err != nil {
		t.Fatal(err)
	}
	production, err := BuildTestSource([]byte(fixture.Input.CanonicalGraphObservationJSON),
		fixture.Input.GraphObservationSHA256, fixture.Input.RunID, fixture.Input.ProjectID)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(production.JSON(), []byte(fixture.Expected.CanonicalEnvelopeJSON)) {
		t.Fatal("Go test-source envelope differs from the Python golden")
	}
	value := production.Envelope()
	actual := []string{
		value.Snapshot.CoverageSHA256, value.Snapshot.CrosswalkSetSHA256,
		value.Snapshot.EdgeSetSHA256, value.EnvelopeSHA256,
		value.Snapshot.ExtractorSetSHA256, value.Snapshot.NodeSetSHA256,
		value.Request.RequestSHA256, value.Snapshot.SnapshotID,
		value.Snapshot.SnapshotIdentitySHA256, value.Snapshot.SnapshotSHA256,
		value.Snapshot.SourceSetSHA256, value.Snapshot.UnresolvedEdgeSetSHA256,
		value.Snapshot.UnresolvedNodeSetSHA256,
	}
	want := []string{
		fixture.Expected.CoverageSHA256, fixture.Expected.CrosswalkSetSHA256,
		fixture.Expected.EdgeSetSHA256, fixture.Expected.EnvelopeSHA256,
		fixture.Expected.ExtractorSetSHA256, fixture.Expected.NodeSetSHA256,
		fixture.Expected.RequestSHA256, fixture.Expected.SnapshotID,
		fixture.Expected.SnapshotIdentitySHA256, fixture.Expected.SnapshotSHA256,
		fixture.Expected.SourceSetSHA256, fixture.Expected.UnresolvedEdgeSetSHA256,
		fixture.Expected.UnresolvedNodeSetSHA256,
	}
	for index := range want {
		if actual[index] != want[index] {
			t.Fatalf("test-source golden digest %d = %q, want %q", index, actual[index], want[index])
		}
	}
}
