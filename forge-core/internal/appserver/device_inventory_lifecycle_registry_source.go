package appserver

import (
	"context"
	"fmt"
	"time"

	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// persistedLifecycleRegistryFileSetReadSource bridges the complete
// owner-scoped enrollment/heartbeat/inventory restart image to the existing
// device observation handlers. It is used by the accepted activation adapter
// as well as deterministic migration tests; reading the image does not make
// inventory authoritative and never accepts a heartbeat or grants execution
// authority.
type persistedLifecycleRegistryFileSetReadSource struct {
	path          string
	evaluatedAtMS uint64
}

func newPersistedLifecycleRegistryFileSetReadSource(
	path string,
	evaluatedAtMS uint64,
) persistedLifecycleRegistryFileSetReadSource {
	return persistedLifecycleRegistryFileSetReadSource{path: path, evaluatedAtMS: evaluatedAtMS}
}

// activatedPersistedLifecycleRegistryFileSetReadSource is the production
// observation adapter used only after the device-fabric activation gate has
// accepted its decisions and evidence. It samples the Coordinator clock for
// each read so liveness and lease expiry cannot remain frozen at startup.
// The underlying file adapter remains read-only and owner-bound.
type activatedPersistedLifecycleRegistryFileSetReadSource struct {
	path string
}

func newActivatedPersistedLifecycleRegistryFileSetReadSource(path string) activatedPersistedLifecycleRegistryFileSetReadSource {
	return activatedPersistedLifecycleRegistryFileSetReadSource{path: path}
}

func (source activatedPersistedLifecycleRegistryFileSetReadSource) readSource() (persistedLifecycleRegistryFileSetReadSource, error) {
	now := time.Now().UnixMilli()
	if now <= 0 || uint64(now) > uint64(deviceplacement.MaxSafeIntegerMS) {
		return persistedLifecycleRegistryFileSetReadSource{}, fmt.Errorf("server observation time is outside the JSON-safe range")
	}
	return newPersistedLifecycleRegistryFileSetReadSource(source.path, uint64(now)), nil
}

func (source activatedPersistedLifecycleRegistryFileSetReadSource) ReadOwnedDeviceInventory(
	ctx context.Context,
	owner model.Owner,
) (deviceplacement.SessionDeviceObservationInventory, error) {
	readSource, err := source.readSource()
	if err != nil {
		return deviceplacement.SessionDeviceObservationInventory{}, err
	}
	return readSource.ReadOwnedDeviceInventory(ctx, owner)
}

func (source activatedPersistedLifecycleRegistryFileSetReadSource) ReadOwnedDeviceInventoryV2(
	ctx context.Context,
	owner model.Owner,
) (deviceplacement.SessionDeviceObservationInventoryV2, error) {
	readSource, err := source.readSource()
	if err != nil {
		return deviceplacement.SessionDeviceObservationInventoryV2{}, err
	}
	return readSource.ReadOwnedDeviceInventoryV2(ctx, owner)
}

func (source activatedPersistedLifecycleRegistryFileSetReadSource) inventoryValues(
	ctx context.Context,
	owner model.Owner,
) ([]deviceinventory.PersistedInventoryState, error) {
	// Resource views preserve the observation and lease timestamps committed by
	// the lifecycle image. They do not evaluate freshness or invent a new
	// resource timestamp; only the inventory projection routes need a
	// Coordinator clock sample for liveness and expiry status.
	return newPersistedLifecycleRegistryFileSetReadSource(source.path, 0).inventoryValues(ctx, owner)
}

func (source persistedLifecycleRegistryFileSetReadSource) readStates(
	ctx context.Context,
	owner model.Owner,
) ([]deviceinventory.PersistedEnrollmentHeartbeatLifecycleState, error) {
	if ctx == nil {
		return nil, fmt.Errorf("persisted lifecycle registry source requires a context")
	}
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	adapter, err := deviceinventory.NewPersistedEnrollmentHeartbeatLifecycleFileSetReadAdapter(
		source.path,
		deviceidentity.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
	)
	if err != nil {
		return nil, err
	}
	return adapter.ReadStates()
}

func (source persistedLifecycleRegistryFileSetReadSource) inventoryValues(
	ctx context.Context,
	owner model.Owner,
) ([]deviceinventory.PersistedInventoryState, error) {
	states, err := source.readStates(ctx, owner)
	if err != nil {
		return nil, err
	}
	values := make([]deviceinventory.PersistedInventoryState, len(states))
	for index, state := range states {
		// ReadStates returns an owned deep copy. The observation builders validate
		// and clone again at their own boundary before serializing a response.
		values[index] = state.Inventory
	}
	return values, nil
}

func (source persistedLifecycleRegistryFileSetReadSource) ReadOwnedDeviceInventory(
	ctx context.Context,
	owner model.Owner,
) (deviceplacement.SessionDeviceObservationInventory, error) {
	values, err := source.inventoryValues(ctx, owner)
	if err != nil {
		return deviceplacement.SessionDeviceObservationInventory{}, err
	}
	return deviceplacement.BuildPersistedInventoryObservation(values, deviceinventory.SnapshotOwner{
		Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
	}, source.evaluatedAtMS)
}

func (source persistedLifecycleRegistryFileSetReadSource) ReadOwnedDeviceInventoryV2(
	ctx context.Context,
	owner model.Owner,
) (deviceplacement.SessionDeviceObservationInventoryV2, error) {
	values, err := source.inventoryValues(ctx, owner)
	if err != nil {
		return deviceplacement.SessionDeviceObservationInventoryV2{}, err
	}
	return deviceplacement.BuildPersistedInventoryObservationV2(values, deviceinventory.SnapshotOwner{
		Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
	}, source.evaluatedAtMS)
}
