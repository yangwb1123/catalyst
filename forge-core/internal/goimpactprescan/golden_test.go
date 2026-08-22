package goimpactprescan

import (
	"bytes"
	"encoding/json"
	"os"
	"testing"
)

type impactFixture struct {
	APIVersion string `json:"api_version"`
	Expected   struct {
		CanonicalEnvelopeJSON string `json:"canonical_envelope_json"`
		EnvelopeSHA256        string `json:"envelope_sha256"`
		ReportSHA256          string `json:"report_sha256"`
		RequestSHA256         string `json:"request_sha256"`
	} `json:"expected"`
	Input struct {
		CanonicalGraphObservationJSON string   `json:"canonical_graph_observation_json"`
		ChangedPaths                  []string `json:"changed_paths"`
		GraphObservationSHA256        string   `json:"graph_observation_sha256"`
		RunID                         string   `json:"run_id"`
	} `json:"input"`
}

func TestGoldenImpactPrescanFixture(t *testing.T) {
	fixture := loadImpactFixture(t)
	production, err := Build(
		[]byte(fixture.Input.CanonicalGraphObservationJSON),
		fixture.Input.GraphObservationSHA256,
		fixture.Input.RunID,
		fixture.Input.ChangedPaths,
	)
	if err != nil {
		t.Fatal(err)
	}
	if production.SHA256() != fixture.Expected.EnvelopeSHA256 ||
		production.ReportSHA256() != fixture.Expected.ReportSHA256 ||
		production.Envelope().Request.RequestSHA256 != fixture.Expected.RequestSHA256 ||
		!bytes.Equal(production.JSON(), []byte(fixture.Expected.CanonicalEnvelopeJSON)) {
		t.Fatal("impact production does not match the frozen golden fixture")
	}
	if _, err := Decode([]byte(fixture.Expected.CanonicalEnvelopeJSON)); err != nil {
		t.Fatalf("decode frozen golden envelope: %v", err)
	}
}

func loadImpactFixture(t *testing.T) impactFixture {
	t.Helper()
	raw, err := os.ReadFile("../../../docs/contracts/fixtures/local-go-package-impact-prescan-v1.json")
	if err != nil {
		t.Fatal(err)
	}
	var fixture impactFixture
	if err := json.Unmarshal(raw, &fixture); err != nil {
		t.Fatal(err)
	}
	if fixture.APIVersion != "forgeos.governance.local-go-package-impact-prescan.fixture/v1" {
		t.Fatalf("fixture API = %q", fixture.APIVersion)
	}
	return fixture
}
