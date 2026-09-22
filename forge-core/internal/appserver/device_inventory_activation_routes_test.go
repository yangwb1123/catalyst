package appserver

import (
	"encoding/json"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/devicefabricgate"
	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"forgeos/forge-core/internal/statefs"
)

func TestDeviceFabricActivationRequiresARealOwnerPrivateInventoryImage(t *testing.T) {
	path := filepath.Join(t.TempDir(), "lifecycle-registry.json")
	config := testConfig(t)
	config.DeviceInventoryLifecycleRegistryFile = path
	if err := config.Validate(); err == nil || !strings.Contains(err.Error(), "requires an enabled device fabric activation") {
		t.Fatalf("inventory image without activation error=%v", err)
	}

	config = testConfig(t)
	config.DeviceFabricActivation = ptrDeviceFabricRequest(acceptedInventoryActivation())
	if err := config.Validate(); err == nil || !strings.Contains(err.Error(), "requires an owner-private lifecycle registry file") {
		t.Fatalf("activation without inventory image error=%v", err)
	}

	config = testConfig(t)
	config.DeviceFabricActivation = ptrDeviceFabricRequest(acceptedInventoryActivation())
	config.DeviceInventoryLifecycleRegistryFile = path
	config.RuntimeExecutable = filepath.Join(t.TempDir(), "forge-runtime")
	if err := os.WriteFile(config.RuntimeExecutable, []byte("runtime"), 0o700); err != nil {
		t.Fatal(err)
	}
	config.RuntimeStateDir = filepath.Join(t.TempDir(), "runtime-state")
	config.SnaplinkIssuer = "https://identity.example"
	config.SnaplinkAudience = "forge-api"
	config.ExpectedTenantID = "tenant-slate"
	config.ExpectedSubjectID = "account-42"
	if err := config.Validate(); err != nil {
		t.Fatalf("accepted inventory activation rejected: %v", err)
	}

	config = testConfig(t)
	config.DeviceClientInstanceSessionViewFile = filepath.Join(t.TempDir(), "client-instance-view.json")
	if err := config.Validate(); err == nil || !strings.Contains(err.Error(), "requires an enabled device fabric activation") {
		t.Fatalf("client-instance image without activation error=%v", err)
	}
}

func TestAcceptedDeviceFabricActivationMountsOwnerScopedInventoryRead(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	first := lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1)
	second := lifecycleRegistrySourceState(t, owner, "device-b", "runner-b", 1)
	path := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{second, first})
	clientInstancePath := writeClientInstanceSessionViewSourceFile(t, owner)
	sessions, err := newAuthenticatedSessionRoutesWithDeviceFabricActivation(
		nil, nil, ptrDeviceFabricRequest(acceptedInventoryActivation()), path,
		clientInstancePath,
	)
	if err != nil {
		t.Fatal(err)
	}
	handler := authenticator.Handler(sessions)

	response := requestConversationAPI(t, handler, identity, http.MethodGet,
		lifecycleRegistryCandidatePath, lifecycleRegistryCandidateReadScope, "", "", "")
	if response.Code != http.StatusOK {
		t.Fatalf("activated lifecycle registry status=%d body=%q", response.Code, response.Body.String())
	}
	var lifecycle lifecycleRegistryCandidateEnvelope
	if err := json.Unmarshal(response.Body.Bytes(), &lifecycle); err != nil {
		t.Fatalf("decode activated lifecycle registry: %v", err)
	}
	if lifecycle.Owner != owner || len(lifecycle.States) != 2 || lifecycle.SchemaVersion == "" {
		t.Fatalf("activated lifecycle registry=%#v", lifecycle)
	}
	response = requestConversationAPI(t, handler, identity, http.MethodPut,
		lifecycleRegistryCandidatePath, lifecycleRegistryCandidateReadScope,
		"application/json", "", `{"states":[]}`)
	if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
		t.Fatalf("activated lifecycle write status=%d body=%q", response.Code, response.Body.String())
	}

	response = requestConversationAPI(t, handler, identity, http.MethodGet,
		deviceInventoryReadCandidatePath, deviceInventoryReadCandidateScope, "", "", "")
	if response.Code != http.StatusOK {
		t.Fatalf("activated v1 status=%d body=%q", response.Code, response.Body.String())
	}
	var v1 deviceplacement.SessionDeviceObservationInventory
	if err := json.Unmarshal(response.Body.Bytes(), &v1); err != nil {
		t.Fatalf("decode activated v1: %v", err)
	}
	if err := deviceplacement.ValidateSessionDeviceObservationInventory(v1); err != nil {
		t.Fatalf("validate activated v1: %v", err)
	}
	if v1.Owner.Subject != owner.Subject || len(v1.Devices) != 2 ||
		v1.Devices[0].Device.DeviceID != "device-a" || v1.Devices[1].Device.DeviceID != "device-b" ||
		v1.ExecutionAuthorized || v1.ReservationCreated || v1.DispatchPerformed {
		t.Fatalf("activated v1 projection=%#v", v1)
	}

	response = requestConversationAPI(t, handler, identity, http.MethodGet,
		deviceInventoryReadCandidateV2Path, deviceInventoryReadCandidateScope, "", "", "")
	if response.Code != http.StatusOK {
		t.Fatalf("activated v2 status=%d body=%q", response.Code, response.Body.String())
	}
	var v2 deviceplacement.SessionDeviceObservationInventoryV2
	if err := json.Unmarshal(response.Body.Bytes(), &v2); err != nil {
		t.Fatalf("decode activated v2: %v", err)
	}
	if err := deviceplacement.ValidateSessionDeviceObservationInventoryV2(v2); err != nil {
		t.Fatalf("validate activated v2: %v", err)
	}
	if v2.Owner.Subject != owner.Subject || len(v2.Devices) != 2 ||
		v2.Devices[0].Device.DeviceID != "device-a" || v2.Devices[1].Device.DeviceID != "device-b" ||
		v2.ExecutionAuthorized || v2.ReservationCreated || v2.DispatchPerformed {
		t.Fatalf("activated v2 projection=%#v", v2)
	}

	response = requestConversationAPI(t, handler, identity, http.MethodPost,
		devicePlacementRegistryCandidatePath, devicePlacementRegistryCandidateScope,
		"application/json", "", registryPlacementRequirementsBody(t))
	if response.Code != http.StatusOK {
		t.Fatalf("activated registry placement status=%d body=%q", response.Code, response.Body.String())
	}
	var placement deviceplacement.PersistedInventoryPlacementV2Evaluation
	if err := json.Unmarshal(response.Body.Bytes(), &placement); err != nil {
		t.Fatalf("decode activated registry placement: %v", err)
	}
	if !validDevicePlacementRegistryCandidateResult(
		placement, model.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}, placement.EvaluatedAtMS,
	) || len(placement.Decisions) != 2 || placement.SelectedDeviceID != nil || placement.SelectedInstanceID != nil ||
		placement.Authority != (deviceplacement.PersistedInventoryPlacementBatchAuthority{}) {
		t.Fatalf("activated registry placement=%#v", placement)
	}

	routes, err := newRoutesWithSessions(
		BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, handler,
	)
	if err != nil {
		t.Fatal(err)
	}
	response = requestConversationAPI(t, routes, identity, http.MethodGet,
		deviceInventoryReadCandidatePath, deviceInventoryReadCandidateScope, "", "", "")
	if response.Code != http.StatusOK {
		t.Fatalf("activated outer route status=%d body=%q", response.Code, response.Body.String())
	}

	response = requestConversationAPI(t, handler, identity, http.MethodGet,
		clientInstanceSessionViewCandidatePath, clientInstanceSessionViewCandidateScope, "", "", "")
	if response.Code != http.StatusOK {
		t.Fatalf("activated client-instance session view status=%d body=%q", response.Code, response.Body.String())
	}
	var sessionsView deviceplacement.ClientInstanceSessionViewObservation
	if err := json.Unmarshal(response.Body.Bytes(), &sessionsView); err != nil {
		t.Fatalf("decode activated client-instance session view: %v", err)
	}
	if err := sessionsView.Validate(); err != nil || sessionsView.Owner.Subject != owner.Subject || len(sessionsView.Instances) != 5 || !sessionsView.ReadOnly {
		t.Fatalf("activated client-instance session view=%#v err=%v", sessionsView, err)
	}

	response = requestConversationAPI(t, handler, identity, http.MethodGet,
		clientInstanceResourceViewCandidatePath, clientInstanceResourceViewCandidateScope, "", "", "")
	if response.Code != http.StatusOK {
		t.Fatalf("activated client-instance resource view status=%d body=%q", response.Code, response.Body.String())
	}
	var resourceView deviceplacement.ClientInstanceResourceViewObservation
	if err := json.Unmarshal(response.Body.Bytes(), &resourceView); err != nil {
		t.Fatalf("decode activated client-instance resource view: %v", err)
	}
	if err := resourceView.Validate(); err != nil || resourceView.Owner.Subject != owner.Subject || len(resourceView.Instances) != 5 || len(resourceView.Devices) != 2 || !resourceView.ReadOnly || resourceView.Authority.ExecutionAuthorized || resourceView.Authority.ReservationCreated || resourceView.Authority.DispatchPerformed {
		t.Fatalf("activated client-instance resource view=%#v err=%v", resourceView, err)
	}
}

func TestAcceptedDeviceFabricActivationReloadsObservationImagesPerRead(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	firstState := lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1)
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{firstState})
	clientPath := writeClientInstanceSessionViewSourceFile(t, owner)
	sessions, err := newAuthenticatedSessionRoutesWithDeviceFabricActivation(
		nil, nil, ptrDeviceFabricRequest(acceptedInventoryActivation()), registryPath, clientPath,
	)
	if err != nil {
		t.Fatal(err)
	}
	handler := authenticator.Handler(sessions)

	readSession := func() deviceplacement.ClientInstanceSessionViewObservation {
		t.Helper()
		response := requestConversationAPI(t, handler, identity, http.MethodGet,
			clientInstanceSessionViewCandidatePath, clientInstanceSessionViewCandidateScope, "", "", "")
		if response.Code != http.StatusOK {
			t.Fatalf("session view status=%d body=%q", response.Code, response.Body.String())
		}
		var view deviceplacement.ClientInstanceSessionViewObservation
		if err := json.Unmarshal(response.Body.Bytes(), &view); err != nil {
			t.Fatalf("decode session view: %v", err)
		}
		if err := view.Validate(); err != nil {
			t.Fatalf("validate session view: %v", err)
		}
		return view
	}
	readResource := func() deviceplacement.ClientInstanceResourceViewObservation {
		t.Helper()
		response := requestConversationAPI(t, handler, identity, http.MethodGet,
			clientInstanceResourceViewCandidatePath, clientInstanceResourceViewCandidateScope, "", "", "")
		if response.Code != http.StatusOK {
			t.Fatalf("resource view status=%d body=%q", response.Code, response.Body.String())
		}
		var view deviceplacement.ClientInstanceResourceViewObservation
		if err := json.Unmarshal(response.Body.Bytes(), &view); err != nil {
			t.Fatalf("decode resource view: %v", err)
		}
		if err := view.Validate(); err != nil {
			t.Fatalf("validate resource view: %v", err)
		}
		return view
	}

	initialSession := readSession()
	if len(initialSession.Instances) != 5 || initialSession.Instances[0].InstanceID != "client-app-001" {
		t.Fatalf("initial session view=%#v", initialSession)
	}
	initialResource := readResource()
	if len(initialResource.Devices) != 1 || initialResource.Devices[0].DeviceID != "device-a" ||
		initialResource.Devices[0].ObservedAtMS != int64(firstState.Inventory.Runner.ServerObservedAtMS) {
		t.Fatalf("initial resource view=%#v", initialResource)
	}

	secondState := lifecycleRegistrySourceState(t, owner, "device-b", "runner-b", 1)
	writeLifecycleRegistrySourceFileAtPath(t, registryPath, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{secondState})
	secondInstances := activationClientInstanceRows()
	secondInstances[3].InstanceID = "client-app-002"
	secondInstances[3].SessionIDs = []string{"conversation-reloaded"}
	secondInstances[3].ObservedAtMS = 201_000
	writeClientInstanceSessionViewSourceFileAtPath(t, clientPath, owner, secondInstances)

	reloadedSession := readSession()
	if len(reloadedSession.Instances) != 5 || reloadedSession.Instances[0].InstanceID != "client-app-002" ||
		reloadedSession.Instances[0].SessionIDs[0] != "conversation-reloaded" {
		t.Fatalf("reloaded session view=%#v", reloadedSession)
	}
	reloadedResource := readResource()
	if len(reloadedResource.Devices) != 1 || reloadedResource.Devices[0].DeviceID != "device-b" ||
		reloadedResource.Devices[0].RunnerInstanceID != "runner-b" ||
		reloadedResource.Devices[0].ObservedAtMS != int64(secondState.Inventory.Runner.ServerObservedAtMS) ||
		reloadedResource.Authority != (deviceplacement.ClientInstanceResourceViewAuthority{}) {
		t.Fatalf("reloaded resource view=%#v", reloadedResource)
	}
}

func TestDeviceFabricActivationRouteRejectsBlockedRequest(t *testing.T) {
	_, err := newAuthenticatedSessionRoutesWithDeviceFabricActivation(
		nil, nil, &devicefabricgate.Request{Mode: devicefabricgate.ModeInventory}, filepath.Join(t.TempDir(), "registry.json"),
		"",
	)
	if err == nil || !strings.Contains(err.Error(), "device fabric activation blocked") {
		t.Fatalf("blocked activation route error=%v", err)
	}
}

func TestAcceptedObserveActivationMountsTheSameOwnerScopedReadOnlyRoutes(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{
		lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1),
	})
	activation := acceptedInventoryActivation()
	activation.Mode = devicefabricgate.ModeObserve
	sessions, err := newAuthenticatedSessionRoutesWithDeviceFabricActivation(
		nil, nil, ptrDeviceFabricRequest(activation), registryPath, "",
	)
	if err != nil {
		t.Fatalf("accepted observe activation rejected: %v", err)
	}
	handler := authenticator.Handler(sessions)
	for _, route := range []string{
		lifecycleRegistryCandidatePath,
		deviceInventoryReadCandidatePath,
		deviceInventoryReadCandidateV2Path,
		devicePlacementRegistryCandidatePath,
	} {
		method := http.MethodGet
		body := ""
		contentType := ""
		if route == devicePlacementRegistryCandidatePath {
			method = http.MethodPost
			body = registryPlacementRequirementsBody(t)
			contentType = "application/json"
		}
		response := requestConversationAPI(t, handler, identity, method, route,
			map[string]string{
				lifecycleRegistryCandidatePath:       lifecycleRegistryCandidateReadScope,
				deviceInventoryReadCandidatePath:     deviceInventoryReadCandidateScope,
				deviceInventoryReadCandidateV2Path:   deviceInventoryReadCandidateScope,
				devicePlacementRegistryCandidatePath: devicePlacementRegistryCandidateScope,
			}[route], contentType, "", body)
		if response.Code != http.StatusOK {
			t.Fatalf("accepted observe route %s status=%d body=%q", route, response.Code, response.Body.String())
		}
	}
	for _, route := range []string{
		conversationCollectionPath + "/conversation-1/execution-consents",
		conversationCollectionPath + "/conversation-1/run-intents",
		conversationCollectionPath + "/conversation-1/runs/run-1/runner-dispatch-plan-preview",
		schedulerSelectionPreviewPath,
	} {
		method := http.MethodGet
		contentType := ""
		body := ""
		scope := "forge:conversations:read"
		if strings.HasSuffix(route, "runner-dispatch-plan-preview") {
			method = http.MethodPost
			contentType = "application/json"
			body = `{}`
		} else if route == schedulerSelectionPreviewPath {
			method = http.MethodPost
			contentType = "application/json"
			scope = schedulerSelectionPreviewScope
			body = `{}`
		}
		response := requestConversationAPI(t, handler, identity, method, route,
			scope, contentType, "", body)
		if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
			t.Fatalf("accepted observe execution route %s status=%d body=%q", route, response.Code, response.Body.String())
		}
	}
}

func TestExecutionAdmissionAssemblyMountsOnlyConsentAndIntentSurface(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	activation := acceptedInventoryActivation()
	activation.Mode = devicefabricgate.ModeExecute
	activation.P4 = devicefabricgate.Decision{
		Status: "accepted", AcceptanceID: "p4-acceptance", AcceptedAtUnixMS: 1,
	}
	activation.Evidence.RunnerIsolation = true
	activation.Evidence.LeaseFencing = true
	activation.Evidence.CancellationAndUncertainWork = true
	activation.Evidence.VaultArtifactAuthorization = true
	activation.Evidence.AuditOutbox = true
	sessions, err := newAuthenticatedSessionRoutesWithDeviceFabricActivation(
		nil, nil, ptrDeviceFabricRequest(activation), filepath.Join(t.TempDir(), "registry.json"), "",
	)
	if err != nil {
		t.Fatalf("execution admission route assembly error=%v", err)
	}
	handler := authenticator.Handler(sessions)
	response := requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationCollectionPath+"/conversation-1/execution-consents", "forge:conversations:read", "", "", "")
	if response.Code != http.StatusServiceUnavailable {
		t.Fatalf("execution consent route status=%d body=%q", response.Code, response.Body.String())
	}
	response = requestConversationAPI(t, handler, identity, http.MethodGet,
		conversationCollectionPath+"/conversation-1/run-intents", "forge:conversations:read", "", "", "")
	if response.Code != http.StatusServiceUnavailable {
		t.Fatalf("pending intent route status=%d body=%q", response.Code, response.Body.String())
	}
	response = requestConversationAPI(t, handler, identity, http.MethodDelete,
		"/api/v1/execution-consents/grant-1", "forge:conversations:write", "", "revoke-key", "")
	if response.Code != http.StatusServiceUnavailable {
		t.Fatalf("execution consent revoke route status=%d body=%q", response.Code, response.Body.String())
	}
	for _, route := range []string{
		conversationCollectionPath + "/conversation-1/runs/run-1/device-observation/preview",
		conversationCollectionPath + "/conversation-1/runs/run-1/runner-receipt-observation/preview",
		conversationCollectionPath + "/conversation-1/runs/run-1/attempt-lease-dispatch-preflight/preview",
		conversationCollectionPath + "/conversation-1/runs/run-1/runner-dispatch-plan-preview",
		conversationCollectionPath + "/conversation-1/runs/run-1/execution-reconciliation/preview",
	} {
		response := requestConversationAPI(t, handler, identity, http.MethodPost, route,
			"forge:conversations:read", "application/json", "", `{}`)
		if response.Code != http.StatusBadRequest {
			t.Fatalf("execution preflight route %s status=%d body=%q", route, response.Code, response.Body.String())
		}
	}
	response = requestConversationAPI(t, handler, identity, http.MethodPost,
		schedulerSelectionPreviewPath, schedulerSelectionPreviewScope, "application/json", "", `{}`)
	if response.Code != http.StatusBadRequest {
		t.Fatalf("scheduler selection preview route status=%d body=%q", response.Code, response.Body.String())
	}
}

func acceptedInventoryActivation() devicefabricgate.Request {
	accepted := devicefabricgate.Decision{
		Status:           "accepted",
		AcceptanceID:     "review-inventory-001",
		AcceptedAtUnixMS: 1,
	}
	return devicefabricgate.Request{
		Mode:    devicefabricgate.ModeInventory,
		ADR0039: accepted,
		ADR0113: accepted,
		ADR0114: accepted,
		Evidence: devicefabricgate.Evidence{
			CoordinatorOwnerIsolation:    true,
			DeviceIdentityProof:          true,
			OwnerApprovalAndRevocation:   true,
			HeartbeatCASAndFreshness:     true,
			InventoryOwnerScope:          true,
			DisabledDefaultAndRouteClose: true,
			SecurityReview:               true,
		},
	}
}

func ptrDeviceFabricRequest(request devicefabricgate.Request) *devicefabricgate.Request {
	return &request
}

func writeClientInstanceSessionViewSourceFile(t *testing.T, owner deviceidentity.Owner) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "client-instance-session-view.json")
	writeClientInstanceSessionViewSourceFileAtPath(t, path, owner, activationClientInstanceRows())
	return path
}

func writeClientInstanceSessionViewSourceFileAtPath(
	t *testing.T,
	path string,
	owner deviceidentity.Owner,
	instances []deviceplacement.ClientInstanceSessionViewInstance,
) {
	t.Helper()
	view, err := deviceplacement.ObserveClientInstanceSessionView(deviceplacement.ClientInstanceSessionViewRequest{
		Owner:     deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
		Instances: instances,
	})
	if err != nil {
		t.Fatal(err)
	}
	data, err := json.Marshal(view)
	if err != nil {
		t.Fatal(err)
	}
	if err := statefs.AtomicWrite(path, append(data, '\n'), 0o600); err != nil {
		t.Fatal(err)
	}
}

func activationClientInstanceRows() []deviceplacement.ClientInstanceSessionViewInstance {
	return []deviceplacement.ClientInstanceSessionViewInstance{
		{InstanceID: "client-cli-001", ClientKind: deviceplacement.ClientKindCLI, SessionIDs: []string{"conversation-a", "conversation-b"}, ObservedAtMS: 200500, Status: "active"},
		{InstanceID: "client-tui-001", ClientKind: deviceplacement.ClientKindTUI, SessionIDs: []string{"conversation-a", "conversation-b"}, ObservedAtMS: 200500, Status: "active"},
		{InstanceID: "client-web-001", ClientKind: deviceplacement.ClientKindWeb, SessionIDs: []string{"conversation-a", "conversation-b"}, ObservedAtMS: 200500, Status: "idle"},
		{InstanceID: "client-app-001", ClientKind: deviceplacement.ClientKindApp, SessionIDs: []string{"conversation-a"}, ObservedAtMS: 200500, Status: "active"},
		{InstanceID: "client-mobile-001", ClientKind: deviceplacement.ClientKindMobile, SessionIDs: []string{"conversation-b"}, ObservedAtMS: 200500, Status: "idle"},
	}
}
