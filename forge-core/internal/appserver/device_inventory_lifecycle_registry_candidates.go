package appserver

import (
	"context"
	"fmt"
	"net/http"

	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
	"forgeos/forge-core/internal/executionprofile"
	"forgeos/forge-core/internal/runtimebridge"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// newAuthenticatedSessionRoutesWithLifecycleRegistryInventoryCandidates
// composes the existing authenticated observation candidates with an
// owner-bound lifecycle-registry file source. It is intentionally a private
// test/migration constructor; production keeps using
// newAuthenticatedSessionRoutes and therefore never mounts either device
// inventory path.
func newAuthenticatedSessionRoutesWithLifecycleRegistryInventoryCandidates(
	client *runtimebridge.Client,
	profiles *executionprofile.Catalog,
	path string,
	evaluatedAtMS uint64,
) http.Handler {
	sessions := newAuthenticatedSessionRoutesWithObservationCandidates(client, profiles)
	source := newPersistedLifecycleRegistryFileSetReadSource(path, evaluatedAtMS)
	v1 := newDeviceInventoryReadCandidateRoutes(&deviceInventoryReadCandidateConfig{
		Enabled: true,
		Source:  source,
	})
	v2 := newDeviceInventoryReadCandidateV2Routes(&deviceInventoryReadCandidateV2Config{
		Enabled: true,
		Source:  source,
	})
	placement := newDevicePlacementRegistryCandidateRoutes(&devicePlacementRegistryCandidateConfig{
		Enabled: true,
		Source:  source,
		Now: func(context.Context) (int64, error) {
			return int64(evaluatedAtMS), nil
		},
	})
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.EscapedPath() {
		case deviceInventoryReadCandidatePath:
			v1.ServeHTTP(w, r)
		case deviceInventoryReadCandidateV2Path:
			v2.ServeHTTP(w, r)
		case devicePlacementRegistryCandidatePath:
			placement.ServeHTTP(w, r)
		default:
			sessions.ServeHTTP(w, r)
		}
	})
}

// newAuthenticatedSessionRoutesWithLifecycleRegistryClientInstanceCandidates
// composes the owner-bound lifecycle-registry inventory source with the
// display-only client-instance session/resource views.  It is intentionally a
// private test/migration constructor; production keeps using
// newAuthenticatedSessionRoutes and therefore leaves all three client-instance
// and device inventory paths unregistered.
//
// The instance rows are caller-declared metadata.  The persisted lifecycle
// registry is read through the same owner-bound source for both client-instance
// views, so a foreign or malformed image cannot be combined with a local
// resource projection.  No route in this constructor authenticates a device,
// selects a target, reserves capacity, schedules, dispatches, or executes work.
func newAuthenticatedSessionRoutesWithLifecycleRegistryClientInstanceCandidates(
	client *runtimebridge.Client,
	profiles *executionprofile.Catalog,
	path string,
	evaluatedAtMS uint64,
	instances []deviceplacement.ClientInstanceSessionViewInstance,
) http.Handler {
	sessions := newAuthenticatedSessionRoutesWithLifecycleRegistryInventoryCandidates(
		client, profiles, path, evaluatedAtMS,
	)
	registrySource := newPersistedLifecycleRegistryFileSetReadSource(path, evaluatedAtMS)
	inventoryReader := persistedLifecycleRegistryInventoryStateReader{source: registrySource}
	resourceSource := newPersistedInventoryClientInstanceResourceViewSource(
		inventoryReader,
		instances,
	)
	resource := newClientInstanceResourceViewCandidateRoutes(&clientInstanceResourceViewCandidateConfig{
		Enabled: true,
		Source:  resourceSource,
	})
	session := newClientInstanceSessionViewCandidateRoutes(&clientInstanceSessionViewCandidateConfig{
		Enabled: true,
		Source: persistedLifecycleRegistryClientInstanceSessionViewSource{
			reader:    inventoryReader,
			instances: cloneClientInstanceSessionViewInstances(instances),
		},
	})
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.EscapedPath() {
		case clientInstanceSessionViewCandidatePath:
			session.ServeHTTP(w, r)
		case clientInstanceResourceViewCandidatePath:
			resource.ServeHTTP(w, r)
		default:
			sessions.ServeHTTP(w, r)
		}
	})
}

// persistedLifecycleRegistryInventoryStateReader adapts the complete
// lifecycle image to the existing resource-view source interface while
// retaining the lifecycle source's owner and canonical-state checks.
type persistedLifecycleRegistryInventoryStateReader struct {
	source lifecycleRegistryInventoryStateSource
}

type lifecycleRegistryInventoryStateSource interface {
	inventoryValues(context.Context, model.Owner) ([]deviceinventory.PersistedInventoryState, error)
}

func (reader persistedLifecycleRegistryInventoryStateReader) readStates(
	ctx context.Context,
	owner model.Owner,
) ([]deviceinventory.PersistedInventoryState, error) {
	return reader.source.inventoryValues(ctx, owner)
}

// persistedLifecycleRegistryClientInstanceSessionViewSource makes the
// client-instance session declaration use the same owner-bound registry read
// boundary as the resource view.  The registry rows are not copied into the
// session response: they are only required to prove that the explicit
// display-only composition is reading the configured owner image.
type persistedLifecycleRegistryClientInstanceSessionViewSource struct {
	reader    clientInstanceResourceViewStateReader
	instances []deviceplacement.ClientInstanceSessionViewInstance
}

func (source persistedLifecycleRegistryClientInstanceSessionViewSource) ReadOwnedClientInstanceSessionView(
	ctx context.Context,
	owner model.Owner,
) (deviceplacement.ClientInstanceSessionViewObservation, error) {
	if ctx == nil {
		return deviceplacement.ClientInstanceSessionViewObservation{}, fmt.Errorf("lifecycle registry client-instance session source requires a context")
	}
	if err := ctx.Err(); err != nil {
		return deviceplacement.ClientInstanceSessionViewObservation{}, err
	}
	if source.reader == nil {
		return deviceplacement.ClientInstanceSessionViewObservation{}, fmt.Errorf("lifecycle registry client-instance session source is unavailable")
	}
	if _, err := source.reader.readStates(ctx, owner); err != nil {
		return deviceplacement.ClientInstanceSessionViewObservation{}, err
	}
	return deviceplacement.ObserveClientInstanceSessionView(deviceplacement.ClientInstanceSessionViewRequest{
		Owner: deviceplacement.Owner{
			Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
		},
		Instances: cloneClientInstanceSessionViewInstances(source.instances),
	})
}

func cloneClientInstanceSessionViewInstances(
	instances []deviceplacement.ClientInstanceSessionViewInstance,
) []deviceplacement.ClientInstanceSessionViewInstance {
	copyInstances := make([]deviceplacement.ClientInstanceSessionViewInstance, len(instances))
	for index, instance := range instances {
		copyInstances[index] = instance
		copyInstances[index].SessionIDs = append([]string(nil), instance.SessionIDs...)
	}
	return copyInstances
}
