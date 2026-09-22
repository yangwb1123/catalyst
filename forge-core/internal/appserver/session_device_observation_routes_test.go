package appserver

import (
	"bytes"
	"encoding/json"
	"net/http"
	"testing"

	"forgeos/forge-core/internal/deviceplacement"
)

func TestSessionDeviceObservationPreviewAggregatesNineInstances(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	sessions := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}

	conversationID, runID := "conversation-001", "run-001"
	body := multiInstanceSessionDeviceObservationPreviewBody(
		t, identity.issuer, "account-42", "tenant-slate", conversationID, runID,
	)
	path := "/api/v1/conversations/" + conversationID + "/runs/" + runID + "/device-observation/preview"
	wantMatches := []bool{true, false, false, false, false, false, false, false, true}
	var firstBody []byte
	for attempt := 0; attempt < 2; attempt++ {
		response := requestConversationAPI(t, routes, identity, http.MethodPost, path,
			"forge:conversations:read", "application/json", "", body)
		if response.Code != http.StatusOK {
			t.Fatalf("device observation attempt %d status=%d body=%q", attempt+1, response.Code, response.Body.String())
		}
		if attempt == 0 {
			firstBody = append([]byte(nil), response.Body.Bytes()...)
		} else if !bytes.Equal(firstBody, response.Body.Bytes()) {
			t.Fatalf("device observation output is not deterministic:\nfirst=%s\nsecond=%s", firstBody, response.Body.Bytes())
		}

		var value deviceplacement.SessionDeviceObservation
		if err := json.Unmarshal(response.Body.Bytes(), &value); err != nil {
			t.Fatalf("decode device observation: %v body=%q", err, response.Body.String())
		}
		if err := deviceplacement.ValidateSessionDeviceObservation(value); err != nil {
			t.Fatalf("validate device observation: %v body=%q", err, response.Body.String())
		}
		wantOwner := deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
		if value.SchemaVersion != deviceplacement.SessionDeviceObservationSchemaVersion ||
			value.EvaluationMode != deviceplacement.EvaluationMode || value.Owner != wantOwner ||
			value.ConversationID != conversationID || value.RunID != runID || value.EvaluatedAtMS != 200000 ||
			!value.OwnerDeclarationUnverified || len(value.Inventory.Devices) != 9 ||
			value.ResourceSummary.DeviceCount != 9 || value.ResourceSummary.RunnerInstanceCount != 9 ||
			value.ResourceSummary.AvailableCPUCores != 66 || value.ResourceSummary.AvailableMemoryBytes != 135168 ||
			value.ResourceSummary.AvailableStorageBytes != 67584 || value.ResourceSummary.AvailableGPUCount != 0 ||
			value.ResourceSummary.AvailableGPUMemoryBytes != 0 || value.ResourceSummary.EligibleDeviceCount != 2 ||
			value.ResourceSummary.EligibleInstanceCount != 2 || value.SelectedDeviceID != nil ||
			value.SelectedInstanceID != nil || value.Authority != (deviceplacement.SessionPlacementAuthority{}) {
			t.Fatalf("unexpected nine-instance observation: %#v", value)
		}
		if len(value.PlacementObservation.Decisions) != len(wantMatches) {
			t.Fatalf("decisions=%d, want %d", len(value.PlacementObservation.Decisions), len(wantMatches))
		}
		for index, got := range value.PlacementObservation.Decisions {
			wantDeviceID := "candidate-" + string(rune('a'+index))
			wantInstanceID := "runner-" + string(rune('a'+index))
			if got.DeviceID != wantDeviceID || got.InstanceID != wantInstanceID ||
				got.MatchesRequirements != wantMatches[index] {
				t.Fatalf("decision[%d]=%#v, want device=%q instance=%q matches=%t", index, got, wantDeviceID, wantInstanceID, wantMatches[index])
			}
			if got.MatchesRequirements && len(got.ExclusionReasons) != 0 {
				t.Fatalf("eligible decision[%d] exclusions=%#v", index, got.ExclusionReasons)
			}
			if !got.MatchesRequirements && len(got.ExclusionReasons) == 0 {
				t.Fatalf("ineligible decision[%d] has no exclusion reason", index)
			}
		}
		if value.Inventory.ExecutionAuthorized || value.Inventory.ReservationCreated || value.Inventory.DispatchPerformed ||
			value.PlacementObservation.Authority != (deviceplacement.SessionPlacementAuthority{}) ||
			value.ResourceSummary.Authority != (deviceplacement.SessionPlacementAuthority{}) {
			t.Fatalf("nine-instance observation exposed authority: %#v", value)
		}
	}
}

func TestSessionDeviceObservationPreviewBindsOwnerAndRun(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	sessions := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}
	body := sessionDeviceObservationPreviewBody(t, identity.issuer, "account-42", "tenant-slate", "conversation-1", "run-1")
	path := "/api/v1/conversations/conversation-1/runs/run-1/device-observation/preview"
	response := requestConversationAPI(t, routes, identity, http.MethodPost, path,
		"forge:conversations:read", "application/json", "", body)
	if response.Code != http.StatusOK {
		t.Fatalf("device observation status=%d body=%q", response.Code, response.Body.String())
	}
	var value deviceplacement.SessionDeviceObservation
	if err := json.Unmarshal(response.Body.Bytes(), &value); err != nil {
		t.Fatalf("decode device observation: %v body=%q", err, response.Body.String())
	}
	if err := deviceplacement.ValidateSessionDeviceObservation(value); err != nil ||
		value.SchemaVersion != deviceplacement.SessionDeviceObservationSchemaVersion ||
		value.ConversationID != "conversation-1" || value.RunID != "run-1" ||
		len(value.Inventory.Devices) != 1 || value.ResourceSummary.DeviceCount != 1 ||
		value.Authority.ExecutionAuthorized {
		t.Fatalf("device observation=%#v validation=%v", value, err)
	}

	foreign := requestConversationAPIAs(t, routes, identity, http.MethodPost, path,
		"forge:conversations:read", "account-foreign", "tenant-slate", "application/json", "", body)
	if foreign.Code != http.StatusForbidden {
		t.Fatalf("foreign owner status=%d body=%q", foreign.Code, foreign.Body.String())
	}
	wrongPath := requestConversationAPI(t, routes, identity, http.MethodPost,
		"/api/v1/conversations/other/runs/run-1/device-observation/preview",
		"forge:conversations:read", "application/json", "", body)
	if wrongPath.Code != http.StatusBadRequest {
		t.Fatalf("foreign path status=%d body=%q", wrongPath.Code, wrongPath.Body.String())
	}
}

func TestSessionDeviceObservationPreviewRejectsIncompleteNestedPlacement(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	sessions := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}
	body := sessionDeviceObservationPreviewBody(t, identity.issuer, "account-42", "tenant-slate", "conversation-1", "run-1")
	var value map[string]any
	if err := json.Unmarshal([]byte(body), &value); err != nil {
		t.Fatal(err)
	}
	delete(value["placement"].(map[string]any)["requirements"].(map[string]any), "gpu")
	encoded, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	response := requestConversationAPI(t, routes, identity, http.MethodPost,
		"/api/v1/conversations/conversation-1/runs/run-1/device-observation/preview",
		"forge:conversations:read", "application/json", "", string(encoded))
	if response.Code != http.StatusBadRequest {
		t.Fatalf("incomplete nested placement status=%d body=%q", response.Code, response.Body.String())
	}
}

func TestSessionDeviceObservationPreviewKeepsExistingDeviceBoundary(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	sessions := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}
	body := sessionDeviceObservationPreviewBody(t, identity.issuer, "account-42", "tenant-slate", "conversation-1", "run-1")
	path := "/api/v1/conversations/conversation-1/runs/run-1/device-observation/preview"
	noScope := requestConversationAPI(t, routes, identity, http.MethodPost, path,
		"forge:conversations:write", "application/json", "", body)
	if noScope.Code != http.StatusForbidden {
		t.Fatalf("missing read scope status=%d body=%q", noScope.Code, noScope.Body.String())
	}
	method := requestConversationAPI(t, routes, identity, http.MethodGet, path,
		"forge:conversations:read", "", "", "")
	if method.Code != http.StatusMethodNotAllowed || method.Header().Get("Allow") != http.MethodPost {
		t.Fatalf("method status=%d allow=%q body=%q", method.Code, method.Header().Get("Allow"), method.Body.String())
	}
	query := requestConversationAPI(t, routes, identity, http.MethodPost, path+"?limit=1",
		"forge:conversations:read", "application/json", "", body)
	if query.Code != http.StatusBadRequest {
		t.Fatalf("query status=%d body=%q", query.Code, query.Body.String())
	}
	deviceRoute := requestConversationAPI(t, routes, identity, http.MethodGet, "/api/v1/devices",
		"forge:conversations:read", "", "", "")
	if deviceRoute.Code != http.StatusNotFound {
		t.Fatalf("device route status=%d body=%q", deviceRoute.Code, deviceRoute.Body.String())
	}
}

func sessionDeviceObservationPreviewBody(
	t *testing.T,
	issuer, subject, tenant, conversationID, runID string,
) string {
	t.Helper()
	var placement map[string]any
	if err := json.Unmarshal([]byte(devicePlacementPreviewBody(issuer, subject, tenant)), &placement); err != nil {
		t.Fatal(err)
	}
	owner := placement["owner"]
	devices := placement["devices"]
	request := map[string]any{
		"owner":           owner,
		"conversation_id": conversationID,
		"run_id":          runID,
		"placement": map[string]any{
			"schema_version":      deviceplacement.RequestSchemaVersion,
			"evaluated_at_ms":     placement["evaluated_at_ms"],
			"owner":               owner,
			"max_snapshot_age_ms": placement["max_snapshot_age_ms"],
			"requirements":        placement["requirements"],
			"devices":             devices,
		},
		"candidates": []any{map[string]any{
			"instance_id": "runner-1",
			"device":      placement["devices"].([]any)[0],
		}},
	}
	encoded, err := json.Marshal(request)
	if err != nil {
		t.Fatal(err)
	}
	return string(encoded)
}
