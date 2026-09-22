package appserver

import (
	"encoding/json"
	"os"
	"path/filepath"
	"runtime"
	"testing"

	"forgeos/forge-core/internal/deviceplacement"
)

// multiInstancePlacementRequest reuses the canonical nine-candidate fixture
// while rebinding its declared owner to the test issuer. The fixture remains
// caller-supplied data: it is never read as live inventory.
func multiInstancePlacementRequest(
	t *testing.T,
	issuer, subject, tenant string,
) (map[string]any, []string) {
	t.Helper()
	_, source, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("resolve multi-instance fixture source")
	}
	fixturePath := filepath.Join(
		filepath.Dir(source), "..", "..", "..", "docs", "contracts", "fixtures",
		"forge-session-placement-observation-v1.json",
	)
	contents, err := os.ReadFile(fixturePath)
	if err != nil {
		t.Fatalf("read multi-instance placement fixture: %v", err)
	}
	var root map[string]any
	if err := json.Unmarshal(contents, &root); err != nil {
		t.Fatalf("decode multi-instance placement fixture: %v", err)
	}
	fixturePlacement, ok := root["placement"].(map[string]any)
	if !ok {
		t.Fatal("multi-instance placement fixture is missing placement")
	}
	rawCandidates, ok := fixturePlacement["candidates"].([]any)
	if !ok || len(rawCandidates) != 9 {
		t.Fatalf("multi-instance placement candidates=%T/%d, want 9", fixturePlacement["candidates"], len(rawCandidates))
	}
	owner := map[string]any{
		"issuer": issuer, "subject": subject, "tenant_id": tenant,
	}
	devices := make([]any, 0, len(rawCandidates))
	instanceIDs := make([]string, 0, len(rawCandidates))
	for index, raw := range rawCandidates {
		candidate, ok := raw.(map[string]any)
		if !ok {
			t.Fatalf("multi-instance candidate[%d] has type %T", index, raw)
		}
		instanceID, ok := candidate["instance_id"].(string)
		if !ok || instanceID == "" {
			t.Fatalf("multi-instance candidate[%d] has invalid instance id", index)
		}
		device, ok := candidate["device"].(map[string]any)
		if !ok {
			t.Fatalf("multi-instance candidate[%d] has invalid device", index)
		}
		device["owner"] = owner
		devices = append(devices, device)
		instanceIDs = append(instanceIDs, instanceID)
	}
	return map[string]any{
		"schema_version":      deviceplacement.RequestSchemaVersion,
		"evaluated_at_ms":     fixturePlacement["evaluated_at_ms"],
		"owner":               owner,
		"max_snapshot_age_ms": fixturePlacement["max_snapshot_age_ms"],
		"requirements":        fixturePlacement["requirements"],
		"devices":             devices,
	}, instanceIDs
}

func multiInstanceDevicePlacementPreviewBody(
	t *testing.T,
	issuer, subject, tenant string,
) string {
	t.Helper()
	placement, _ := multiInstancePlacementRequest(t, issuer, subject, tenant)
	encoded, err := json.Marshal(placement)
	if err != nil {
		t.Fatalf("encode multi-instance placement request: %v", err)
	}
	return string(encoded)
}

func multiInstanceSessionDeviceObservationPreviewBody(
	t *testing.T,
	issuer, subject, tenant, conversationID, runID string,
) string {
	t.Helper()
	placement, instanceIDs := multiInstancePlacementRequest(
		t, issuer, subject, tenant,
	)
	owner := placement["owner"]
	devices := placement["devices"].([]any)
	candidates := make([]any, 0, len(devices))
	for index, device := range devices {
		candidates = append(candidates, map[string]any{
			"instance_id": instanceIDs[index],
			"device":      device,
		})
	}
	request := map[string]any{
		"owner":           owner,
		"conversation_id": conversationID,
		"run_id":          runID,
		"placement":       placement,
		"candidates":      candidates,
	}
	encoded, err := json.Marshal(request)
	if err != nil {
		t.Fatalf("encode multi-instance session observation request: %v", err)
	}
	return string(encoded)
}
