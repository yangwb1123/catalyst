package appserver

import (
	"context"
	"encoding/json"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/deviceheartbeat"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"forgeos/forge-core/internal/statefs"
)

func TestPersistedInventoryReadSourceBindsVerifiedOwnerAndReturnsObservation(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	stateOwner := deviceinventory.SnapshotOwner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	state := persistedInventoryReadSourceState(t, stateOwner)
	source := newPersistedInventoryReadSource([]deviceinventory.PersistedInventoryState{state}, 200_000)
	handler := authenticator.Handler(newDeviceInventoryReadCandidateRoutes(&deviceInventoryReadCandidateConfig{
		Enabled: true, Source: source,
	}))
	response := requestConversationAPI(t, handler, identity, http.MethodGet, deviceInventoryReadCandidatePath,
		deviceInventoryReadCandidateScope, "", "", "")
	if response.Code != 200 {
		t.Fatalf("candidate status=%d body=%q", response.Code, response.Body.String())
	}
	var value deviceplacement.SessionDeviceObservationInventory
	err := json.Unmarshal(response.Body.Bytes(), &value)
	if err != nil {
		t.Fatalf("decode observation inventory: %v body=%q", err, response.Body.String())
	}
	if err := deviceplacement.ValidateSessionDeviceObservationInventory(value); err != nil {
		t.Fatalf("validate observation inventory: %v", err)
	}
	if value.Owner != (deviceplacement.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}) ||
		len(value.Devices) != 1 || value.Devices[0].Device.DeviceID != "device-a" ||
		value.Devices[0].InstanceID != "runner-a" || value.ExecutionAuthorized || value.ReservationCreated || value.DispatchPerformed {
		t.Fatalf("observation=%#v", value)
	}
}

func TestPersistedInventoryReadSourceRejectsForeignOwnerAndCanceledContext(t *testing.T) {
	stateOwner := deviceinventory.SnapshotOwner{Issuer: "issuer", Subject: "account-42", TenantID: "tenant-slate"}
	source := newPersistedInventoryReadSource([]deviceinventory.PersistedInventoryState{
		persistedInventoryReadSourceState(t, stateOwner),
	}, 200_000)
	if _, err := source.ReadOwnedDeviceInventory(context.Background(), model.Owner{
		Issuer: "issuer", Subject: "account-foreign", TenantID: "tenant-slate",
	}); err != deviceinventory.ErrInventoryOwnerMismatch {
		t.Fatalf("foreign owner error=%v", err)
	}
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if _, err := source.ReadOwnedDeviceInventory(ctx, model.Owner{
		Issuer: stateOwner.Issuer, Subject: stateOwner.Subject, TenantID: stateOwner.TenantID,
	}); err != context.Canceled {
		t.Fatalf("canceled context error=%v", err)
	}
}

func TestPersistedInventoryFileReadSourceBindsAuthenticatedOwnerAfterRestart(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	stateOwner := deviceinventory.SnapshotOwner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	state := persistedInventoryReadSourceState(t, stateOwner)
	path := writePersistedInventoryFileForCandidate(t, state)
	source := newPersistedInventoryFileReadSource(path, 200_000)
	handler := authenticator.Handler(newDeviceInventoryReadCandidateRoutes(&deviceInventoryReadCandidateConfig{
		Enabled: true, Source: source,
	}))
	response := requestConversationAPI(t, handler, identity, http.MethodGet, deviceInventoryReadCandidatePath,
		deviceInventoryReadCandidateScope, "", "", "")
	if response.Code != http.StatusOK {
		t.Fatalf("file candidate status=%d body=%q", response.Code, response.Body.String())
	}
	var value deviceplacement.SessionDeviceObservationInventory
	if err := json.Unmarshal(response.Body.Bytes(), &value); err != nil {
		t.Fatalf("decode file observation: %v body=%q", err, response.Body.String())
	}
	if err := deviceplacement.ValidateSessionDeviceObservationInventory(value); err != nil {
		t.Fatalf("validate file observation: %v", err)
	}
	if value.Owner.Subject != "account-42" || len(value.Devices) != 1 ||
		value.Devices[0].Device.DeviceID != "device-a" || value.Devices[0].InstanceID != "runner-a" ||
		value.ExecutionAuthorized || value.ReservationCreated || value.DispatchPerformed {
		t.Fatalf("file observation=%#v", value)
	}

	v2Handler := authenticator.Handler(newDeviceInventoryReadCandidateV2Routes(&deviceInventoryReadCandidateV2Config{
		Enabled: true, Source: source,
	}))
	v2Response := requestConversationAPI(t, v2Handler, identity, http.MethodGet, deviceInventoryReadCandidateV2Path,
		deviceInventoryReadCandidateScope, "", "", "")
	if v2Response.Code != http.StatusOK {
		t.Fatalf("file v2 candidate status=%d body=%q", v2Response.Code, v2Response.Body.String())
	}
	var v2 deviceplacement.SessionDeviceObservationInventoryV2
	if err := json.Unmarshal(v2Response.Body.Bytes(), &v2); err != nil {
		t.Fatalf("decode file v2 observation: %v body=%q", err, v2Response.Body.String())
	}
	if err := deviceplacement.ValidateSessionDeviceObservationInventoryV2(v2); err != nil {
		t.Fatalf("validate file v2 observation: %v", err)
	}
	if len(v2.Devices) != 1 || v2.Devices[0].Revision != state.Revision ||
		v2.Devices[0].HeartbeatSequence != state.Runner.HeartbeatSequence ||
		v2.ExecutionAuthorized || v2.ReservationCreated || v2.DispatchPerformed {
		t.Fatalf("file v2 observation=%#v", v2)
	}
}

func TestPersistedInventoryFileReadSourceRejectsAuthenticatedForeignOwner(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	path := writePersistedInventoryFileForCandidate(t, persistedInventoryReadSourceState(t, deviceinventory.SnapshotOwner{
		Issuer: identity.issuer, Subject: "account-foreign", TenantID: "tenant-slate",
	}))
	handler := authenticator.Handler(newDeviceInventoryReadCandidateRoutes(&deviceInventoryReadCandidateConfig{
		Enabled: true, Source: newPersistedInventoryFileReadSource(path, 200_000),
	}))
	response := requestConversationAPI(t, handler, identity, http.MethodGet, deviceInventoryReadCandidatePath,
		deviceInventoryReadCandidateScope, "", "", "")
	if response.Code != http.StatusBadGateway || !strings.Contains(response.Body.String(), `"code":"conversation_service_error"`) {
		t.Fatalf("foreign file candidate status=%d body=%q", response.Code, response.Body.String())
	}
}

func TestPersistedInventoryFileSetReadSourceProjectsAllInstancesThroughV1AndV2(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	owner := deviceinventory.SnapshotOwner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	first := persistedInventoryReadSourceState(t, owner)
	second := persistedInventoryReadSourceState(t, owner)
	second.Device.DeviceID = "device-b"
	second.Runner.DeviceID = "device-b"
	second.Runner.InstanceID = "runner-b"
	path := writePersistedInventoryFileSetForCandidate(t, owner, []deviceinventory.PersistedInventoryState{second, first})
	source := newPersistedInventoryFileSetReadSource(path, 200_000)

	v1Handler := authenticator.Handler(newDeviceInventoryReadCandidateRoutes(&deviceInventoryReadCandidateConfig{
		Enabled: true, Source: source,
	}))
	v1Response := requestConversationAPI(t, v1Handler, identity, http.MethodGet, deviceInventoryReadCandidatePath,
		deviceInventoryReadCandidateScope, "", "", "")
	if v1Response.Code != http.StatusOK {
		t.Fatalf("file-set v1 status=%d body=%q", v1Response.Code, v1Response.Body.String())
	}
	var v1 deviceplacement.SessionDeviceObservationInventory
	if err := json.Unmarshal(v1Response.Body.Bytes(), &v1); err != nil {
		t.Fatalf("decode file-set v1 observation: %v", err)
	}
	if err := deviceplacement.ValidateSessionDeviceObservationInventory(v1); err != nil {
		t.Fatalf("validate file-set v1 observation: %v", err)
	}
	if v1.Owner.Subject != owner.Subject || len(v1.Devices) != 2 ||
		v1.Devices[0].Device.DeviceID != "device-a" || v1.Devices[0].InstanceID != "runner-a" ||
		v1.Devices[1].Device.DeviceID != "device-b" || v1.Devices[1].InstanceID != "runner-b" ||
		v1.ExecutionAuthorized || v1.ReservationCreated || v1.DispatchPerformed {
		t.Fatalf("file-set v1 observation=%#v", v1)
	}

	v2Handler := authenticator.Handler(newDeviceInventoryReadCandidateV2Routes(&deviceInventoryReadCandidateV2Config{
		Enabled: true, Source: source,
	}))
	v2Response := requestConversationAPI(t, v2Handler, identity, http.MethodGet, deviceInventoryReadCandidateV2Path,
		deviceInventoryReadCandidateScope, "", "", "")
	if v2Response.Code != http.StatusOK {
		t.Fatalf("file-set v2 status=%d body=%q", v2Response.Code, v2Response.Body.String())
	}
	var v2 deviceplacement.SessionDeviceObservationInventoryV2
	if err := json.Unmarshal(v2Response.Body.Bytes(), &v2); err != nil {
		t.Fatalf("decode file-set v2 observation: %v", err)
	}
	if err := deviceplacement.ValidateSessionDeviceObservationInventoryV2(v2); err != nil {
		t.Fatalf("validate file-set v2 observation: %v", err)
	}
	if v2.Owner.Subject != owner.Subject || len(v2.Devices) != 2 ||
		v2.Devices[0].Device.DeviceID != "device-a" || v2.Devices[1].Device.DeviceID != "device-b" ||
		v2.ExecutionAuthorized || v2.ReservationCreated || v2.DispatchPerformed {
		t.Fatalf("file-set v2 observation=%#v", v2)
	}
}

func TestPersistedInventoryFileSetReadSourceRefreshesAfterAtomicReplacement(t *testing.T) {
	owner := deviceinventory.SnapshotOwner{Issuer: "issuer", Subject: "account-42", TenantID: "tenant-slate"}
	first := persistedInventoryReadSourceState(t, owner)
	second := persistedInventoryReadSourceState(t, owner)
	second.Device.DeviceID = "device-b"
	second.Runner.DeviceID = "device-b"
	second.Runner.InstanceID = "runner-b"
	path := writePersistedInventoryFileSetForCandidate(t, owner, []deviceinventory.PersistedInventoryState{first, second})
	source := newPersistedInventoryFileSetReadSource(path, 200_000)
	initial, err := source.ReadOwnedDeviceInventoryV2(context.Background(), model.Owner{
		Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
	})
	if err != nil {
		t.Fatalf("initial file-set read: %v", err)
	}
	if len(initial.Devices) != 2 || initial.Devices[0].HeartbeatSequence != 1 {
		t.Fatalf("initial observation=%#v", initial)
	}

	nextRunner := first.Runner
	nextRunner.HeartbeatSequence = 2
	nextRunner.ServerObservedAtMS = 101_000
	nextRunner.CapabilityLeaseExpiresAtMS = 201_000
	nextFirst, err := deviceinventory.CommitPersistedInventory(&first, first.Revision, first.Device, nextRunner)
	if err != nil {
		t.Fatalf("commit replacement: %v", err)
	}
	if err := writePersistedInventoryFileSetForCandidateAtPath(path, owner, []deviceinventory.PersistedInventoryState{second, nextFirst}); err != nil {
		t.Fatalf("replace file-set image: %v", err)
	}
	updated, err := source.ReadOwnedDeviceInventoryV2(context.Background(), model.Owner{
		Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
	})
	if err != nil {
		t.Fatalf("updated file-set read: %v", err)
	}
	if len(updated.Devices) != 2 || updated.Devices[0].Revision != nextFirst.Revision ||
		updated.Devices[0].HeartbeatSequence != 2 || updated.Devices[1].HeartbeatSequence != 1 ||
		updated.ExecutionAuthorized || updated.ReservationCreated || updated.DispatchPerformed {
		t.Fatalf("updated observation=%#v", updated)
	}
	if _, err := source.ReadOwnedDeviceInventoryV2(context.Background(), model.Owner{
		Issuer: owner.Issuer, Subject: "foreign", TenantID: owner.TenantID,
	}); err != deviceinventory.ErrInventoryOwnerMismatch {
		t.Fatalf("foreign owner error=%v, want %v", err, deviceinventory.ErrInventoryOwnerMismatch)
	}
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if _, err := source.ReadOwnedDeviceInventoryV2(ctx, model.Owner{
		Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
	}); err != context.Canceled {
		t.Fatalf("canceled context error=%v, want %v", err, context.Canceled)
	}
}

func writePersistedInventoryFileForCandidate(t *testing.T, state deviceinventory.PersistedInventoryState) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "inventory.json")
	envelope := struct {
		SchemaVersion string                                  `json:"schema_version"`
		State         deviceinventory.PersistedInventoryState `json:"state"`
	}{
		SchemaVersion: "forge.device-inventory-file/v1",
		State:         state,
	}
	data, err := json.Marshal(envelope)
	if err != nil {
		t.Fatal(err)
	}
	if err := statefs.AtomicWrite(path, append(data, '\n'), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(path); err != nil {
		t.Fatal(err)
	}
	return path
}

func writePersistedInventoryFileSetForCandidate(t *testing.T, owner deviceinventory.SnapshotOwner, states []deviceinventory.PersistedInventoryState) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "inventory-set.json")
	if err := writePersistedInventoryFileSetForCandidateAtPath(path, owner, states); err != nil {
		t.Fatal(err)
	}
	return path
}

func writePersistedInventoryFileSetForCandidateAtPath(path string, owner deviceinventory.SnapshotOwner, states []deviceinventory.PersistedInventoryState) error {
	envelope := struct {
		SchemaVersion string                                    `json:"schema_version"`
		Owner         deviceinventory.SnapshotOwner             `json:"owner"`
		States        []deviceinventory.PersistedInventoryState `json:"states"`
	}{
		SchemaVersion: "forge.device-inventory-file-set/v1",
		Owner:         owner,
		States:        states,
	}
	data, err := json.Marshal(envelope)
	if err != nil {
		return err
	}
	return statefs.AtomicWrite(path, append(data, '\n'), 0o600)
}

func persistedInventoryReadSourceState(t *testing.T, owner deviceinventory.SnapshotOwner) deviceinventory.PersistedInventoryState {
	t.Helper()
	capabilities, err := deviceheartbeat.NewCapabilitySnapshot(
		"linux", "amd64", 8, 7, 16<<30, 8<<30, 100<<30, 50<<30,
		nil, []string{"oci"},
	)
	if err != nil {
		t.Fatal(err)
	}
	return deviceinventory.PersistedInventoryState{
		Revision: 1,
		Device: deviceinventory.DeviceRecord{
			DeviceID: "device-a", Owner: owner, ApprovalState: "approved", CordonState: "clear", ReservationState: "none",
		},
		Runner: deviceinventory.RunnerInstanceRecord{
			DeviceID: "device-a", InstanceID: "runner-a", Generation: 1, HeartbeatSequence: 1,
			ServerObservedAtMS: 100_000, CapabilityLeaseExpiresAtMS: 200_000, Liveness: "online", Capabilities: capabilities,
		},
	}
}
