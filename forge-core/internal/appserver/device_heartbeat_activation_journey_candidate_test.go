package appserver

import (
	"context"
	"encoding/json"
	"net/http"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
	"forgeos/forge-core/internal/statefs"
)

// TestLifecycleHeartbeatCandidateFeedsAcceptedInventoryAndResourceViews keeps
// the pre-activation heartbeat candidate and the accepted read-only assembly
// in one journey. The candidate is mounted on a test handler only; the
// accepted handler is reconstructed from the persisted image, which also
// exercises the restart boundary without opening production enrollment.
func TestLifecycleHeartbeatCandidateFeedsAcceptedInventoryAndResourceViews(t *testing.T) {
	identity, authenticator := newMultiPrincipalConversationTestIdentity(t)
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	registryPath := filepath.Join(t.TempDir(), "lifecycle-registry.json")
	clientPath := writeClientInstanceSessionViewSourceFile(t, owner)
	if err := statefs.EnsurePrivateDir(filepath.Dir(registryPath)); err != nil {
		t.Fatal(err)
	}
	candidate := authenticator.Handler(newLifecycleRegistryCandidateRoutes(&lifecycleRegistryCandidateConfig{
		Enabled: true,
		Store:   newPersistedLifecycleRegistryCandidateStore(registryPath),
		Heartbeat: &lifecycleHeartbeatCandidateConfig{
			Enabled:            true,
			AllowUnsignedProof: true,
			Now:                func(context.Context) (uint64, error) { return 120_000, nil },
			StaleAfterMS:       90_000,
		},
	}))

	post := requestConversationAPI(t, candidate, identity, http.MethodPost,
		lifecycleHeartbeatCandidatePath, lifecycleHeartbeatCandidateScope,
		"application/json", "", lifecycleHeartbeatCandidateBody(t, identity.issuer, owner.Subject, owner.TenantID, 1, 1, 0))
	if post.Code != http.StatusOK {
		t.Fatalf("heartbeat candidate status=%d body=%q", post.Code, post.Body.String())
	}
	var published lifecycleHeartbeatCandidateResponse
	if err := json.Unmarshal(post.Body.Bytes(), &published); err != nil {
		t.Fatalf("decode heartbeat candidate: %v", err)
	}
	if published.Owner != owner || published.DeviceID != "device-a" || published.Revision != 1 ||
		published.Heartbeat.Instance.HeartbeatSequence != 1 || !published.PreviewOnly || !published.CandidatePublished ||
		published.Authority != (deviceinventory.LifecycleAuthority{}) {
		t.Fatalf("unexpected published candidate=%#v", published)
	}

	// A second route assembly reads the exact image written above. This is the
	// accepted inventory/read projection boundary, not a heartbeat mount.
	sessions, err := newAuthenticatedSessionRoutesWithDeviceFabricActivation(
		nil, nil, ptrDeviceFabricRequest(acceptedInventoryActivation()), registryPath, clientPath,
	)
	if err != nil {
		t.Fatalf("accepted activation assembly: %v", err)
	}
	accepted := authenticator.Handler(sessions)

	response := requestConversationAPI(t, accepted, identity, http.MethodGet,
		deviceInventoryReadCandidateV2Path, deviceInventoryReadCandidateScope, "", "", "")
	if response.Code != http.StatusOK {
		t.Fatalf("accepted inventory v2 status=%d body=%q", response.Code, response.Body.String())
	}
	var inventory deviceplacement.SessionDeviceObservationInventoryV2
	if err := json.Unmarshal(response.Body.Bytes(), &inventory); err != nil {
		t.Fatalf("decode accepted inventory v2: %v", err)
	}
	if err := deviceplacement.ValidateSessionDeviceObservationInventoryV2(inventory); err != nil {
		t.Fatalf("validate accepted inventory v2: %v", err)
	}
	if inventory.Owner.Subject != owner.Subject || len(inventory.Devices) != 1 ||
		inventory.Devices[0].Device.DeviceID != "device-a" || inventory.Devices[0].HeartbeatSequence != 1 ||
		inventory.ExecutionAuthorized || inventory.ReservationCreated || inventory.DispatchPerformed {
		t.Fatalf("accepted inventory v2=%#v", inventory)
	}

	response = requestConversationAPI(t, accepted, identity, http.MethodGet,
		clientInstanceSessionViewCandidatePath, clientInstanceSessionViewCandidateScope, "", "", "")
	if response.Code != http.StatusOK {
		t.Fatalf("accepted client-instance session view status=%d body=%q", response.Code, response.Body.String())
	}
	var sessionView deviceplacement.ClientInstanceSessionViewObservation
	if err := json.Unmarshal(response.Body.Bytes(), &sessionView); err != nil {
		t.Fatalf("decode accepted client-instance session view: %v", err)
	}
	if err := sessionView.Validate(); err != nil || sessionView.Owner.Subject != owner.Subject || !sessionView.ReadOnly {
		t.Fatalf("accepted client-instance session view=%#v err=%v", sessionView, err)
	}

	response = requestConversationAPI(t, accepted, identity, http.MethodGet,
		clientInstanceResourceViewCandidatePath, clientInstanceResourceViewCandidateScope, "", "", "")
	if response.Code != http.StatusOK {
		t.Fatalf("accepted client-instance resource view status=%d body=%q", response.Code, response.Body.String())
	}
	var resourceView deviceplacement.ClientInstanceResourceViewObservation
	if err := json.Unmarshal(response.Body.Bytes(), &resourceView); err != nil {
		t.Fatalf("decode accepted client-instance resource view: %v", err)
	}
	if err := resourceView.Validate(); err != nil || resourceView.Owner.Subject != owner.Subject ||
		len(resourceView.Devices) != 1 || resourceView.Devices[0].DeviceID != "device-a" ||
		!resourceView.ReadOnly || resourceView.Authority.ExecutionAuthorized ||
		resourceView.Authority.ReservationCreated || resourceView.Authority.DispatchPerformed {
		t.Fatalf("accepted client-instance resource view=%#v err=%v", resourceView, err)
	}

	// The same persisted image remains private to the authenticated owner.
	foreign := requestConversationAPIAs(t, accepted, identity, http.MethodGet,
		deviceInventoryReadCandidateV2Path, deviceInventoryReadCandidateScope,
		"account-foreign", owner.TenantID, "", "", "")
	if foreign.Code != http.StatusBadGateway || !strings.Contains(foreign.Body.String(), `"code":"conversation_service_error"`) ||
		strings.Contains(foreign.Body.String(), "device-a") {
		t.Fatalf("foreign inventory read status=%d body=%q", foreign.Code, foreign.Body.String())
	}

	// Replaying the same heartbeat against the persisted revision is rejected
	// before the candidate can replace the image.
	replay := requestConversationAPI(t, candidate, identity, http.MethodPost,
		lifecycleHeartbeatCandidatePath, lifecycleHeartbeatCandidateScope,
		"application/json", "", lifecycleHeartbeatCandidateBody(t, identity.issuer, owner.Subject, owner.TenantID, 1, 1, 1))
	if replay.Code != http.StatusConflict || !strings.Contains(replay.Body.String(), `"code":"heartbeat_conflict"`) {
		t.Fatalf("heartbeat replay status=%d body=%q", replay.Code, replay.Body.String())
	}

	// Keep this explicit so the test documents that the accepted route is a
	// read-only projection surface and does not expose candidate writes.
	write := requestConversationAPI(t, accepted, identity, http.MethodPost,
		lifecycleHeartbeatCandidatePath, lifecycleHeartbeatCandidateScope,
		"application/json", "", lifecycleHeartbeatCandidateBody(t, identity.issuer, owner.Subject, owner.TenantID, 1, 2, 1))
	if write.Code != http.StatusNotFound {
		t.Fatalf("accepted heartbeat route status=%d body=%q", write.Code, write.Body.String())
	}

}
