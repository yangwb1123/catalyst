package appserver

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

func TestClientInstanceResourceViewCandidateDefaultsClosed(t *testing.T) {
	cases := []*clientInstanceResourceViewCandidateConfig{
		nil,
		{},
		{Source: &fixtureClientInstanceResourceViewSource{}},
	}
	for _, config := range cases {
		request := httptest.NewRequest(http.MethodGet, clientInstanceResourceViewCandidatePath, nil)
		response := httptest.NewRecorder()
		newClientInstanceResourceViewCandidateRoutes(config).ServeHTTP(response, request)
		if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
			t.Fatalf("default-disabled resource view response=%d body=%q", response.Code, response.Body.String())
		}
	}
}

func TestPersistedInventoryClientInstanceResourceViewCandidateBindsOwnerAndPreservesResources(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceinventory.SnapshotOwner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	first := persistedInventoryReadSourceState(t, owner)
	first.Device.ReservationState = "reserved"
	second := persistedInventoryReadSourceState(t, owner)
	second.Revision = 2
	second.Device.DeviceID = "device-b"
	second.Runner.DeviceID = "device-b"
	second.Runner.InstanceID = "runner-b"
	second.Runner.Generation = 2
	second.Runner.HeartbeatSequence = 4
	second.Runner.Liveness = "offline"
	instances := []deviceplacement.ClientInstanceSessionViewInstance{
		{InstanceID: "client-web", ClientKind: deviceplacement.ClientKindWeb, SessionIDs: []string{"conversation-a"}, ObservedAtMS: 200_500, Status: "idle"},
		{InstanceID: "client-cli", ClientKind: deviceplacement.ClientKindCLI, SessionIDs: []string{"conversation-a", "conversation-b"}, ObservedAtMS: 200_500, Status: "active"},
	}
	path := writePersistedInventoryFileSetForCandidate(t, owner, []deviceinventory.PersistedInventoryState{second, first})
	source := newPersistedInventoryFileSetClientInstanceResourceViewSource(path, instances)
	handler := authenticator.Handler(newClientInstanceResourceViewCandidateRoutes(&clientInstanceResourceViewCandidateConfig{
		Enabled: true, Source: source,
	}))
	response := requestConversationAPI(t, handler, identity, http.MethodGet,
		clientInstanceResourceViewCandidatePath, clientInstanceResourceViewCandidateScope, "", "", "")
	if response.Code != http.StatusOK {
		t.Fatalf("resource view candidate status=%d body=%q", response.Code, response.Body.String())
	}
	var got deviceplacement.ClientInstanceResourceViewObservation
	if err := json.Unmarshal(response.Body.Bytes(), &got); err != nil {
		t.Fatalf("decode resource view candidate: %v", err)
	}
	if err := got.Validate(); err != nil {
		t.Fatalf("validate resource view candidate: %v", err)
	}
	if got.Owner.Subject != "account-42" || len(got.Instances) != 2 || got.Instances[0].InstanceID != "client-cli" ||
		len(got.Devices) != 2 || got.Devices[0].DeviceID != "device-a" || got.Devices[0].RunnerInstanceID != "runner-a" ||
		got.Devices[0].ReservationState != "reserved" || got.Devices[1].Liveness != "offline" ||
		got.Authority != (deviceplacement.ClientInstanceResourceViewAuthority{}) || !got.ReadOnly {
		t.Fatalf("resource view candidate=%#v", got)
	}

	production := authenticator.Handler(newConversationRoutes(nil))
	request := httptest.NewRequest(http.MethodGet, clientInstanceResourceViewCandidatePath, nil)
	request.Header.Set("Authorization", "Bearer "+identity.token(clientInstanceResourceViewCandidateScope))
	productionResponse := httptest.NewRecorder()
	production.ServeHTTP(productionResponse, request)
	if productionResponse.Code != http.StatusNotFound {
		t.Fatalf("production resource view route status=%d body=%q", productionResponse.Code, productionResponse.Body.String())
	}
}

func TestClientInstanceResourceViewCandidateRejectsQueryBodyScopeAndForeignOwner(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	source := &fixtureClientInstanceResourceViewSource{value: fixtureClientInstanceResourceView(owner)}
	handler := authenticator.Handler(newClientInstanceResourceViewCandidateRoutes(&clientInstanceResourceViewCandidateConfig{
		Enabled: true, Source: source,
	}))
	query := requestConversationAPI(t, handler, identity, http.MethodGet,
		clientInstanceResourceViewCandidatePath+"?limit=1", clientInstanceResourceViewCandidateScope, "", "", "")
	if query.Code != http.StatusBadRequest || source.calls != 0 {
		t.Fatalf("query status=%d calls=%d body=%q", query.Code, source.calls, query.Body.String())
	}
	method := requestConversationAPI(t, handler, identity, http.MethodPost,
		clientInstanceResourceViewCandidatePath, clientInstanceResourceViewCandidateScope, "application/json", "", `{}`)
	if method.Code != http.StatusMethodNotAllowed || source.calls != 0 {
		t.Fatalf("method status=%d calls=%d body=%q", method.Code, source.calls, method.Body.String())
	}
	noScope := requestConversationAPI(t, handler, identity, http.MethodGet,
		clientInstanceResourceViewCandidatePath, "forge:conversations:read", "", "", "")
	if noScope.Code != http.StatusForbidden || source.calls != 0 {
		t.Fatalf("scope status=%d calls=%d body=%q", noScope.Code, source.calls, noScope.Body.String())
	}
	foreign := &fixtureClientInstanceResourceViewSource{value: fixtureClientInstanceResourceView(deviceplacement.Owner{
		Issuer: identity.issuer, Subject: "account-foreign", TenantID: "tenant-slate",
	})}
	foreignHandler := authenticator.Handler(newClientInstanceResourceViewCandidateRoutes(&clientInstanceResourceViewCandidateConfig{
		Enabled: true, Source: foreign,
	}))
	foreignResponse := requestConversationAPI(t, foreignHandler, identity, http.MethodGet,
		clientInstanceResourceViewCandidatePath, clientInstanceResourceViewCandidateScope, "", "", "")
	if foreignResponse.Code != http.StatusBadGateway || !strings.Contains(foreignResponse.Body.String(), `"code":"client_instance_resource_view_invalid"`) {
		t.Fatalf("foreign source status=%d body=%q", foreignResponse.Code, foreignResponse.Body.String())
	}
}

type fixtureClientInstanceResourceViewSource struct {
	value deviceplacement.ClientInstanceResourceViewObservation
	err   error
	calls int
	owner model.Owner
}

func (source *fixtureClientInstanceResourceViewSource) ReadOwnedClientInstanceResourceView(
	_ context.Context,
	owner model.Owner,
) (deviceplacement.ClientInstanceResourceViewObservation, error) {
	source.calls++
	source.owner = owner
	return source.value, source.err
}

func fixtureClientInstanceResourceView(owner deviceplacement.Owner) deviceplacement.ClientInstanceResourceViewObservation {
	view, err := deviceplacement.ObserveClientInstanceResourceView(deviceplacement.ClientInstanceResourceViewRequest{
		Owner: owner,
		Instances: []deviceplacement.ClientInstanceSessionViewInstance{{
			InstanceID: "client-cli", ClientKind: deviceplacement.ClientKindCLI,
			SessionIDs: []string{"conversation-a"}, ObservedAtMS: 200_500, Status: "active",
		}},
		Devices: []deviceplacement.ClientInstanceResourceViewDevice{{
			DeviceID: "device-a", RunnerInstanceID: "runner-a", Owner: owner, Revision: 1,
			Generation: 1, HeartbeatSequence: 1, ObservedAtMS: 100_000,
			ApprovalState: "approved", CordonState: "clear", ReservationState: "none", Liveness: "online",
			OS: "linux", Architecture: "amd64", CPUCores: 8, AvailableCPUCores: 7,
			MemoryBytes: 16 << 30, AvailableMemoryBytes: 8 << 30, StorageBytes: 100 << 30,
			AvailableStorageBytes: 50 << 30,
		}},
	})
	if err != nil {
		panic(err)
	}
	return view
}
