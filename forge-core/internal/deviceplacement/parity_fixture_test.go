package deviceplacement

import (
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

type placementParityFixture struct {
	SchemaVersion    string                     `json:"schema_version"`
	EvaluatedAtMS    int64                      `json:"evaluated_at_ms"`
	MaxSnapshotAgeMS int64                      `json:"max_snapshot_age_ms"`
	Owner            Owner                      `json:"owner"`
	Requirements     Requirements               `json:"requirements"`
	Candidates       []placementParityCandidate `json:"candidates"`
	Expected         []placementParityExpected  `json:"expected"`
}

type placementParityCandidate struct {
	InstanceID string `json:"instance_id"`
	Device     Device `json:"device"`
}

type placementParityExpected struct {
	DeviceID            string   `json:"device_id"`
	MatchesRequirements bool     `json:"matches_requirements"`
	ExclusionReasons    []string `json:"exclusion_reasons"`
}

func TestPolicyParityFixture(t *testing.T) {
	testPolicyParityFixture(t, "forge-device-placement-policy-parity-v1.json")
}

func TestGpuPolicyParityFixture(t *testing.T) {
	testPolicyParityFixture(t, "forge-device-placement-gpu-policy-parity-v1.json")
}

func testPolicyParityFixture(t *testing.T, name string) {
	t.Helper()
	path := filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", name)
	encoded, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var fixture placementParityFixture
	if err := json.Unmarshal(encoded, &fixture); err != nil {
		t.Fatalf("decode shared placement fixture: %v", err)
	}
	if fixture.SchemaVersion != "forge.device-placement-policy-parity-test/v1" {
		t.Fatalf("unexpected fixture schema %q", fixture.SchemaVersion)
	}
	request := Request{
		SchemaVersion: RequestSchemaVersion, EvaluatedAtMS: fixture.EvaluatedAtMS,
		Owner: fixture.Owner, MaxSnapshotAgeMS: fixture.MaxSnapshotAgeMS,
		Requirements: fixture.Requirements,
		Devices:      make([]Device, 0, len(fixture.Candidates)),
	}
	for _, candidate := range fixture.Candidates {
		request.Devices = append(request.Devices, candidate.Device)
	}
	result, err := Evaluate(request)
	if err != nil {
		t.Fatalf("Evaluate shared placement fixture: %v", err)
	}
	if len(result.DeviceResults) != len(fixture.Expected) {
		t.Fatalf("got %d candidate results, want %d", len(result.DeviceResults), len(fixture.Expected))
	}
	for index, expected := range fixture.Expected {
		actual := result.DeviceResults[index]
		if actual.DeviceID != expected.DeviceID || actual.MatchesRequirements != expected.MatchesRequirements ||
			!reflect.DeepEqual(actual.ExclusionReasons, expected.ExclusionReasons) {
			t.Errorf("candidate %d = %#v, want %#v", index, actual, expected)
		}
	}
	if result.ExecutionAuthorized || result.ReservationCreated || result.DispatchPerformed {
		t.Fatal("shared policy fixture must remain non-authoritative")
	}
}
