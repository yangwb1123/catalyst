package appserver

import (
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"forgeos/forge-core/internal/deviceplacement"
)

func TestDevicePlacementPreviewBindsOwnerAndRemainsStateless(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	sessions := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}
	body := devicePlacementPreviewBody(identity.issuer, "account-42", "tenant-slate")
	response := requestConversationAPI(t, routes, identity, http.MethodPost, devicePlacementPreviewPath,
		"forge:conversations:read", "application/json", "", body)
	if response.Code != http.StatusOK {
		t.Fatalf("placement preview status=%d body=%q", response.Code, response.Body.String())
	}
	var result deviceplacement.Result
	if err := json.Unmarshal(response.Body.Bytes(), &result); err != nil {
		t.Fatalf("decode placement preview: %v body=%q", err, response.Body.String())
	}
	if result.SchemaVersion != deviceplacement.ResultSchemaVersion ||
		result.EvaluationMode != deviceplacement.EvaluationMode ||
		!result.OwnerDeclarationUnverified || !result.DeviceAttributesUnverified ||
		result.ExecutionAuthorized || result.ReservationCreated || result.DispatchPerformed ||
		len(result.DeviceResults) != 1 || !result.DeviceResults[0].MatchesRequirements ||
		result.DeviceResults[0].DeviceID != "device-1" {
		t.Fatalf("placement preview result=%#v", result)
	}
	assertContractHeaders(t, response.Header(), response.Body.Len(), "")

	foreign := requestConversationAPIAs(t, routes, identity, http.MethodPost, devicePlacementPreviewPath,
		"forge:conversations:read", "account-foreign", "tenant-slate", "application/json", "", body)
	if foreign.Code != http.StatusForbidden || foreign.Body.String() == response.Body.String() {
		t.Fatalf("foreign owner preview status=%d body=%q", foreign.Code, foreign.Body.String())
	}

	invalid := requestConversationAPI(t, routes, identity, http.MethodPost, devicePlacementPreviewPath,
		"forge:conversations:read", "application/json", "", `{"schema_version":"wrong"}`)
	if invalid.Code != http.StatusBadRequest {
		t.Fatalf("invalid placement preview status=%d body=%q", invalid.Code, invalid.Body.String())
	}
}

func TestDevicePlacementPreviewRequiresReadScopeAndHasNoDeviceRoutes(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	sessions := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}
	body := devicePlacementPreviewBody(identity.issuer, "account-42", "tenant-slate")
	noScope := requestConversationAPI(t, routes, identity, http.MethodPost, devicePlacementPreviewPath,
		"forge:conversations:write", "application/json", "", body)
	if noScope.Code != http.StatusForbidden {
		t.Fatalf("placement preview without read scope status=%d body=%q", noScope.Code, noScope.Body.String())
	}
	method := requestConversationAPI(t, routes, identity, http.MethodGet, devicePlacementPreviewPath,
		"forge:conversations:read", "", "", "")
	if method.Code != http.StatusMethodNotAllowed || method.Header().Get("Allow") != http.MethodPost {
		t.Fatalf("placement preview method status=%d allow=%q body=%q", method.Code, method.Header().Get("Allow"), method.Body.String())
	}
	query := requestConversationAPI(t, routes, identity, http.MethodPost, devicePlacementPreviewPath+"?limit=1",
		"forge:conversations:read", "application/json", "", body)
	if query.Code != http.StatusBadRequest {
		t.Fatalf("placement preview query status=%d body=%q", query.Code, query.Body.String())
	}
	for _, target := range []string{"/api/v1/devices?limit=25", "/api/v1/devices/device-1/heartbeats"} {
		response := requestConversationAPI(t, routes, identity, http.MethodGet, target,
			"forge:conversations:read", "", "", "")
		if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
			t.Fatalf("device route %s status=%d body=%q", target, response.Code, response.Body.String())
		}
	}
}

func TestDevicePlacementPreviewUsesStrictAuthenticatedJSONBoundary(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	sessions := authenticator.Handler(newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil))
	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions,
	)
	if err != nil {
		t.Fatal(err)
	}
	body := devicePlacementPreviewBody(identity.issuer, "account-42", "tenant-slate")
	tests := []struct {
		name        string
		contentType string
		body        string
		status      int
	}{
		{
			name:        "missing bearer",
			contentType: "application/json",
			body:        body,
			status:      http.StatusUnauthorized,
		},
		{
			name:        "unsupported content type",
			contentType: "text/plain",
			body:        body,
			status:      http.StatusUnsupportedMediaType,
		},
		{
			name:        "unknown field",
			contentType: "application/json",
			body:        strings.Replace(body, `"devices":[`, `"unexpected":true,"devices":[`, 1),
			status:      http.StatusBadRequest,
		},
		{
			name:        "duplicate field",
			contentType: "application/json",
			body:        strings.Replace(body, `"schema_version":"forge.device-placement-dry-run/v1"`, `"schema_version":"forge.device-placement-dry-run/v1","schema_version":"forge.device-placement-dry-run/v1"`, 1),
			status:      http.StatusBadRequest,
		},
		{
			name:        "null field",
			contentType: "application/json",
			body:        strings.Replace(body, `"evaluated_at_ms":1800000000000`, `"evaluated_at_ms":null`, 1),
			status:      http.StatusBadRequest,
		},
		{
			name:        "trailing value",
			contentType: "application/json",
			body:        body + ` {}`,
			status:      http.StatusBadRequest,
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			var response *httptest.ResponseRecorder
			if test.name == "missing bearer" {
				response = requestConversationAPIToken(t, routes, "", http.MethodPost,
					devicePlacementPreviewPath, test.contentType, "", test.body)
			} else {
				response = requestConversationAPI(t, routes, identity, http.MethodPost,
					devicePlacementPreviewPath, "forge:conversations:read", test.contentType, "", test.body)
			}
			if response.Code != test.status {
				t.Fatalf("placement preview status=%d body=%q, want %d", response.Code, response.Body.String(), test.status)
			}
		})
	}
}

func devicePlacementPreviewBody(issuer, subject, tenant string) string {
	owner := fmt.Sprintf(`{"issuer":%q,"subject":%q,"tenant_id":%q}`, issuer, subject, tenant)
	return fmt.Sprintf(`{"schema_version":%q,"evaluated_at_ms":1800000000000,"owner":%s,"max_snapshot_age_ms":60000,"requirements":{"os":"linux","architecture":"amd64","min_cpu_cores":4,"min_memory_bytes":8589934592,"min_storage_bytes":21474836480,"runtime":"oci","gpu":{"required":false,"min_memory_bytes":0,"runtime":""},"data_residency_zones":["us-west"],"minimum_trust_zone":"standard","sandbox_floor":"container","concurrency_slots":1},"devices":[{"device_id":"device-1","owner":%s,"approval_state":"approved","cordon_state":"clear","liveness":"online","snapshot_observed_at_ms":1799999999000,"lease_expires_at_ms":1800000060000,"os":"linux","architecture":"amd64","available_cpu_cores":8,"available_memory_bytes":17179869184,"available_storage_bytes":107374182400,"runtimes":["oci"],"gpu":{"present":false,"memory_bytes":0,"runtime":""},"data_residency_zones":["us-west"],"trust_zone":"standard","sandbox_levels":["container"],"concurrency_limit":4,"active_concurrency":1}]}`, deviceplacement.RequestSchemaVersion, owner, owner)
}
