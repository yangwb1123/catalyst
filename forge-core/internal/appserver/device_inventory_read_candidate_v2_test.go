package appserver

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

type fixtureDeviceInventoryReadV2Source struct {
	value deviceplacement.SessionDeviceObservationInventoryV2
	err   error
	calls int
	owner model.Owner
}

func (source *fixtureDeviceInventoryReadV2Source) ReadOwnedDeviceInventoryV2(
	_ context.Context,
	owner model.Owner,
) (deviceplacement.SessionDeviceObservationInventoryV2, error) {
	source.calls++
	source.owner = owner
	return source.value, source.err
}

func TestDeviceInventoryReadCandidateV2DefaultsClosedWithoutExplicitConfig(t *testing.T) {
	source := &fixtureDeviceInventoryReadV2Source{}
	for _, test := range []struct {
		name   string
		config *deviceInventoryReadCandidateV2Config
	}{
		{name: "missing config"},
		{name: "disabled config", config: &deviceInventoryReadCandidateV2Config{Source: source}},
		{name: "enabled without source", config: &deviceInventoryReadCandidateV2Config{Enabled: true}},
	} {
		t.Run(test.name, func(t *testing.T) {
			request := httptest.NewRequest(http.MethodGet, deviceInventoryReadCandidateV2Path, nil)
			response := httptest.NewRecorder()
			newDeviceInventoryReadCandidateV2Routes(test.config).ServeHTTP(response, request)
			if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
				t.Fatalf("default-disabled v2 candidate response=%d body=%q", response.Code, response.Body.String())
			}
			assertContractHeaders(t, response.Header(), len(notFoundBody), "")
		})
	}
	if source.calls != 0 {
		t.Fatalf("default-disabled v2 candidate invoked source %d times", source.calls)
	}
}

func TestDeviceInventoryReadCandidateV2UsesVerifiedOwnerAndRetainsLosslessValues(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	value := fixtureDeviceInventoryReadV2Value(owner)
	source := &fixtureDeviceInventoryReadV2Source{value: value}
	handler := authenticator.Handler(newDeviceInventoryReadCandidateV2Routes(&deviceInventoryReadCandidateV2Config{
		Enabled: true, Source: source,
	}))
	response := requestConversationAPI(t, handler, identity, http.MethodGet,
		deviceInventoryReadCandidateV2Path, deviceInventoryReadCandidateScope, "", "", "")
	if response.Code != http.StatusOK {
		t.Fatalf("v2 candidate status=%d body=%q", response.Code, response.Body.String())
	}
	var got deviceplacement.SessionDeviceObservationInventoryV2
	if err := json.Unmarshal(response.Body.Bytes(), &got); err != nil {
		t.Fatalf("decode v2 candidate response: %v body=%q", err, response.Body.String())
	}
	if err := deviceplacement.ValidateSessionDeviceObservationInventoryV2(got); err != nil {
		t.Fatalf("v2 candidate response validation: %v", err)
	}
	if source.calls != 1 || source.owner != (model.Owner{
		Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate",
	}) || got.Owner != owner || len(got.Devices) != 1 ||
		got.Devices[0].Device.ReservationState != "reserved" ||
		len(got.Devices[0].Device.GPUs) != 2 || got.ExecutionAuthorized ||
		got.ReservationCreated || got.DispatchPerformed {
		t.Fatalf("source=%#v response=%#v", source, got)
	}
	if !strings.Contains(response.Body.String(), `"inventory_declarations_unverified":true`) {
		t.Fatalf("v2 candidate omitted unverified marker: %q", response.Body.String())
	}
}

func TestDeviceInventoryReadCandidateV2RejectsForeignSourceAndInvalidRequest(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	foreign := fixtureDeviceInventoryReadV2Value(owner)
	foreign.Owner.Subject = "account-foreign"
	foreign.Devices[0].Device.Owner.Subject = "account-foreign"
	source := &fixtureDeviceInventoryReadV2Source{value: foreign}
	handler := authenticator.Handler(newDeviceInventoryReadCandidateV2Routes(&deviceInventoryReadCandidateV2Config{
		Enabled: true, Source: source,
	}))
	response := requestConversationAPI(t, handler, identity, http.MethodGet,
		deviceInventoryReadCandidateV2Path, deviceInventoryReadCandidateScope, "", "", "")
	if response.Code != http.StatusBadGateway || source.calls != 1 ||
		!strings.Contains(response.Body.String(), `"code":"device_inventory_invalid"`) {
		t.Fatalf("foreign v2 source status=%d calls=%d body=%q", response.Code, source.calls, response.Body.String())
	}
	query := requestConversationAPI(t, handler, identity, http.MethodGet,
		deviceInventoryReadCandidateV2Path+"?limit=1", deviceInventoryReadCandidateScope, "", "", "")
	if query.Code != http.StatusBadRequest || source.calls != 1 {
		t.Fatalf("v2 query status=%d calls=%d body=%q", query.Code, source.calls, query.Body.String())
	}
	method := requestConversationAPI(t, handler, identity, http.MethodPost,
		deviceInventoryReadCandidateV2Path, deviceInventoryReadCandidateScope, "application/json", "", `{}`)
	if method.Code != http.StatusMethodNotAllowed || source.calls != 1 {
		t.Fatalf("v2 method status=%d calls=%d body=%q", method.Code, source.calls, method.Body.String())
	}
	noScope := requestConversationAPI(t, handler, identity, http.MethodGet,
		deviceInventoryReadCandidateV2Path, "forge:conversations:read", "", "", "")
	if noScope.Code != http.StatusForbidden || source.calls != 1 {
		t.Fatalf("v2 scope status=%d calls=%d body=%q", noScope.Code, source.calls, noScope.Body.String())
	}
}

func TestDeviceInventoryReadCandidateV2SourceFailureFailsClosed(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	source := &fixtureDeviceInventoryReadV2Source{err: errors.New("fixture unavailable")}
	handler := authenticator.Handler(newDeviceInventoryReadCandidateV2Routes(&deviceInventoryReadCandidateV2Config{
		Enabled: true, Source: source,
	}))
	response := requestConversationAPI(t, handler, identity, http.MethodGet,
		deviceInventoryReadCandidateV2Path, deviceInventoryReadCandidateScope, "", "", "")
	if response.Code != http.StatusBadGateway ||
		!strings.Contains(response.Body.String(), `"code":"conversation_service_error"`) {
		t.Fatalf("v2 source failure status=%d body=%q", response.Code, response.Body.String())
	}
}

func TestDeviceInventoryReadCandidateV2ResponseBudgetFailsClosed(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	value := fixtureDeviceInventoryReadV2Value(owner)
	value.Devices = make([]deviceplacement.SessionPlacementCandidateV2, 0, deviceplacement.MaxDevices)
	for deviceIndex := 0; deviceIndex < deviceplacement.MaxDevices; deviceIndex++ {
		gpus := make([]deviceplacement.GPUDeclarationV2, 0, 32)
		for gpuIndex := 0; gpuIndex < 32; gpuIndex++ {
			gpus = append(gpus, deviceplacement.GPUDeclarationV2{
				ID: fmt.Sprintf("gpu-%02d", gpuIndex), Vendor: strings.Repeat("v", 64),
				MemoryBytes: 1 << 30, AvailableMemoryBytes: 1 << 30,
			})
		}
		value.Devices = append(value.Devices, deviceplacement.SessionPlacementCandidateV2{
			InstanceID: fmt.Sprintf("runner-%03d", deviceIndex), Revision: uint64(deviceIndex + 1),
			Generation: uint64(deviceIndex + 1), HeartbeatSequence: uint64(deviceIndex + 1),
			Device: deviceplacement.DeviceV2{
				DeviceID: fmt.Sprintf("device-%03d", deviceIndex), Owner: owner,
				ApprovalState: "approved", CordonState: "clear", ReservationState: "none",
				Liveness: "online", SnapshotObservedAtMS: 100_000, LeaseExpiresAtMS: 200_000,
				OS: "linux", Architecture: "amd64", AvailableCPUCores: 1,
				AvailableMemoryBytes: 1 << 30, AvailableStorage: 1 << 30,
				Runtimes: []string{}, GPUs: gpus, DataResidencyZones: []string{},
				TrustZone: "unknown", SandboxLevels: []string{}, ConcurrencyLimit: 0,
				ActiveConcurrency: 0,
			},
		})
	}
	source := &fixtureDeviceInventoryReadV2Source{value: value}
	handler := authenticator.Handler(newDeviceInventoryReadCandidateV2Routes(&deviceInventoryReadCandidateV2Config{
		Enabled: true, Source: source,
	}))
	response := requestConversationAPI(t, handler, identity, http.MethodGet,
		deviceInventoryReadCandidateV2Path, deviceInventoryReadCandidateScope, "", "", "")
	if response.Code != http.StatusBadGateway || source.calls != 1 ||
		!strings.Contains(response.Body.String(), `"code":"device_inventory_invalid"`) {
		t.Fatalf("v2 response budget status=%d calls=%d body=%q", response.Code, source.calls, response.Body.String())
	}
}

func TestPersistedInventoryReadV2SourceBindsOwnerAndContext(t *testing.T) {
	owner := deviceinventory.SnapshotOwner{Issuer: "issuer", Subject: "account-42", TenantID: "tenant-slate"}
	source := newPersistedInventoryReadV2Source([]deviceinventory.PersistedInventoryState{
		persistedInventoryReadSourceState(t, owner),
	}, 200_000)
	value, err := source.ReadOwnedDeviceInventoryV2(context.Background(), model.Owner{
		Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
	})
	if err != nil {
		t.Fatalf("v2 persisted source error=%v", err)
	}
	if err := deviceplacement.ValidateSessionDeviceObservationInventoryV2(value); err != nil {
		t.Fatalf("v2 persisted source value: %v", err)
	}
	if value.Owner.Subject != owner.Subject || len(value.Devices) != 1 ||
		value.Devices[0].Device.DeviceID != "device-a" || value.ExecutionAuthorized {
		t.Fatalf("v2 persisted source value=%#v", value)
	}
	if _, err := source.ReadOwnedDeviceInventoryV2(context.Background(), model.Owner{
		Issuer: owner.Issuer, Subject: "account-foreign", TenantID: owner.TenantID,
	}); err == nil {
		t.Fatal("foreign owner was accepted by v2 persisted source")
	}
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if _, err := source.ReadOwnedDeviceInventoryV2(ctx, model.Owner{
		Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
	}); err != context.Canceled {
		t.Fatalf("canceled v2 persisted source error=%v", err)
	}
}

// fixtureDeviceInventoryReadV2Value intentionally exercises reservation and
// multiple GPU declarations so the route cannot accidentally downgrade v2 to
// the legacy single-GPU shape.
func fixtureDeviceInventoryReadV2Value(owner deviceplacement.Owner) deviceplacement.SessionDeviceObservationInventoryV2 {
	return deviceplacement.SessionDeviceObservationInventoryV2{
		SchemaVersion:              deviceplacement.SessionDeviceObservationInventoryV2SchemaVersion,
		EvaluationMode:             deviceplacement.EvaluationMode,
		EvaluatedAtMS:              200_000,
		Owner:                      owner,
		OwnerDeclarationUnverified: true,
		InventoryUnverified:        true,
		Notice:                     deviceplacement.SessionDeviceObservationInventoryV2Notice,
		Devices: []deviceplacement.SessionPlacementCandidateV2{{
			InstanceID: "runner-a", Revision: 1, Generation: 1, HeartbeatSequence: 1,
			Device: deviceplacement.DeviceV2{
				DeviceID: "device-a", Owner: owner, ApprovalState: "approved", CordonState: "clear",
				ReservationState: "reserved", Liveness: "online", SnapshotObservedAtMS: 100_000,
				LeaseExpiresAtMS: 200_000, OS: "linux", Architecture: "amd64", AvailableCPUCores: 7,
				AvailableMemoryBytes: 8 << 30, AvailableStorage: 50 << 30,
				Runtimes: []string{"go", "rust"}, GPUs: []deviceplacement.GPUDeclarationV2{
					{ID: "gpu-a", Vendor: "NVIDIA", MemoryBytes: 16 << 30, AvailableMemoryBytes: 12 << 30},
					{ID: "gpu-b", Vendor: "NVIDIA", MemoryBytes: 8 << 30, AvailableMemoryBytes: 4 << 30},
				},
				DataResidencyZones: []string{}, TrustZone: "unknown", SandboxLevels: []string{},
				ConcurrencyLimit: 0, ActiveConcurrency: 0,
			},
		}},
		ExecutionAuthorized: false, ReservationCreated: false, DispatchPerformed: false,
	}
}
