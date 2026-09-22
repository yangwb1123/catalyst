package appserver

import (
	"encoding/json"
	"net/http"
	"strings"
	"testing"

	"forgeos/forge-core/internal/deviceplacement"
)

func TestSessionDeviceObservationPreviewRejectsDuplicateAndUnsafeDeclarations(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	sessions := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}
	path := "/api/v1/conversations/conversation-001/runs/run-001/device-observation/preview"
	base := multiInstanceSessionDeviceObservationPreviewBody(
		t, identity.issuer, "account-42", "tenant-slate", "conversation-001", "run-001",
	)
	tests := []struct {
		name string
		body string
	}{
		{
			name: "duplicate json key",
			body: strings.Replace(base,
				`"conversation_id":"conversation-001","owner":`,
				`"conversation_id":"conversation-001","owner":{},"owner":`, 1),
		},
		{
			name: "duplicate instance id",
			body: mutateSessionDeviceObservationBody(t, base, func(value map[string]any) {
				candidates := value["candidates"].([]any)
				first := candidates[0].(map[string]any)
				second := candidates[1].(map[string]any)
				second["instance_id"] = first["instance_id"]
			}),
		},
		{
			name: "duplicate device id",
			body: mutateSessionDeviceObservationBody(t, base, func(value map[string]any) {
				candidates := value["candidates"].([]any)
				firstDevice := candidates[0].(map[string]any)["device"].(map[string]any)
				secondDevice := candidates[1].(map[string]any)["device"].(map[string]any)
				secondDevice["device_id"] = firstDevice["device_id"]
				devices := value["placement"].(map[string]any)["devices"].([]any)
				devices[1].(map[string]any)["device_id"] = firstDevice["device_id"]
			}),
		},
		{
			name: "unsafe evaluated timestamp",
			body: mutateSessionDeviceObservationBody(t, base, func(value map[string]any) {
				value["placement"].(map[string]any)["evaluated_at_ms"] = deviceplacement.MaxSafeIntegerMS + 1
			}),
		},
		{
			name: "unsafe resource integer",
			body: mutateSessionDeviceObservationBody(t, base, func(value map[string]any) {
				mutateSessionDeviceObservationDevices(t, value, func(device map[string]any) {
					device["available_memory_bytes"] = deviceplacement.MaxSafeIntegerMS + 1
				})
			}),
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			response := requestConversationAPI(t, routes, identity, http.MethodPost, path,
				"forge:conversations:read", "application/json", "", test.body)
			if response.Code != http.StatusBadRequest {
				t.Fatalf("adversarial observation status=%d body=%q", response.Code, response.Body.String())
			}
		})
	}
}

func TestSessionDeviceObservationPreviewAcceptsZeroCapacityUnknownState(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	sessions := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}
	body := mutateSessionDeviceObservationBody(t,
		multiInstanceSessionDeviceObservationPreviewBody(
			t, identity.issuer, "account-42", "tenant-slate", "conversation-001", "run-001",
		), func(value map[string]any) {
			mutateSessionDeviceObservationDevices(t, value, func(device map[string]any) {
				device["approval_state"] = "unknown"
				device["cordon_state"] = "unknown"
				device["liveness"] = "unknown"
				device["snapshot_observed_at_ms"] = 0
				device["lease_expires_at_ms"] = 0
				device["available_cpu_cores"] = 0
				device["available_memory_bytes"] = 0
				device["available_storage_bytes"] = 0
				device["trust_zone"] = "unknown"
				device["concurrency_limit"] = 0
				device["active_concurrency"] = 0
			})
		},
	)
	response := requestConversationAPI(t, routes, identity, http.MethodPost,
		"/api/v1/conversations/conversation-001/runs/run-001/device-observation/preview",
		"forge:conversations:read", "application/json", "", body)
	if response.Code != http.StatusOK {
		t.Fatalf("zero-capacity observation status=%d body=%q", response.Code, response.Body.String())
	}
	var observation deviceplacement.SessionDeviceObservation
	if err := json.Unmarshal(response.Body.Bytes(), &observation); err != nil {
		t.Fatalf("decode zero-capacity observation: %v", err)
	}
	if err := deviceplacement.ValidateSessionDeviceObservation(observation); err != nil {
		t.Fatalf("validate zero-capacity observation: %v", err)
	}
	if observation.ResourceSummary.DeviceCount != 9 || observation.ResourceSummary.RunnerInstanceCount != 9 ||
		observation.ResourceSummary.AvailableCPUCores != 0 || observation.ResourceSummary.AvailableMemoryBytes != 0 ||
		observation.ResourceSummary.AvailableStorageBytes != 0 || observation.ResourceSummary.AvailableGPUCount != 0 ||
		observation.ResourceSummary.AvailableGPUMemoryBytes != 0 || observation.ResourceSummary.EligibleDeviceCount != 0 ||
		observation.ResourceSummary.EligibleInstanceCount != 0 || observation.Authority != (deviceplacement.SessionPlacementAuthority{}) {
		t.Fatalf("unexpected zero-capacity observation: %#v", observation)
	}
}

func mutateSessionDeviceObservationBody(
	t *testing.T, body string, mutate func(map[string]any),
) string {
	t.Helper()
	var value map[string]any
	if err := json.Unmarshal([]byte(body), &value); err != nil {
		t.Fatal(err)
	}
	mutate(value)
	encoded, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	return string(encoded)
}

func mutateSessionDeviceObservationDevices(
	t *testing.T, value map[string]any, mutate func(map[string]any),
) {
	t.Helper()
	placement := value["placement"].(map[string]any)
	placementDevices := placement["devices"].([]any)
	candidates := value["candidates"].([]any)
	if len(placementDevices) != len(candidates) {
		t.Fatalf("placement devices=%d candidates=%d", len(placementDevices), len(candidates))
	}
	for index := range candidates {
		mutate(placementDevices[index].(map[string]any))
		candidate := candidates[index].(map[string]any)
		mutate(candidate["device"].(map[string]any))
	}
}
