package appserver

import (
	"encoding/json"
	"testing"

	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// TestPersistedLifecycleRegistryPlacementPreviewContract pins the value
// boundary exposed by the registry-backed placement candidate. The registry
// image is read through the same owner-scoped source as the inventory
// candidates, then evaluated by the existing lossless v2 comparator. Keeping
// this test at the composition seam means Rust and Flutter can reuse the v2
// decision/authority contract without waiting for the HTTP route to exist.
func TestPersistedLifecycleRegistryPlacementPreviewContract(t *testing.T) {
	owner := deviceidentity.Owner{
		Issuer:   "https://id.example",
		Subject:  "account-42",
		TenantID: "tenant-slate",
	}
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{
		lifecycleRegistrySourceState(t, owner, "device-b", "runner-b", 1),
		lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1),
	})
	source := newPersistedLifecycleRegistryFileSetReadSource(registryPath, 200_000)
	principal := model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}
	observation, err := source.ReadOwnedDeviceInventoryV2(t.Context(), principal)
	if err != nil {
		t.Fatalf("read registry inventory v2: %v", err)
	}

	requirements := deviceplacement.Requirements{
		OS:                 "linux",
		Architecture:       "amd64",
		MinCPUCores:        1,
		MinMemoryBytes:     1,
		MinStorageBytes:    1,
		Runtime:            "oci",
		GPU:                deviceplacement.GPURequirement{},
		DataResidencyZones: []string{"us-west"},
		MinimumTrustZone:   "standard",
		SandboxFloor:       "container",
		ConcurrencySlots:   1,
	}
	evaluation, err := deviceplacement.EvaluatePersistedInventoryObservationV2(
		observation,
		deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
		requirements,
		200_000,
	)
	if err != nil {
		t.Fatalf("evaluate registry placement preview: %v", err)
	}
	if evaluation.SchemaVersion != deviceplacement.PersistedInventoryPlacementV2SchemaVersion ||
		evaluation.EvaluationMode != deviceplacement.PersistedInventoryPlacementV2EvaluationMode ||
		evaluation.SourceSchemaVersion != deviceplacement.SessionDeviceObservationInventoryV2SchemaVersion ||
		evaluation.EvaluatedAtMS != 200_000 ||
		evaluation.SelectedDeviceID != nil ||
		evaluation.SelectedInstanceID != nil ||
		evaluation.Authority != (deviceplacement.PersistedInventoryPlacementBatchAuthority{}) {
		t.Fatalf("registry placement preview envelope=%#v", evaluation)
	}
	if evaluation.Owner.Subject != owner.Subject || len(evaluation.Decisions) != 2 {
		t.Fatalf("registry placement preview owner/decisions=%#v", evaluation)
	}
	if evaluation.Decisions[0].DeviceID != "device-a" || evaluation.Decisions[1].DeviceID != "device-b" ||
		evaluation.EligibleCandidateCount != 0 {
		t.Fatalf("registry placement preview ordering/eligibility=%#v", evaluation)
	}
	for _, decision := range evaluation.Decisions {
		if !decision.OwnerDeclarationUnverified || !decision.DeviceAttributesUnverified ||
			decision.MatchesRequirements || len(decision.ExclusionReasons) == 0 {
			t.Fatalf("registry placement preview decision=%#v", decision)
		}
	}

	// The HTTP request is intentionally one exact object. Owner and
	// observation time come from authentication/server clock, so neither can
	// be supplied by a caller or accidentally become part of the write path.
	requestBytes, err := json.Marshal(struct {
		Requirements deviceplacement.Requirements `json:"requirements"`
	}{Requirements: requirements})
	if err != nil {
		t.Fatalf("marshal registry placement request: %v", err)
	}
	var request map[string]json.RawMessage
	if err := json.Unmarshal(requestBytes, &request); err != nil || len(request) != 1 {
		t.Fatalf("registry placement request shape=%s err=%v", requestBytes, err)
	}
	if _, ok := request["requirements"]; !ok {
		t.Fatalf("registry placement request omitted requirements: %s", requestBytes)
	}

	responseBytes, err := json.Marshal(evaluation)
	if err != nil {
		t.Fatalf("marshal registry placement response: %v", err)
	}
	var response map[string]json.RawMessage
	if err := json.Unmarshal(responseBytes, &response); err != nil {
		t.Fatalf("decode registry placement response: %v", err)
	}
	for _, field := range []string{
		"schema_version", "evaluation_mode", "source_schema_version", "evaluation_owner",
		"evaluated_at_ms", "notice", "decisions", "eligible_candidate_count",
		"selected_device_id", "selected_instance_id", "authority",
	} {
		if _, ok := response[field]; !ok {
			t.Fatalf("registry placement response omitted %q: %s", field, responseBytes)
		}
	}
	if len(response) != 11 {
		t.Fatalf("registry placement response fields=%d want 11: %s", len(response), responseBytes)
	}
	if string(response["selected_device_id"]) != "null" || string(response["selected_instance_id"]) != "null" {
		t.Fatalf("registry placement response selected a target: %s", responseBytes)
	}
}
