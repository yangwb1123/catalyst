package appserver

import (
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

type fixtureDeviceInventoryReadSource struct {
	value deviceplacement.SessionDeviceObservationInventory
	err   error
	calls int
	owner model.Owner
}

func TestDeviceInventoryReadCandidateDefaultsClosedWithoutExplicitConfig(t *testing.T) {
	source := &fixtureDeviceInventoryReadSource{}
	cases := []struct {
		name   string
		config *deviceInventoryReadCandidateConfig
	}{
		{name: "missing config", config: nil},
		{name: "disabled config", config: &deviceInventoryReadCandidateConfig{Source: source}},
		{name: "enabled without source", config: &deviceInventoryReadCandidateConfig{Enabled: true}},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			request := httptest.NewRequest(http.MethodGet, deviceInventoryReadCandidatePath, nil)
			response := httptest.NewRecorder()
			newDeviceInventoryReadCandidateRoutes(test.config).ServeHTTP(response, request)
			if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
				t.Fatalf("default-disabled candidate response=%d body=%q", response.Code, response.Body.String())
			}
			assertContractHeaders(t, response.Header(), len(notFoundBody), "")
		})
	}
	if source.calls != 0 {
		t.Fatalf("default-disabled candidate invoked source %d times", source.calls)
	}
}

func (source *fixtureDeviceInventoryReadSource) ReadOwnedDeviceInventory(
	_ context.Context,
	owner model.Owner,
) (deviceplacement.SessionDeviceObservationInventory, error) {
	source.calls++
	source.owner = owner
	return source.value, source.err
}

func TestDeviceInventoryReadCandidateUsesVerifiedOwnerAndFixtureSource(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := model.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	source := &fixtureDeviceInventoryReadSource{
		value: fixtureDeviceInventoryReadValue(deviceplacement.Owner{
			Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate",
		}),
	}
	handler := authenticator.Handler(newDeviceInventoryReadCandidateRoutes(&deviceInventoryReadCandidateConfig{
		Enabled: true, Source: source,
	}))
	response := requestConversationAPI(t, handler, identity, http.MethodGet,
		deviceInventoryReadCandidatePath, deviceInventoryReadCandidateScope, "", "", "")
	if response.Code != http.StatusOK {
		t.Fatalf("candidate status=%d body=%q", response.Code, response.Body.String())
	}
	var got deviceplacement.SessionDeviceObservationInventory
	if err := json.Unmarshal(response.Body.Bytes(), &got); err != nil {
		t.Fatalf("decode candidate response: %v body=%q", err, response.Body.String())
	}
	if err := deviceplacement.ValidateSessionDeviceObservationInventory(got); err != nil {
		t.Fatalf("candidate response validation: %v", err)
	}
	if source.calls != 1 || source.owner != owner || got.Owner.Subject != owner.Subject ||
		len(got.Devices) != 1 || got.ExecutionAuthorized || got.ReservationCreated || got.DispatchPerformed {
		t.Fatalf("source=%#v response=%#v", source, got)
	}
	if !strings.Contains(response.Body.String(), `"inventory_declarations_unverified":true`) {
		t.Fatalf("candidate omitted unverified marker: %q", response.Body.String())
	}
}

func TestDeviceInventoryReadCandidateRejectsForeignSourceOwner(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	source := &fixtureDeviceInventoryReadSource{
		value: fixtureDeviceInventoryReadValue(deviceplacement.Owner{
			Issuer: identity.issuer, Subject: "account-foreign", TenantID: "tenant-slate",
		}),
	}
	handler := authenticator.Handler(newDeviceInventoryReadCandidateRoutes(&deviceInventoryReadCandidateConfig{
		Enabled: true, Source: source,
	}))
	response := requestConversationAPI(t, handler, identity, http.MethodGet,
		deviceInventoryReadCandidatePath, deviceInventoryReadCandidateScope, "", "", "")
	if response.Code != http.StatusBadGateway || source.calls != 1 {
		t.Fatalf("foreign source status=%d calls=%d body=%q", response.Code, source.calls, response.Body.String())
	}
	if !strings.Contains(response.Body.String(), `"code":"device_inventory_invalid"`) {
		t.Fatalf("foreign source error=%q", response.Body.String())
	}
}

func TestDeviceInventoryReadCandidateRejectsUnsafeSourceNumbers(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	value := fixtureDeviceInventoryReadValue(owner)
	value.Devices[0].Device.LeaseExpiresAtMS = deviceplacement.MaxSafeIntegerMS + 1
	source := &fixtureDeviceInventoryReadSource{value: value}
	handler := authenticator.Handler(newDeviceInventoryReadCandidateRoutes(&deviceInventoryReadCandidateConfig{
		Enabled: true, Source: source,
	}))
	response := requestConversationAPI(t, handler, identity, http.MethodGet,
		deviceInventoryReadCandidatePath, deviceInventoryReadCandidateScope, "", "", "")
	if response.Code != http.StatusBadGateway || source.calls != 1 {
		t.Fatalf("unsafe source status=%d calls=%d body=%q", response.Code, source.calls, response.Body.String())
	}
}

func TestDeviceInventoryReadCandidateRejectsQueryBodyAndScope(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	source := &fixtureDeviceInventoryReadSource{value: fixtureDeviceInventoryReadValue(deviceplacement.Owner{
		Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate",
	})}
	handler := authenticator.Handler(newDeviceInventoryReadCandidateRoutes(&deviceInventoryReadCandidateConfig{
		Enabled: true, Source: source,
	}))
	query := requestConversationAPI(t, handler, identity, http.MethodGet,
		deviceInventoryReadCandidatePath+"?limit=1", deviceInventoryReadCandidateScope, "", "", "")
	if query.Code != http.StatusBadRequest || source.calls != 0 {
		t.Fatalf("query status=%d calls=%d body=%q", query.Code, source.calls, query.Body.String())
	}
	method := requestConversationAPI(t, handler, identity, http.MethodPost,
		deviceInventoryReadCandidatePath, deviceInventoryReadCandidateScope, "application/json", "", `{}`)
	if method.Code != http.StatusMethodNotAllowed || source.calls != 0 {
		t.Fatalf("method status=%d calls=%d body=%q", method.Code, source.calls, method.Body.String())
	}
	noScope := requestConversationAPI(t, handler, identity, http.MethodGet,
		deviceInventoryReadCandidatePath, "forge:conversations:read", "", "", "")
	if noScope.Code != http.StatusForbidden || source.calls != 0 {
		t.Fatalf("scope status=%d calls=%d body=%q", noScope.Code, source.calls, noScope.Body.String())
	}
}

func TestDeviceInventoryReadCandidateSourceFailureFailsClosed(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	source := &fixtureDeviceInventoryReadSource{err: errors.New("fixture unavailable")}
	handler := authenticator.Handler(newDeviceInventoryReadCandidateRoutes(&deviceInventoryReadCandidateConfig{
		Enabled: true, Source: source,
	}))
	response := requestConversationAPI(t, handler, identity, http.MethodGet,
		deviceInventoryReadCandidatePath, deviceInventoryReadCandidateScope, "", "", "")
	if response.Code != http.StatusBadGateway || !strings.Contains(response.Body.String(), `"code":"conversation_service_error"`) {
		t.Fatalf("source failure status=%d body=%q", response.Code, response.Body.String())
	}
}

func TestDeviceInventoryReadCandidateResponseBudgetFailsClosed(t *testing.T) {
	request := httptest.NewRequest(http.MethodGet, deviceInventoryReadCandidatePath, nil)
	response := httptest.NewRecorder()
	writeDeviceInventoryReadCandidateJSON(response, request, http.StatusOK, struct {
		Payload string `json:"payload"`
	}{Payload: strings.Repeat("x", deviceplacement.MaxRequestBytes)})
	if response.Code != http.StatusBadGateway || !strings.Contains(response.Body.String(), `"code":"device_inventory_invalid"`) {
		t.Fatalf("oversized response status=%d body=%q", response.Code, response.Body.String())
	}
}

func fixtureDeviceInventoryReadValue(owner deviceplacement.Owner) deviceplacement.SessionDeviceObservationInventory {
	return deviceplacement.SessionDeviceObservationInventory{
		SchemaVersion:              "forge.device-inventory-observation/v1",
		EvaluationMode:             deviceplacement.EvaluationMode,
		EvaluatedAtMS:              200000,
		Owner:                      owner,
		OwnerDeclarationUnverified: true,
		InventoryUnverified:        true,
		Notice:                     deviceplacement.SessionDeviceObservationInventoryNotice,
		Devices: []deviceplacement.SessionPlacementCandidate{{
			InstanceID: "runner-a",
			Device: deviceplacement.Device{
				DeviceID: "device-a", Owner: owner, ApprovalState: "approved", CordonState: "clear", Liveness: "online",
				SnapshotObservedAtMS: 150000, LeaseExpiresAtMS: 210000,
				OS: "linux", Architecture: "amd64", AvailableCPUCores: 8, AvailableMemoryBytes: 16384,
				AvailableStorage: 8192, Runtimes: []string{"oci"},
				GPU: deviceplacement.GPUDeclaration{}, DataResidencyZones: []string{"us-west"}, TrustZone: "standard",
				SandboxLevels: []string{"container"}, ConcurrencyLimit: 4, ActiveConcurrency: 1,
			},
		}},
		ExecutionAuthorized: false,
		ReservationCreated:  false,
		DispatchPerformed:   false,
	}
}

// fixtureDeviceInventoryReadValueMultiple extends the single-row fixture used
// by focused route tests with a second distinct Runner/device pair. Both rows
// intentionally carry the same owner declaration so the authenticated E2E
// proves owner binding across a bounded multi-Runner page.
func fixtureDeviceInventoryReadValueMultiple(owner deviceplacement.Owner) deviceplacement.SessionDeviceObservationInventory {
	value := fixtureDeviceInventoryReadValue(owner)
	value.Devices = append(value.Devices, deviceplacement.SessionPlacementCandidate{
		InstanceID: "runner-b",
		Device: deviceplacement.Device{
			DeviceID: "device-b", Owner: owner, ApprovalState: "approved", CordonState: "clear", Liveness: "online",
			SnapshotObservedAtMS: 150000, LeaseExpiresAtMS: 210000,
			OS: "linux", Architecture: "arm64", AvailableCPUCores: 4, AvailableMemoryBytes: 8192,
			AvailableStorage: 4096, Runtimes: []string{"oci"},
			GPU: deviceplacement.GPUDeclaration{}, DataResidencyZones: []string{"us-west"}, TrustZone: "standard",
			SandboxLevels: []string{"container"}, ConcurrencyLimit: 2, ActiveConcurrency: 0,
		},
	})
	return value
}
