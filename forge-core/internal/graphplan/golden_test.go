package graphplan

import (
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

type goldenFixture struct {
	V     uint16 `json:"v"`
	Input struct {
		GraphID             string `json:"graph_id"`
		GraphManifestSHA256 string `json:"graph_manifest_sha256"`
		Spec                Spec   `json:"spec"`
	} `json:"input"`
	ExpectedPlan                 Plan   `json:"expected_plan"`
	ExpectedCanonicalPayloadJSON string `json:"expected_canonical_payload_json"`
	ExpectedCanonicalPlanJSON    string `json:"expected_canonical_plan_json"`
}

func TestSharedCorePlanGolden(t *testing.T) {
	fixture := readGoldenFixture(t)
	if fixture.V != 1 {
		t.Fatalf("fixture version = %d, want 1", fixture.V)
	}
	plan, err := Build(
		fixture.Input.Spec,
		fixture.Input.GraphID,
		fixture.Input.GraphManifestSHA256,
	)
	if err != nil {
		t.Fatalf("Build golden: %v", err)
	}
	if !reflect.DeepEqual(plan, fixture.ExpectedPlan) {
		t.Fatalf("plan = %#v, want %#v", plan, fixture.ExpectedPlan)
	}
	assertGoldenBytes(t, plan, fixture)
}

func readGoldenFixture(t *testing.T) goldenFixture {
	t.Helper()
	path := filepath.Join(
		"..", "..", "..", "docs", "contracts", "fixtures",
		"group-agent-graph-core-plan-v1.json",
	)
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read shared golden: %v", err)
	}
	var fixture goldenFixture
	if err := json.Unmarshal(data, &fixture); err != nil {
		t.Fatalf("decode shared golden: %v", err)
	}
	return fixture
}

func assertGoldenBytes(t *testing.T, plan Plan, fixture goldenFixture) {
	t.Helper()
	encoded, err := MarshalPlan(plan)
	if err != nil {
		t.Fatalf("MarshalPlan golden: %v", err)
	}
	if string(encoded) != fixture.ExpectedCanonicalPlanJSON {
		t.Fatalf("canonical plan = %s, want %s", encoded, fixture.ExpectedCanonicalPlanJSON)
	}
	payload := planPayload{
		V: plan.V, SchedulerProtocolVersion: plan.SchedulerProtocolVersion,
		GraphVersion: plan.GraphVersion, GraphID: plan.GraphID,
		GraphManifestSHA256: plan.GraphManifestSHA256,
		AuthoredNodeIDs:     plan.AuthoredNodeIDs, Edges: plan.Edges, Waves: plan.Waves,
		ExecutionContractPresent:  plan.ExecutionContractPresent,
		DispatchAuthorityReleased: plan.DispatchAuthorityReleased,
	}
	payloadJSON, err := encodeCanonical(payload)
	if err != nil {
		t.Fatalf("encode golden payload: %v", err)
	}
	if string(payloadJSON) != fixture.ExpectedCanonicalPayloadJSON {
		t.Fatalf("canonical payload = %s, want %s", payloadJSON, fixture.ExpectedCanonicalPayloadJSON)
	}
}
