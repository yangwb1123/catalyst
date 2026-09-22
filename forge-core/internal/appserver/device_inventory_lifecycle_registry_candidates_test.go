package appserver

import (
	"encoding/json"
	"net/http"
	"strings"
	"testing"

	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
)

func TestLifecycleRegistryInventoryCandidateCompositionProjectsV1AndV2FromOneOwnerSource(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	first := lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1)
	second := lifecycleRegistrySourceState(t, owner, "device-b", "runner-b", 1)
	path := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{second, first})
	handler := authenticator.Handler(newAuthenticatedSessionRoutesWithLifecycleRegistryInventoryCandidates(
		nil, nil, path, 200_000,
	))

	v1Response := requestConversationAPI(t, handler, identity, http.MethodGet,
		deviceInventoryReadCandidatePath, deviceInventoryReadCandidateScope, "", "", "")
	if v1Response.Code != http.StatusOK {
		t.Fatalf("v1 status=%d body=%q", v1Response.Code, v1Response.Body.String())
	}
	var v1 deviceplacement.SessionDeviceObservationInventory
	if err := json.Unmarshal(v1Response.Body.Bytes(), &v1); err != nil {
		t.Fatalf("decode v1: %v", err)
	}
	if err := deviceplacement.ValidateSessionDeviceObservationInventory(v1); err != nil {
		t.Fatalf("validate v1: %v", err)
	}
	if v1.Owner.Subject != owner.Subject || len(v1.Devices) != 2 ||
		v1.Devices[0].Device.DeviceID != "device-a" || v1.Devices[0].InstanceID != "runner-a" ||
		v1.Devices[1].Device.DeviceID != "device-b" || v1.Devices[1].InstanceID != "runner-b" ||
		v1.ExecutionAuthorized || v1.ReservationCreated || v1.DispatchPerformed {
		t.Fatalf("v1 projection=%#v", v1)
	}

	v2Response := requestConversationAPI(t, handler, identity, http.MethodGet,
		deviceInventoryReadCandidateV2Path, deviceInventoryReadCandidateScope, "", "", "")
	if v2Response.Code != http.StatusOK {
		t.Fatalf("v2 status=%d body=%q", v2Response.Code, v2Response.Body.String())
	}
	var v2 deviceplacement.SessionDeviceObservationInventoryV2
	if err := json.Unmarshal(v2Response.Body.Bytes(), &v2); err != nil {
		t.Fatalf("decode v2: %v", err)
	}
	if err := deviceplacement.ValidateSessionDeviceObservationInventoryV2(v2); err != nil {
		t.Fatalf("validate v2: %v", err)
	}
	if v2.Owner.Subject != owner.Subject || len(v2.Devices) != 2 ||
		v2.Devices[0].Device.DeviceID != "device-a" || v2.Devices[0].Revision != first.Revision ||
		v2.Devices[0].HeartbeatSequence != first.Heartbeat.Instance.HeartbeatSequence ||
		v2.Devices[1].Device.DeviceID != "device-b" || v2.Devices[1].Revision != second.Revision ||
		v2.Devices[1].HeartbeatSequence != second.Heartbeat.Instance.HeartbeatSequence ||
		v2.ExecutionAuthorized || v2.ReservationCreated || v2.DispatchPerformed {
		t.Fatalf("v2 projection=%#v", v2)
	}
}

func TestLifecycleRegistryInventoryCandidateCompositionRemainsOutsideProductionConstructor(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	sessions := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	for _, path := range []string{deviceInventoryReadCandidatePath, deviceInventoryReadCandidateV2Path} {
		response := requestConversationAPI(t, sessions, identity, http.MethodGet,
			path, deviceInventoryReadCandidateScope, "", "", "")
		if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
			t.Fatalf("production candidate path=%s status=%d body=%q", path, response.Code, response.Body.String())
		}
	}
}

func TestLifecycleRegistryClientInstanceCandidatesProjectOneOwnerSource(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	first := lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1)
	second := lifecycleRegistrySourceState(t, owner, "device-b", "runner-b", 1)
	path := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{second, first})
	instances := []deviceplacement.ClientInstanceSessionViewInstance{
		{InstanceID: "client-mobile", ClientKind: deviceplacement.ClientKindMobile, SessionIDs: []string{"conversation-mobile"}, ObservedAtMS: 200_000, Status: "idle"},
		{InstanceID: "client-web", ClientKind: deviceplacement.ClientKindWeb, SessionIDs: []string{"conversation-web", "conversation-shared"}, ObservedAtMS: 200_000, Status: "active"},
		{InstanceID: "client-cli", ClientKind: deviceplacement.ClientKindCLI, SessionIDs: []string{"conversation-shared"}, ObservedAtMS: 200_000, Status: "active"},
		{InstanceID: "client-app", ClientKind: deviceplacement.ClientKindApp, SessionIDs: []string{"conversation-app"}, ObservedAtMS: 200_000, Status: "idle"},
		{InstanceID: "client-tui", ClientKind: deviceplacement.ClientKindTUI, SessionIDs: []string{"conversation-shared"}, ObservedAtMS: 200_000, Status: "active"},
	}
	handler := authenticator.Handler(newAuthenticatedSessionRoutesWithLifecycleRegistryClientInstanceCandidates(
		nil, nil, path, 200_000, instances,
	))

	sessionResponse := requestConversationAPI(t, handler, identity, http.MethodGet,
		clientInstanceSessionViewCandidatePath, clientInstanceSessionViewCandidateScope, "", "", "")
	if sessionResponse.Code != http.StatusOK {
		t.Fatalf("session view status=%d body=%q", sessionResponse.Code, sessionResponse.Body.String())
	}
	var sessionView deviceplacement.ClientInstanceSessionViewObservation
	if err := json.Unmarshal(sessionResponse.Body.Bytes(), &sessionView); err != nil {
		t.Fatalf("decode session view: %v", err)
	}
	if err := sessionView.Validate(); err != nil {
		t.Fatalf("validate session view: %v", err)
	}
	if sessionView.Owner.Subject != owner.Subject || len(sessionView.Instances) != len(instances) ||
		sessionView.Instances[0].InstanceID != "client-app" || sessionView.Instances[4].InstanceID != "client-web" ||
		sessionView.Authority != (deviceplacement.ClientInstanceSessionViewAuthority{}) || !sessionView.ReadOnly {
		t.Fatalf("session view=%#v", sessionView)
	}

	resourceResponse := requestConversationAPI(t, handler, identity, http.MethodGet,
		clientInstanceResourceViewCandidatePath, clientInstanceResourceViewCandidateScope, "", "", "")
	if resourceResponse.Code != http.StatusOK {
		t.Fatalf("resource view status=%d body=%q", resourceResponse.Code, resourceResponse.Body.String())
	}
	var resourceView deviceplacement.ClientInstanceResourceViewObservation
	if err := json.Unmarshal(resourceResponse.Body.Bytes(), &resourceView); err != nil {
		t.Fatalf("decode resource view: %v", err)
	}
	if err := resourceView.Validate(); err != nil {
		t.Fatalf("validate resource view: %v", err)
	}
	if resourceView.Owner.Subject != owner.Subject || len(resourceView.Instances) != len(instances) ||
		len(resourceView.Devices) != 2 || resourceView.Devices[0].DeviceID != "device-a" ||
		resourceView.Devices[1].DeviceID != "device-b" ||
		resourceView.Authority != (deviceplacement.ClientInstanceResourceViewAuthority{}) || !resourceView.ReadOnly {
		t.Fatalf("resource view=%#v", resourceView)
	}
}

func TestLifecycleRegistryClientInstanceCandidatesRejectOwnerDriftAndDuplicateRows(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	state := lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1)

	t.Run("foreign registry owner", func(t *testing.T) {
		foreign := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-foreign", TenantID: owner.TenantID}
		path := writeLifecycleRegistrySourceFile(t, foreign, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{
			lifecycleRegistrySourceState(t, foreign, "device-a", "runner-a", 1),
		})
		handler := authenticator.Handler(newAuthenticatedSessionRoutesWithLifecycleRegistryClientInstanceCandidates(
			nil, nil, path, 200_000, validLifecycleClientInstances(),
		))
		for _, route := range []string{clientInstanceSessionViewCandidatePath, clientInstanceResourceViewCandidatePath} {
			scope := clientInstanceResourceViewCandidateScope
			if route == clientInstanceSessionViewCandidatePath {
				scope = clientInstanceSessionViewCandidateScope
			}
			response := requestConversationAPI(t, handler, identity, http.MethodGet, route,
				scope, "", "", "")
			if response.Code != http.StatusBadGateway || !strings.Contains(response.Body.String(), `"code":"conversation_service_error"`) {
				t.Fatalf("foreign route=%s status=%d body=%q", route, response.Code, response.Body.String())
			}
		}
	})

	t.Run("duplicate client instance", func(t *testing.T) {
		path := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{state})
		instances := validLifecycleClientInstances()
		instances = append(instances, instances[0])
		handler := authenticator.Handler(newAuthenticatedSessionRoutesWithLifecycleRegistryClientInstanceCandidates(
			nil, nil, path, 200_000, instances,
		))
		for _, route := range []string{clientInstanceSessionViewCandidatePath, clientInstanceResourceViewCandidatePath} {
			scope := clientInstanceResourceViewCandidateScope
			if route == clientInstanceSessionViewCandidatePath {
				scope = clientInstanceSessionViewCandidateScope
			}
			response := requestConversationAPI(t, handler, identity, http.MethodGet, route,
				scope, "", "", "")
			if response.Code != http.StatusBadGateway || !strings.Contains(response.Body.String(), `"code":"conversation_service_error"`) {
				t.Fatalf("duplicate instance route=%s status=%d body=%q", route, response.Code, response.Body.String())
			}
		}
	})

	t.Run("duplicate persisted device", func(t *testing.T) {
		path := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{state, state})
		handler := authenticator.Handler(newAuthenticatedSessionRoutesWithLifecycleRegistryClientInstanceCandidates(
			nil, nil, path, 200_000, validLifecycleClientInstances(),
		))
		response := requestConversationAPI(t, handler, identity, http.MethodGet,
			clientInstanceResourceViewCandidatePath, clientInstanceResourceViewCandidateScope, "", "", "")
		if response.Code != http.StatusBadGateway || !strings.Contains(response.Body.String(), `"code":"conversation_service_error"`) {
			t.Fatalf("duplicate device status=%d body=%q", response.Code, response.Body.String())
		}
	})
}

func TestLifecycleRegistryClientInstanceCandidatesRejectTransportShapeAndRemainClosedInProduction(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	state := lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1)
	path := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{state})
	handler := authenticator.Handler(newAuthenticatedSessionRoutesWithLifecycleRegistryClientInstanceCandidates(
		nil, nil, path, 200_000, validLifecycleClientInstances(),
	))

	for _, test := range []struct {
		name, path, scope, wrongScope string
	}{
		{name: "session", path: clientInstanceSessionViewCandidatePath, scope: clientInstanceSessionViewCandidateScope, wrongScope: clientInstanceResourceViewCandidateScope},
		{name: "resource", path: clientInstanceResourceViewCandidatePath, scope: clientInstanceResourceViewCandidateScope, wrongScope: clientInstanceSessionViewCandidateScope},
	} {
		t.Run(test.name, func(t *testing.T) {
			query := requestConversationAPI(t, handler, identity, http.MethodGet,
				test.path+"?limit=1", test.scope, "", "", "")
			if query.Code != http.StatusBadRequest {
				t.Fatalf("query status=%d body=%q", query.Code, query.Body.String())
			}
			body := requestConversationAPI(t, handler, identity, http.MethodGet,
				test.path, test.scope, "application/json", "", `{}`)
			if body.Code != http.StatusBadRequest {
				t.Fatalf("body status=%d body=%q", body.Code, body.Body.String())
			}
			scope := requestConversationAPI(t, handler, identity, http.MethodGet,
				test.path, test.wrongScope, "", "", "")
			if scope.Code != http.StatusForbidden {
				t.Fatalf("scope status=%d body=%q", scope.Code, scope.Body.String())
			}
		})
	}
	production := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	for _, route := range []string{clientInstanceSessionViewCandidatePath, clientInstanceResourceViewCandidatePath} {
		response := requestConversationAPI(t, production, identity, http.MethodGet, route,
			clientInstanceResourceViewCandidateScope, "", "", "")
		if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
			t.Fatalf("production route=%s status=%d body=%q", route, response.Code, response.Body.String())
		}
	}
}

func validLifecycleClientInstances() []deviceplacement.ClientInstanceSessionViewInstance {
	return []deviceplacement.ClientInstanceSessionViewInstance{
		{InstanceID: "client-cli", ClientKind: deviceplacement.ClientKindCLI, SessionIDs: []string{"conversation-shared"}, ObservedAtMS: 200_000, Status: "active"},
	}
}
