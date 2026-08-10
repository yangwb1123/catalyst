package commandobservationevidencecontract

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"forgeos/forge-core/internal/governancecontract"
)

type goldenFixture struct {
	APIVersion string          `json:"api_version"`
	Expected   goldenExpected  `json:"expected"`
	Request    json.RawMessage `json:"request"`
}

type goldenExpected struct {
	CanonicalCommandJSON        string `json:"canonical_command_json"`
	CanonicalEvidenceRecordJSON string `json:"canonical_evidence_record_json"`
	CanonicalObservationJSON    string `json:"canonical_observation_json"`
	CanonicalRequestJSON        string `json:"canonical_request_json"`
	CommandSHA256               string `json:"command_sha256"`
	EvidenceRecordSHA256        string `json:"evidence_record_sha256"`
	RequestSHA256               string `json:"request_sha256"`
	Result                      string `json:"result"`
	SourceSnapshotSHA256        string `json:"source_snapshot_sha256"`
}

func TestGoldenCommandObservationEvidenceAdaptation(t *testing.T) {
	fixture := loadGoldenFixture(t)
	if fixture.APIVersion != "forgeos.governance.command-observation-evidence-adapter.fixture/v1" {
		t.Fatalf("fixture api_version = %q", fixture.APIVersion)
	}
	request := decodeFixtureRequest(t, fixture.Request)
	requestJSON, err := canonicalRequestJSON(request)
	if err != nil {
		t.Fatalf("canonicalRequestJSON: %v", err)
	}
	assertGoldenEqual(t, "request JSON", string(requestJSON), fixture.Expected.CanonicalRequestJSON)
	adaptation, err := AdaptCanonicalRequest(requestJSON)
	if err != nil {
		t.Fatalf("AdaptCanonicalRequest: %v", err)
	}
	assertGoldenAdaptation(t, adaptation, fixture.Expected)
}

func assertGoldenAdaptation(t *testing.T, adaptation *Adaptation, expected goldenExpected) {
	t.Helper()
	assertGoldenEqual(t, "command JSON", string(adaptation.CommandJSON()), expected.CanonicalCommandJSON)
	assertGoldenEqual(t, "observation JSON", string(adaptation.ObservationJSON()), expected.CanonicalObservationJSON)
	assertGoldenEqual(t, "request JSON", string(adaptation.RequestJSON()), expected.CanonicalRequestJSON)
	assertGoldenEqual(t, "command digest", adaptation.CommandSHA256, expected.CommandSHA256)
	assertGoldenEqual(t, "source digest", adaptation.SourceSnapshotSHA256, expected.SourceSnapshotSHA256)
	assertGoldenEqual(t, "request digest", adaptation.RequestSHA256, expected.RequestSHA256)
	assertGoldenEqual(t, "result", adaptation.Result, expected.Result)
	assertGoldenEqual(t, "evidence digest", adaptation.Evidence.Digest(), expected.EvidenceRecordSHA256)
	assertGoldenEqual(t, "evidence JSON", string(adaptation.EvidenceJSON()), expected.CanonicalEvidenceRecordJSON)
	if _, err := governancecontract.DecodeRecord(adaptation.EvidenceJSON()); err != nil {
		t.Fatalf("DecodeRecord(golden evidence): %v", err)
	}
}

func decodeFixtureRequest(t *testing.T, raw json.RawMessage) Request {
	t.Helper()
	var request Request
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&request); err != nil {
		t.Fatalf("decode fixture request: %v", err)
	}
	return request
}

func loadGoldenFixture(t *testing.T) goldenFixture {
	t.Helper()
	path := filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", "command-observation-evidence-adapter-v1.json")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read fixture: %v", err)
	}
	var fixture goldenFixture
	if err := json.Unmarshal(data, &fixture); err != nil {
		t.Fatalf("decode fixture: %v", err)
	}
	return fixture
}

func assertGoldenEqual(t *testing.T, name, got, want string) {
	t.Helper()
	if got != want {
		t.Fatalf("%s mismatch\ngot:  %s\nwant: %s", name, got, want)
	}
}
