package appserver

import (
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

func TestDevicePlacementRegistryCandidateDefaultsClosedAndProductionRemainsUnregistered(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	source := &fixtureDeviceInventoryReadV2Source{}
	configs := []*devicePlacementRegistryCandidateConfig{
		nil,
		{Source: source, Now: func(context.Context) (int64, error) { return 200_000, nil }},
		{Enabled: true, Now: func(context.Context) (int64, error) { return 200_000, nil }},
		{Enabled: true, Source: source},
	}
	for index, config := range configs {
		t.Run("disabled-"+string(rune('a'+index)), func(t *testing.T) {
			request := httptest.NewRequest(http.MethodPost, devicePlacementRegistryCandidatePath, strings.NewReader(`{}`))
			response := httptest.NewRecorder()
			newDevicePlacementRegistryCandidateRoutes(config).ServeHTTP(response, request)
			if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
				t.Fatalf("disabled candidate status=%d body=%q", response.Code, response.Body.String())
			}
		})
	}
	if source.calls != 0 {
		t.Fatalf("disabled candidate invoked source %d times", source.calls)
	}

	production := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	body := registryPlacementRequirementsBody(t)
	response := requestConversationAPI(t, production, identity, http.MethodPost,
		devicePlacementRegistryCandidatePath, devicePlacementRegistryCandidateScope,
		"application/json", "", body)
	if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
		t.Fatalf("production candidate status=%d body=%q", response.Code, response.Body.String())
	}
}

func TestDevicePlacementRegistryCandidateUsesVerifiedOwnerClockAndV2Source(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	source := &fixtureDeviceInventoryReadV2Source{value: fixtureDeviceInventoryReadV2Value(owner)}
	clockCalls := 0
	handler := authenticator.Handler(newDevicePlacementRegistryCandidateRoutes(&devicePlacementRegistryCandidateConfig{
		Enabled: true,
		Source:  source,
		Now: func(context.Context) (int64, error) {
			clockCalls++
			return 300_000, nil
		},
	}))
	response := requestConversationAPI(t, handler, identity, http.MethodPost,
		devicePlacementRegistryCandidatePath, devicePlacementRegistryCandidateScope,
		"application/json", "", registryPlacementRequirementsBody(t))
	if response.Code != http.StatusOK {
		t.Fatalf("registry placement status=%d body=%q", response.Code, response.Body.String())
	}
	var result deviceplacement.PersistedInventoryPlacementV2Evaluation
	if err := json.Unmarshal(response.Body.Bytes(), &result); err != nil {
		t.Fatalf("decode registry placement: %v body=%q", err, response.Body.String())
	}
	if clockCalls != 1 || source.calls != 1 || source.owner != (model.Owner{
		Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate",
	}) {
		t.Fatalf("dependency calls=%d source=%#v", clockCalls, source)
	}
	if result.Owner != owner || result.EvaluatedAtMS != 300_000 ||
		result.SchemaVersion != deviceplacement.PersistedInventoryPlacementV2SchemaVersion ||
		result.EvaluationMode != deviceplacement.PersistedInventoryPlacementV2EvaluationMode ||
		result.SourceSchemaVersion != deviceplacement.SessionDeviceObservationInventoryV2SchemaVersion ||
		len(result.Decisions) != 1 || result.SelectedDeviceID != nil || result.SelectedInstanceID != nil ||
		result.Authority != (deviceplacement.PersistedInventoryPlacementBatchAuthority{}) || result.EligibleCandidateCount != 0 {
		t.Fatalf("registry placement result=%#v", result)
	}
	if !result.Decisions[0].OwnerDeclarationUnverified || !result.Decisions[0].DeviceAttributesUnverified {
		t.Fatalf("registry placement decision lost unverified markers=%#v", result.Decisions[0])
	}
	assertContractHeaders(t, response.Header(), response.Body.Len(), "")
}

func TestDevicePlacementRegistryCandidateStrictBoundaryAndOwnerIsolation(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	source := &fixtureDeviceInventoryReadV2Source{value: fixtureDeviceInventoryReadV2Value(owner)}
	clockCalls := 0
	handler := authenticator.Handler(newDevicePlacementRegistryCandidateRoutes(&devicePlacementRegistryCandidateConfig{
		Enabled: true,
		Source:  source,
		Now: func(context.Context) (int64, error) {
			clockCalls++
			return 300_000, nil
		},
	}))
	valid := registryPlacementRequirementsBody(t)
	for _, test := range []struct {
		name   string
		method string
		path   string
		scope  string
		body   string
		want   int
	}{
		{name: "unknown-root", method: http.MethodPost, path: devicePlacementRegistryCandidatePath, scope: devicePlacementRegistryCandidateScope, body: `{"requirements":{},"unexpected":true}`, want: http.StatusBadRequest},
		{name: "duplicate-root", method: http.MethodPost, path: devicePlacementRegistryCandidatePath, scope: devicePlacementRegistryCandidateScope, body: `{"requirements":{"os":"linux","architecture":"amd64","min_cpu_cores":1,"min_memory_bytes":1,"min_storage_bytes":1,"runtime":"oci","gpu":{"required":false,"min_memory_bytes":0,"runtime":""},"data_residency_zones":["us-west"],"minimum_trust_zone":"untrusted","sandbox_floor":"process","concurrency_slots":1},"requirements":{"os":"linux","architecture":"amd64","min_cpu_cores":1,"min_memory_bytes":1,"min_storage_bytes":1,"runtime":"oci","gpu":{"required":false,"min_memory_bytes":0,"runtime":""},"data_residency_zones":["us-west"],"minimum_trust_zone":"untrusted","sandbox_floor":"process","concurrency_slots":1}}`, want: http.StatusBadRequest},
		{name: "valid", method: http.MethodPost, path: devicePlacementRegistryCandidatePath, scope: devicePlacementRegistryCandidateScope, body: valid, want: http.StatusOK},
		{name: "unknown-requirement", method: http.MethodPost, path: devicePlacementRegistryCandidatePath, scope: devicePlacementRegistryCandidateScope, body: `{"requirements":{"os":"linux","architecture":"amd64","min_cpu_cores":1,"min_memory_bytes":1,"min_storage_bytes":1,"runtime":"oci","gpu":{"required":false,"min_memory_bytes":0,"runtime":""},"data_residency_zones":["us-west"],"minimum_trust_zone":"untrusted","sandbox_floor":"process","concurrency_slots":1,"extra":true}}`, want: http.StatusBadRequest},
		{name: "query", method: http.MethodPost, path: devicePlacementRegistryCandidatePath + "?limit=1", scope: devicePlacementRegistryCandidateScope, body: valid, want: http.StatusBadRequest},
		{name: "method", method: http.MethodGet, path: devicePlacementRegistryCandidatePath, scope: devicePlacementRegistryCandidateScope, want: http.StatusMethodNotAllowed},
		{name: "scope", method: http.MethodPost, path: devicePlacementRegistryCandidatePath, scope: "forge:conversations:read", body: valid, want: http.StatusForbidden},
	} {
		t.Run(test.name, func(t *testing.T) {
			response := requestConversationAPI(t, handler, identity, test.method, test.path,
				test.scope, "application/json", "", test.body)
			if response.Code != test.want {
				t.Fatalf("status=%d want=%d body=%q", response.Code, test.want, response.Body.String())
			}
		})
	}
	if clockCalls != 1 || source.calls != 1 {
		t.Fatalf("invalid boundary reached dependencies clock=%d source=%d", clockCalls, source.calls)
	}

	foreign := fixtureDeviceInventoryReadV2Value(owner)
	foreign.Owner.Subject = "account-foreign"
	foreign.Devices[0].Device.Owner.Subject = "account-foreign"
	foreignSource := &fixtureDeviceInventoryReadV2Source{value: foreign}
	foreignHandler := authenticator.Handler(newDevicePlacementRegistryCandidateRoutes(&devicePlacementRegistryCandidateConfig{
		Enabled: true, Source: foreignSource, Now: func(context.Context) (int64, error) { return 300_000, nil },
	}))
	foreignResponse := requestConversationAPI(t, foreignHandler, identity, http.MethodPost,
		devicePlacementRegistryCandidatePath, devicePlacementRegistryCandidateScope,
		"application/json", "", valid)
	if foreignResponse.Code != http.StatusBadGateway || foreignSource.calls != 1 ||
		!strings.Contains(foreignResponse.Body.String(), `"code":"device_placement_invalid"`) {
		t.Fatalf("foreign source status=%d calls=%d body=%q", foreignResponse.Code, foreignSource.calls, foreignResponse.Body.String())
	}

	foreignOwnerResponse := requestConversationAPIAs(t, handler, identity, http.MethodPost,
		devicePlacementRegistryCandidatePath, devicePlacementRegistryCandidateScope,
		"account-foreign", "tenant-slate", "application/json", "", valid)
	if foreignOwnerResponse.Code != http.StatusForbidden || source.calls != 1 {
		t.Fatalf("foreign bearer status=%d source calls=%d body=%q", foreignOwnerResponse.Code, source.calls, foreignOwnerResponse.Body.String())
	}
}

func TestLifecycleRegistryPlacementCandidateCompositionReadsPersistedV2Source(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	path := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{
		lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1),
	})
	handler := authenticator.Handler(newAuthenticatedSessionRoutesWithLifecycleRegistryInventoryCandidates(
		nil, nil, path, 200_000,
	))
	response := requestConversationAPI(t, handler, identity, http.MethodPost,
		devicePlacementRegistryCandidatePath, devicePlacementRegistryCandidateScope,
		"application/json", "", registryPlacementRequirementsBody(t))
	if response.Code != http.StatusOK {
		t.Fatalf("persisted registry placement status=%d body=%q", response.Code, response.Body.String())
	}
	var result deviceplacement.PersistedInventoryPlacementV2Evaluation
	if err := json.Unmarshal(response.Body.Bytes(), &result); err != nil {
		t.Fatal(err)
	}
	if result.Owner.Subject != owner.Subject || result.EvaluatedAtMS != 200_000 || len(result.Decisions) != 1 ||
		result.SelectedDeviceID != nil || result.SelectedInstanceID != nil ||
		result.Authority != (deviceplacement.PersistedInventoryPlacementBatchAuthority{}) {
		t.Fatalf("persisted registry placement result=%#v", result)
	}
}

func TestDevicePlacementRegistryCandidateClockAndSourceFailuresFailClosed(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	valid := registryPlacementRequirementsBody(t)
	clockError := errors.New("clock unavailable")
	clockFailure := authenticator.Handler(newDevicePlacementRegistryCandidateRoutes(&devicePlacementRegistryCandidateConfig{
		Enabled: true, Source: &fixtureDeviceInventoryReadV2Source{value: fixtureDeviceInventoryReadV2Value(owner)},
		Now: func(context.Context) (int64, error) { return 0, clockError },
	}))
	response := requestConversationAPI(t, clockFailure, identity, http.MethodPost,
		devicePlacementRegistryCandidatePath, devicePlacementRegistryCandidateScope,
		"application/json", "", valid)
	if response.Code != http.StatusBadGateway || !strings.Contains(response.Body.String(), `"code":"conversation_service_error"`) {
		t.Fatalf("clock failure status=%d body=%q", response.Code, response.Body.String())
	}

	sourceFailure := authenticator.Handler(newDevicePlacementRegistryCandidateRoutes(&devicePlacementRegistryCandidateConfig{
		Enabled: true, Source: &fixtureDeviceInventoryReadV2Source{err: errors.New("registry unavailable")},
		Now: func(context.Context) (int64, error) { return 200_000, nil },
	}))
	response = requestConversationAPI(t, sourceFailure, identity, http.MethodPost,
		devicePlacementRegistryCandidatePath, devicePlacementRegistryCandidateScope,
		"application/json", "", valid)
	if response.Code != http.StatusBadGateway || !strings.Contains(response.Body.String(), `"code":"conversation_service_error"`) {
		t.Fatalf("source failure status=%d body=%q", response.Code, response.Body.String())
	}
}

func registryPlacementRequirementsBody(t *testing.T) string {
	t.Helper()
	body, err := json.Marshal(devicePlacementRegistryCandidateRequest{Requirements: deviceplacement.Requirements{
		OS: "linux", Architecture: "amd64", MinCPUCores: 1, MinMemoryBytes: 1, MinStorageBytes: 1,
		Runtime: "oci", GPU: deviceplacement.GPURequirement{}, DataResidencyZones: []string{"us-west"},
		MinimumTrustZone: "untrusted", SandboxFloor: "process", ConcurrencySlots: 1,
	}})
	if err != nil {
		t.Fatal(err)
	}
	return string(body)
}
