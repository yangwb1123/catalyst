package appserver

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"testing"

	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
	runmodel "forgeos/forge-core/internal/runtimebridge/runmodel"
)

// nativeMobileClientInstanceObservationSource is a test-only owner-bound
// source for the host-side Android/iOS acceptance journey. It deliberately
// exposes only a display projection: the selected mobile row contains the
// one Conversation under test and the resource view carries the same rows.
// No registration, heartbeat, reservation, scheduling, or execution state is
// created by this source.
type nativeMobileClientInstanceObservationSource struct {
	conversationID *string
	inventoryCalls int
	inventoryOwner model.Owner
	scheduler      *nativeMobileSchedulerPreviewSource
}

// nativeMobileSchedulerPreviewSource is a separate, test-only planning image
// for the scheduler handoff. The ordinary inventory observation above keeps
// the reserved declaration visible to the native resource screen; this image
// deliberately uses reservation_state=none and a fresh lease so the preview
// cannot accidentally treat a display reservation as scheduling authority.
// It never writes a registry, creates a reservation, issues a lease, or
// dispatches a Runner.
type nativeMobileSchedulerPreviewSource struct {
	inventoryCalls int
	inventoryOwner model.Owner
	policyCalls    int
	policyOwner    model.Owner
}

func (source *nativeMobileSchedulerPreviewSource) ReadOwnedDeviceInventoryV2(
	ctx context.Context,
	owner model.Owner,
) (deviceplacement.SessionDeviceObservationInventoryV2, error) {
	if ctx == nil {
		return deviceplacement.SessionDeviceObservationInventoryV2{}, fmt.Errorf("native mobile scheduler inventory requires a context")
	}
	if err := ctx.Err(); err != nil {
		return deviceplacement.SessionDeviceObservationInventoryV2{}, err
	}
	source.inventoryCalls++
	source.inventoryOwner = owner
	declaredOwner := deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}
	value := fixtureDeviceInventoryReadV2Value(declaredOwner)
	value.Devices[0].Device.ReservationState = "none"
	value.Devices[0].Device.SnapshotObservedAtMS = 250_000
	value.Devices[0].Device.LeaseExpiresAtMS = 600_000
	if value.Devices[0].Device.ReservationState != "none" ||
		value.Devices[0].Device.SnapshotObservedAtMS <= 0 ||
		value.Devices[0].Device.LeaseExpiresAtMS <= 300_000 {
		return deviceplacement.SessionDeviceObservationInventoryV2{}, fmt.Errorf("native mobile scheduler fixture is not fresh and unreserved")
	}
	if err := deviceplacement.ValidateSessionDeviceObservationInventoryV2(value); err != nil {
		return deviceplacement.SessionDeviceObservationInventoryV2{}, err
	}
	return value, nil
}

func (source *nativeMobileSchedulerPreviewSource) ReadOwnedDevicePlacementPolicy(
	ctx context.Context,
	owner model.Owner,
) (deviceplacement.PlacementPolicyRegistry, error) {
	if ctx == nil {
		return deviceplacement.PlacementPolicyRegistry{}, fmt.Errorf("native mobile scheduler policy requires a context")
	}
	if err := ctx.Err(); err != nil {
		return deviceplacement.PlacementPolicyRegistry{}, err
	}
	source.policyCalls++
	source.policyOwner = owner
	declaredOwner := deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}
	return deviceplacement.PlacementPolicyRegistry{
		SchemaVersion:  deviceplacement.PlacementPolicyRegistrySchemaVersion,
		EvaluationMode: deviceplacement.PlacementPolicyRegistryEvaluationMode,
		Owner:          declaredOwner,
		Policies: []deviceplacement.PlacementPolicy{{
			DeviceID: "device-a", InstanceID: "runner-a", Revision: 1,
			Generation: 1, HeartbeatSequence: 1,
			DataResidencyZones: []string{"us-west"}, TrustZone: "untrusted",
			SandboxLevels: []string{"process"}, ConcurrencyLimit: 1,
			ActiveConcurrency: 0,
		}},
	}, nil
}

func (source *nativeMobileClientInstanceObservationSource) schedulerPreviewSource() *nativeMobileSchedulerPreviewSource {
	if source.scheduler == nil {
		source.scheduler = &nativeMobileSchedulerPreviewSource{}
	}
	return source.scheduler
}

func (source *nativeMobileClientInstanceObservationSource) views(
	owner model.Owner,
) (deviceplacement.ClientInstanceSessionViewObservation, deviceplacement.ClientInstanceResourceViewObservation, error) {
	if source == nil || source.conversationID == nil || *source.conversationID == "" {
		return deviceplacement.ClientInstanceSessionViewObservation{}, deviceplacement.ClientInstanceResourceViewObservation{}, fmt.Errorf("native mobile Conversation is not initialized")
	}
	instances, err := deviceplacement.ObserveClientInstanceSessionView(
		deviceplacement.ClientInstanceSessionViewRequest{
			Owner: deviceplacement.Owner{
				Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
			},
			Instances: []deviceplacement.ClientInstanceSessionViewInstance{
				{
					InstanceID:   "client-mobile",
					ClientKind:   deviceplacement.ClientKindMobile,
					SessionIDs:   []string{*source.conversationID},
					ObservedAtMS: 200_500,
					Status:       "active",
				},
				{
					InstanceID:   "client-web",
					ClientKind:   deviceplacement.ClientKindWeb,
					SessionIDs:   []string{"conversation-hidden"},
					ObservedAtMS: 200_500,
					Status:       "idle",
				},
			},
		},
	)
	if err != nil {
		return deviceplacement.ClientInstanceSessionViewObservation{}, deviceplacement.ClientInstanceResourceViewObservation{}, err
	}
	resource := fixtureClientInstanceResourceView(deviceplacement.Owner{
		Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
	})
	resource.Instances = append([]deviceplacement.ClientInstanceSessionViewInstance(nil), instances.Instances...)
	if err := resource.Validate(); err != nil {
		return deviceplacement.ClientInstanceSessionViewObservation{}, deviceplacement.ClientInstanceResourceViewObservation{}, err
	}
	return instances, resource, nil
}

func (source *nativeMobileClientInstanceObservationSource) ReadOwnedClientInstanceSessionView(
	ctx context.Context,
	owner model.Owner,
) (deviceplacement.ClientInstanceSessionViewObservation, error) {
	if ctx == nil {
		return deviceplacement.ClientInstanceSessionViewObservation{}, fmt.Errorf("native mobile session view requires a context")
	}
	if err := ctx.Err(); err != nil {
		return deviceplacement.ClientInstanceSessionViewObservation{}, err
	}
	session, _, err := source.views(owner)
	return session, err
}

func (source *nativeMobileClientInstanceObservationSource) ReadOwnedClientInstanceResourceView(
	ctx context.Context,
	owner model.Owner,
) (deviceplacement.ClientInstanceResourceViewObservation, error) {
	if ctx == nil {
		return deviceplacement.ClientInstanceResourceViewObservation{}, fmt.Errorf("native mobile resource view requires a context")
	}
	if err := ctx.Err(); err != nil {
		return deviceplacement.ClientInstanceResourceViewObservation{}, err
	}
	_, resource, err := source.views(owner)
	return resource, err
}

// ReadOwnedDeviceInventoryV2 supplies the same bounded, lossless observation
// consumed by the explicit native cold-start candidate. The fixture is
// owner-derived from the verified bearer and remains display-only: it does
// not enroll or heartbeat a device, select capacity, reserve a target, or
// authorize execution.
func (source *nativeMobileClientInstanceObservationSource) ReadOwnedDeviceInventoryV2(
	ctx context.Context,
	owner model.Owner,
) (deviceplacement.SessionDeviceObservationInventoryV2, error) {
	if ctx == nil {
		return deviceplacement.SessionDeviceObservationInventoryV2{}, fmt.Errorf("native mobile inventory requires a context")
	}
	if err := ctx.Err(); err != nil {
		return deviceplacement.SessionDeviceObservationInventoryV2{}, err
	}
	source.inventoryCalls++
	source.inventoryOwner = owner
	value := fixtureDeviceInventoryReadV2Value(deviceplacement.Owner{
		Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
	})
	if err := deviceplacement.ValidateSessionDeviceObservationInventoryV2(value); err != nil {
		return deviceplacement.SessionDeviceObservationInventoryV2{}, err
	}
	return value, nil
}

// nativeMobileClientInstanceObservationRoutes wraps the ordinary test
// session surface and intercepts only the two explicit display candidates.
// Keeping this wrapper in a *_test.go file ensures the production constructor
// and Run server remain default-off.
type nativeMobileClientInstanceObservationRoutes struct {
	fallback  http.Handler
	session   http.Handler
	resource  http.Handler
	inventory http.Handler
	scheduler http.Handler
}

func (routes nativeMobileClientInstanceObservationRoutes) ServeHTTP(
	w http.ResponseWriter,
	r *http.Request,
) {
	switch r.URL.EscapedPath() {
	case clientInstanceSessionViewCandidatePath:
		routes.session.ServeHTTP(w, r)
	case clientInstanceResourceViewCandidatePath:
		routes.resource.ServeHTTP(w, r)
	case deviceInventoryReadCandidateV2Path:
		routes.inventory.ServeHTTP(w, r)
	case schedulerSelectionPreviewPath:
		routes.scheduler.ServeHTTP(w, r)
	default:
		routes.fallback.ServeHTTP(w, r)
	}
}

func newNativeMobileClientInstanceObservationRoutes(
	fallback http.Handler,
	source *nativeMobileClientInstanceObservationSource,
	schedulerBackend conversationBackend,
) http.Handler {
	return nativeMobileClientInstanceObservationRoutes{
		fallback: fallback,
		session: newClientInstanceSessionViewCandidateRoutes(&clientInstanceSessionViewCandidateConfig{
			Enabled: true,
			Source:  source,
		}),
		resource: newClientInstanceResourceViewCandidateRoutes(&clientInstanceResourceViewCandidateConfig{
			Enabled: true,
			Source:  source,
		}),
		inventory: newDeviceInventoryReadCandidateV2Routes(&deviceInventoryReadCandidateV2Config{
			Enabled: true,
			Source:  source,
		}),
		scheduler: newSchedulerSelectionPreviewRoutes(&schedulerSelectionPreviewConfig{
			Enabled:      true,
			Source:       source.schedulerPreviewSource(),
			PolicySource: source.schedulerPreviewSource(),
			Backend:      schedulerBackend,
			Now:          func(context.Context) (int64, error) { return 300_000, nil },
		}),
	}
}

func TestNativeMobileClientInstanceObservationRoutesServeOwnerBoundPair(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	conversationID := "conversation-native"
	source := &nativeMobileClientInstanceObservationSource{conversationID: &conversationID}
	schedulerBackend := &fakeConversationBackend{runPage: runmodel.OwnedRunPage{
		ConversationID: conversationID,
		Runs: []runmodel.OwnedRunSummary{{
			RunID: "native-scheduler-run", PromptID: "native-scheduler-prompt",
			CreatedAtMS: 1, LatestSequence: 1, Status: "completed",
		}},
	}}
	routes := authenticator.Handler(newNativeMobileClientInstanceObservationRoutes(
		newAuthenticatedSessionRoutesWithObservationCandidates(nil, nil), source, schedulerBackend,
	))

	sessionResponse := requestConversationAPI(t, routes, identity, http.MethodGet,
		clientInstanceSessionViewCandidatePath, clientInstanceSessionViewCandidateScope, "", "", "")
	if sessionResponse.Code != http.StatusOK {
		t.Fatalf("native session view status=%d body=%q", sessionResponse.Code, sessionResponse.Body.String())
	}
	var session deviceplacement.ClientInstanceSessionViewObservation
	if err := json.Unmarshal(sessionResponse.Body.Bytes(), &session); err != nil {
		t.Fatalf("decode native session view: %v", err)
	}
	if err := session.Validate(); err != nil {
		t.Fatalf("validate native session view: %v", err)
	}

	resourceResponse := requestConversationAPI(t, routes, identity, http.MethodGet,
		clientInstanceResourceViewCandidatePath, clientInstanceResourceViewCandidateScope, "", "", "")
	if resourceResponse.Code != http.StatusOK {
		t.Fatalf("native resource view status=%d body=%q", resourceResponse.Code, resourceResponse.Body.String())
	}
	var resource deviceplacement.ClientInstanceResourceViewObservation
	if err := json.Unmarshal(resourceResponse.Body.Bytes(), &resource); err != nil {
		t.Fatalf("decode native resource view: %v", err)
	}
	if err := resource.Validate(); err != nil {
		t.Fatalf("validate native resource view: %v", err)
	}
	if session.Owner != resource.Owner || len(session.Instances) != 2 ||
		session.Instances[0].InstanceID != "client-mobile" ||
		!containsString(session.Instances[0].SessionIDs, conversationID) ||
		session.Instances[0].ClientKind != deviceplacement.ClientKindMobile ||
		len(resource.Devices) != 1 || !resource.ReadOnly ||
		resource.Authority != (deviceplacement.ClientInstanceResourceViewAuthority{}) {
		t.Fatalf("native pair did not converge: session=%#v resource=%#v", session, resource)
	}

	inventoryResponse := requestConversationAPI(t, routes, identity, http.MethodGet,
		deviceInventoryReadCandidateV2Path, deviceInventoryReadCandidateScope, "", "", "")
	if inventoryResponse.Code != http.StatusOK {
		t.Fatalf("native inventory v2 status=%d body=%q", inventoryResponse.Code, inventoryResponse.Body.String())
	}
	var inventory deviceplacement.SessionDeviceObservationInventoryV2
	if err := json.Unmarshal(inventoryResponse.Body.Bytes(), &inventory); err != nil {
		t.Fatalf("decode native inventory v2: %v", err)
	}
	if err := deviceplacement.ValidateSessionDeviceObservationInventoryV2(inventory); err != nil {
		t.Fatalf("validate native inventory v2: %v", err)
	}
	wantOwner := deviceplacement.Owner{
		Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate",
	}
	if source.inventoryCalls != 1 || source.inventoryOwner != (model.Owner{
		Issuer: wantOwner.Issuer, Subject: wantOwner.Subject, TenantID: wantOwner.TenantID,
	}) || inventory.Owner != wantOwner || len(inventory.Devices) != 1 ||
		inventory.Devices[0].InstanceID != "runner-a" ||
		inventory.Devices[0].Device.DeviceID != "device-a" ||
		inventory.Devices[0].Device.Owner != wantOwner ||
		inventory.Devices[0].Device.ReservationState != "reserved" ||
		len(inventory.Devices[0].Device.GPUs) != 2 ||
		inventory.ExecutionAuthorized || inventory.ReservationCreated || inventory.DispatchPerformed {
		t.Fatalf("native inventory v2 owner/tuple/authority mismatch: source=%#v inventory=%#v", source, inventory)
	}

	previewBody := `{"conversation_id":"conversation-native","run_id":"native-scheduler-run","attempt_id":"native-scheduler-attempt","requirements":{"os":"linux","architecture":"amd64","min_cpu_cores":1,"min_memory_bytes":1,"min_storage_bytes":1,"runtime":"go","gpu":{"required":false,"min_memory_bytes":0,"runtime":""},"data_residency_zones":["us-west"],"minimum_trust_zone":"untrusted","sandbox_floor":"process","concurrency_slots":1}}`
	previewResponse := requestConversationAPI(t, routes, identity, http.MethodPost,
		schedulerSelectionPreviewPath, schedulerSelectionPreviewScope, "application/json", "", previewBody)
	if previewResponse.Code != http.StatusOK {
		t.Fatalf("native scheduler preview status=%d body=%q", previewResponse.Code, previewResponse.Body.String())
	}
	var preview deviceplacement.SchedulerSelectionPreviewObservation
	if err := json.Unmarshal(previewResponse.Body.Bytes(), &preview); err != nil {
		t.Fatalf("decode native scheduler preview: %v", err)
	}
	if err := preview.Validate(); err != nil {
		t.Fatalf("validate native scheduler preview: %v", err)
	}
	schedulerOwner := model.Owner{Issuer: wantOwner.Issuer, Subject: wantOwner.Subject, TenantID: wantOwner.TenantID}
	if source.scheduler == nil || source.scheduler.inventoryCalls != 1 || source.scheduler.policyCalls != 1 ||
		source.scheduler.inventoryOwner != schedulerOwner || source.scheduler.policyOwner != schedulerOwner ||
		preview.Owner != wantOwner || preview.SelectedDeviceID == nil || *preview.SelectedDeviceID != "device-a" ||
		preview.SelectedInstanceID == nil || *preview.SelectedInstanceID != "runner-a" ||
		!preview.PreviewOnly || preview.Authority != (deviceplacement.SchedulerSelectionPreviewAuthority{}) {
		t.Fatalf("native scheduler preview owner/tuple/authority mismatch: source=%#v preview=%#v", source.scheduler, preview)
	}
}

func containsString(values []string, target string) bool {
	for _, value := range values {
		if value == target {
			return true
		}
	}
	return false
}
