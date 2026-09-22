package appserver

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

func TestSchedulerSelectionPreviewDefaultsClosedWithoutDependencies(t *testing.T) {
	source := &fixtureDeviceInventoryReadV2Source{}
	for _, config := range []*schedulerSelectionPreviewConfig{
		nil,
		{Source: source, Now: func(context.Context) (int64, error) { return 300_000, nil }},
		{Enabled: true, Now: func(context.Context) (int64, error) { return 300_000, nil }},
		{Enabled: true, Source: source},
	} {
		response := httptest.NewRecorder()
		newSchedulerSelectionPreviewRoutes(config).ServeHTTP(response,
			httptest.NewRequest(http.MethodPost, schedulerSelectionPreviewPath, nil))
		if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
			t.Fatalf("disabled scheduler preview status=%d body=%q", response.Code, response.Body.String())
		}
	}
	if source.calls != 0 {
		t.Fatalf("disabled scheduler preview invoked source %d times", source.calls)
	}
}

func TestSchedulerSelectionPreviewUsesVerifiedOwnerAndReturnsNoAuthority(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	source := &fixtureDeviceInventoryReadV2Source{value: fixtureDeviceInventoryReadV2Value(owner)}
	clockCalls := 0
	handler := authenticator.Handler(newSchedulerSelectionPreviewRoutes(&schedulerSelectionPreviewConfig{
		Enabled: true,
		Source:  source,
		Now: func(context.Context) (int64, error) {
			clockCalls++
			return 300_000, nil
		},
	}))
	body, err := json.Marshal(schedulerSelectionPreviewRequest{
		ConversationID: "conversation-1", RunID: "run-1", AttemptID: "attempt-1",
		Requirements: deviceplacement.Requirements{
			OS: "linux", Architecture: "amd64", MinCPUCores: 1, MinMemoryBytes: 1, MinStorageBytes: 1,
			Runtime: "go", GPU: deviceplacement.GPURequirement{}, DataResidencyZones: []string{"us-west"},
			MinimumTrustZone: "untrusted", SandboxFloor: "process", ConcurrencySlots: 1,
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	response := requestConversationAPI(t, handler, identity, http.MethodPost,
		schedulerSelectionPreviewPath, schedulerSelectionPreviewScope, "application/json", "", string(body))
	if response.Code != http.StatusOK {
		t.Fatalf("scheduler preview status=%d body=%q", response.Code, response.Body.String())
	}
	var observation deviceplacement.SchedulerSelectionPreviewObservation
	if err := json.Unmarshal(response.Body.Bytes(), &observation); err != nil {
		t.Fatalf("decode scheduler preview: %v body=%q", err, response.Body.String())
	}
	if err := observation.Validate(); err != nil {
		t.Fatalf("validate scheduler preview: %v", err)
	}
	if clockCalls != 1 || source.calls != 1 || source.owner != (model.Owner{
		Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate",
	}) {
		t.Fatalf("dependency calls=%d source=%#v", clockCalls, source)
	}
	if observation.SelectionAvailable || observation.SelectedDeviceID != nil || observation.SelectedInstanceID != nil ||
		observation.SelectionReason != "no_eligible_candidate" || observation.Authority != (deviceplacement.SchedulerSelectionPreviewAuthority{}) {
		t.Fatalf("scheduler preview observation=%#v", observation)
	}
	assertContractHeaders(t, response.Header(), response.Body.Len(), "")
}

func TestSchedulerSelectionPreviewStrictBoundaryAndOwnerIsolation(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	source := &fixtureDeviceInventoryReadV2Source{value: fixtureDeviceInventoryReadV2Value(owner)}
	handler := authenticator.Handler(newSchedulerSelectionPreviewRoutes(&schedulerSelectionPreviewConfig{
		Enabled: true, Source: source, Now: func(context.Context) (int64, error) { return 300_000, nil },
	}))
	valid := `{"conversation_id":"conversation-1","run_id":"run-1","attempt_id":"attempt-1","requirements":{"os":"linux","architecture":"amd64","min_cpu_cores":1,"min_memory_bytes":1,"min_storage_bytes":1,"runtime":"go","gpu":{"required":false,"min_memory_bytes":0,"runtime":""},"data_residency_zones":["us-west"],"minimum_trust_zone":"untrusted","sandbox_floor":"process","concurrency_slots":1}}`
	for _, test := range []struct {
		name   string
		method string
		path   string
		scope  string
		body   string
		want   int
	}{
		{name: "unknown field", method: http.MethodPost, path: schedulerSelectionPreviewPath, scope: schedulerSelectionPreviewScope, body: `{"unexpected":true}`, want: http.StatusBadRequest},
		{name: "query", method: http.MethodPost, path: schedulerSelectionPreviewPath + "?limit=1", scope: schedulerSelectionPreviewScope, body: valid, want: http.StatusBadRequest},
		{name: "method", method: http.MethodGet, path: schedulerSelectionPreviewPath, scope: schedulerSelectionPreviewScope, want: http.StatusMethodNotAllowed},
		{name: "scope", method: http.MethodPost, path: schedulerSelectionPreviewPath, scope: "forge:conversations:read", body: valid, want: http.StatusForbidden},
	} {
		t.Run(test.name, func(t *testing.T) {
			response := requestConversationAPI(t, handler, identity, test.method, test.path,
				test.scope, "application/json", "", test.body)
			if response.Code != test.want {
				t.Fatalf("scheduler preview status=%d want=%d body=%q", response.Code, test.want, response.Body.String())
			}
		})
	}
	if source.calls != 0 {
		t.Fatalf("invalid scheduler preview boundary reached source %d times", source.calls)
	}
	foreign := requestConversationAPIAs(t, handler, identity, http.MethodPost,
		schedulerSelectionPreviewPath, schedulerSelectionPreviewScope,
		"account-foreign", "tenant-slate", "application/json", "", valid)
	if foreign.Code != http.StatusForbidden || source.calls != 0 {
		t.Fatalf("foreign scheduler preview status=%d source calls=%d body=%q", foreign.Code, source.calls, foreign.Body.String())
	}
}
