package artifactevidencecontract

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
	CanonicalEvidenceRecordJSON string `json:"canonical_evidence_record_json"`
	CanonicalRequestJSON        string `json:"canonical_request_json"`
	CanonicalSourceJSON         string `json:"canonical_source_json"`
	EvidenceRecordSHA256        string `json:"evidence_record_sha256"`
	RequestSHA256               string `json:"request_sha256"`
	Result                      string `json:"result"`
	SourceSnapshotSHA256        string `json:"source_snapshot_sha256"`
}

func TestGoldenArtifactEvidenceAdaptation(t *testing.T) {
	fixture := loadGoldenFixture(t)
	if fixture.APIVersion != "forgeos.governance.artifact-evidence-adapter.fixture/v1" {
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
	assertGoldenEqual(t, "request digest", adaptation.RequestSHA256, expected.RequestSHA256)
	assertGoldenEqual(t, "source digest", adaptation.SourceSHA256, expected.SourceSnapshotSHA256)
	assertGoldenEqual(t, "result", adaptation.Result, expected.Result)
	assertGoldenEqual(t, "evidence digest", adaptation.Evidence.Digest(), expected.EvidenceRecordSHA256)
	assertGoldenEqual(t, "evidence JSON", string(adaptation.EvidenceJSON()), expected.CanonicalEvidenceRecordJSON)
	assertGoldenEqual(t, "source JSON", string(adaptation.SourceJSON()), expected.CanonicalSourceJSON)
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
	path := filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", "artifact-evidence-adapter-v1.json")
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
