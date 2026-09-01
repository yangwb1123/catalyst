package platformcorecontract

import (
	"bytes"
	"encoding/json"
	"os"
	"testing"
)

type goldenFixture struct {
	APIVersion      string          `json:"api_version"`
	ArtifactRef     ArtifactRef     `json:"artifact_ref"`
	CommandEnvelope CommandEnvelope `json:"command_envelope"`
	EventEnvelope   EventEnvelope   `json:"event_envelope"`
	Expected        goldenExpected  `json:"expected"`
}

type goldenExpected struct {
	ArtifactRefSHA256     string `json:"artifact_ref_sha256"`
	CommandEnvelopeSHA256 string `json:"command_envelope_sha256"`
	EventEnvelopeSHA256   string `json:"event_envelope_sha256"`
}

func loadGoldenFixture(t *testing.T) goldenFixture {
	t.Helper()
	raw, err := os.ReadFile("../../../docs/contracts/fixtures/platform-core-envelope-v1.json")
	if err != nil {
		t.Fatal(err)
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	decoder.DisallowUnknownFields()
	var fixture goldenFixture
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatal(err)
	}
	if err := normalizeEnvelopeMaps(&fixture.CommandEnvelope.Payload, &fixture.CommandEnvelope.Extensions); err != nil {
		t.Fatal(err)
	}
	if err := normalizeEnvelopeMaps(&fixture.EventEnvelope.Payload, &fixture.EventEnvelope.Extensions); err != nil {
		t.Fatal(err)
	}
	return fixture
}
