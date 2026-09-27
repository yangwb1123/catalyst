package appserver

import (
	"context"
	"fmt"

	"forgeos/forge-core/internal/deviceheartbeat"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
)

// persistedInventoryReadSource is a test-injected bridge from already
// restored value-level inventory to the private read candidate. It is not
// wired into production routes and has no storage, listener, credential, or
// clock dependency.
type persistedInventoryReadSource struct {
	values        []deviceinventory.PersistedInventoryState
	evaluatedAtMS uint64
}

func newPersistedInventoryReadSource(
	values []deviceinventory.PersistedInventoryState,
	evaluatedAtMS uint64,
) deviceInventoryReadSource {
	copyValues := make([]deviceinventory.PersistedInventoryState, len(values))
	for index, value := range values {
		copyValues[index] = value
		copyValues[index].Runner.Capabilities.GPUs = append([]deviceheartbeat.GPUCapability(nil), value.Runner.Capabilities.GPUs...)
		copyValues[index].Runner.Capabilities.Runtimes = append([]string(nil), value.Runner.Capabilities.Runtimes...)
	}
	return persistedInventoryReadSource{values: copyValues, evaluatedAtMS: evaluatedAtMS}
}

// persistedInventoryReadV2Source is the lossless companion to
// persistedInventoryReadSource. Both adapters consume already restored state
// and remain test-injected; neither reads storage, listens for heartbeats, or
// grants inventory authority.
type persistedInventoryReadV2Source struct {
	values        []deviceinventory.PersistedInventoryState
	evaluatedAtMS uint64
}

// persistedInventoryFileReadSource is an explicitly injected bridge from the
// read-only filesystem adapter to the private candidate routes.  It is not
// constructed by production route wiring; its only purpose is to exercise the
// owner-bound restart boundary with the same HTTP projection used by fixture
// sources.  It never writes the file, accepts a heartbeat, or grants authority.
type persistedInventoryFileReadSource struct {
	path          string
	evaluatedAtMS uint64
}

// persistedInventoryFileSetReadSource is the aggregate companion to the
// single-state file source. It remains explicitly injected and read-only; the
// aggregate file is an atomically replaced observation image, not a heartbeat
// listener or authoritative device registry.
type persistedInventoryFileSetReadSource struct {
	path          string
	evaluatedAtMS uint64
}

// persistedPlacementPolicyFileSource is the explicit policy-complete join
// source used by scheduler selection preview and the fenced scheduler lease
// route. It is separate from the lifecycle inventory source because the
// latter intentionally contains unknown residency, trust, sandbox, and
// concurrency attributes.
type persistedPlacementPolicyFileSource struct {
	path string
}

func newPersistedInventoryFileReadSource(path string, evaluatedAtMS uint64) persistedInventoryFileReadSource {
	return persistedInventoryFileReadSource{path: path, evaluatedAtMS: evaluatedAtMS}
}

func newPersistedInventoryFileSetReadSource(path string, evaluatedAtMS uint64) persistedInventoryFileSetReadSource {
	return persistedInventoryFileSetReadSource{path: path, evaluatedAtMS: evaluatedAtMS}
}

func newPersistedPlacementPolicyFileSource(path string) persistedPlacementPolicyFileSource {
	return persistedPlacementPolicyFileSource{path: path}
}

func (source persistedPlacementPolicyFileSource) ReadOwnedDevicePlacementPolicy(
	ctx context.Context,
	owner model.Owner,
) (deviceplacement.PlacementPolicyRegistry, error) {
	if ctx == nil {
		return deviceplacement.PlacementPolicyRegistry{}, fmt.Errorf("placement policy source requires a context")
	}
	if err := ctx.Err(); err != nil {
		return deviceplacement.PlacementPolicyRegistry{}, err
	}
	adapter, err := deviceplacement.NewPlacementPolicyRegistryFileReadAdapter(source.path, deviceplacement.Owner{
		Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
	})
	if err != nil {
		return deviceplacement.PlacementPolicyRegistry{}, err
	}
	return adapter.Read()
}

func newPersistedInventoryReadV2Source(
	values []deviceinventory.PersistedInventoryState,
	evaluatedAtMS uint64,
) deviceInventoryReadV2Source {
	copyValues := make([]deviceinventory.PersistedInventoryState, len(values))
	for index, value := range values {
		copyValues[index] = value
		copyValues[index].Runner.Capabilities.GPUs = append([]deviceheartbeat.GPUCapability(nil), value.Runner.Capabilities.GPUs...)
		copyValues[index].Runner.Capabilities.Runtimes = append([]string(nil), value.Runner.Capabilities.Runtimes...)
	}
	return persistedInventoryReadV2Source{values: copyValues, evaluatedAtMS: evaluatedAtMS}
}

func (source persistedInventoryReadV2Source) ReadOwnedDeviceInventoryV2(
	ctx context.Context,
	owner model.Owner,
) (deviceplacement.SessionDeviceObservationInventoryV2, error) {
	if ctx == nil {
		return deviceplacement.SessionDeviceObservationInventoryV2{}, fmt.Errorf("persisted inventory v2 source requires a context")
	}
	if err := ctx.Err(); err != nil {
		return deviceplacement.SessionDeviceObservationInventoryV2{}, err
	}
	return deviceplacement.BuildPersistedInventoryObservationV2(source.values, deviceinventory.SnapshotOwner{
		Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
	}, source.evaluatedAtMS)
}

func (source persistedInventoryFileReadSource) readState(
	ctx context.Context,
	owner model.Owner,
) (deviceinventory.PersistedInventoryState, error) {
	if ctx == nil {
		return deviceinventory.PersistedInventoryState{}, fmt.Errorf("persisted inventory file source requires a context")
	}
	if err := ctx.Err(); err != nil {
		return deviceinventory.PersistedInventoryState{}, err
	}
	adapter, err := deviceinventory.NewPersistedInventoryFileReadAdapter(
		source.path,
		deviceinventory.SnapshotOwner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
		source.evaluatedAtMS,
		deviceinventory.DefaultStaleAfterMS,
	)
	if err != nil {
		return deviceinventory.PersistedInventoryState{}, err
	}
	return adapter.ReadState()
}

func (source persistedInventoryFileReadSource) ReadOwnedDeviceInventory(
	ctx context.Context,
	owner model.Owner,
) (deviceplacement.SessionDeviceObservationInventory, error) {
	state, err := source.readState(ctx, owner)
	if err != nil {
		return deviceplacement.SessionDeviceObservationInventory{}, err
	}
	return deviceplacement.BuildPersistedInventoryObservation([]deviceinventory.PersistedInventoryState{state}, deviceinventory.SnapshotOwner{
		Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
	}, source.evaluatedAtMS)
}

func (source persistedInventoryFileReadSource) ReadOwnedDeviceInventoryV2(
	ctx context.Context,
	owner model.Owner,
) (deviceplacement.SessionDeviceObservationInventoryV2, error) {
	state, err := source.readState(ctx, owner)
	if err != nil {
		return deviceplacement.SessionDeviceObservationInventoryV2{}, err
	}
	return deviceplacement.BuildPersistedInventoryObservationV2([]deviceinventory.PersistedInventoryState{state}, deviceinventory.SnapshotOwner{
		Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
	}, source.evaluatedAtMS)
}

func (source persistedInventoryFileSetReadSource) readStates(
	ctx context.Context,
	owner model.Owner,
) ([]deviceinventory.PersistedInventoryState, error) {
	if ctx == nil {
		return nil, fmt.Errorf("persisted inventory file-set source requires a context")
	}
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	adapter, err := deviceinventory.NewPersistedInventoryFileSetReadAdapter(
		source.path,
		deviceinventory.SnapshotOwner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
	)
	if err != nil {
		return nil, err
	}
	return adapter.ReadStates()
}

func (source persistedInventoryFileSetReadSource) ReadOwnedDeviceInventory(
	ctx context.Context,
	owner model.Owner,
) (deviceplacement.SessionDeviceObservationInventory, error) {
	states, err := source.readStates(ctx, owner)
	if err != nil {
		return deviceplacement.SessionDeviceObservationInventory{}, err
	}
	return deviceplacement.BuildPersistedInventoryObservation(states, deviceinventory.SnapshotOwner{
		Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
	}, source.evaluatedAtMS)
}

func (source persistedInventoryFileSetReadSource) ReadOwnedDeviceInventoryV2(
	ctx context.Context,
	owner model.Owner,
) (deviceplacement.SessionDeviceObservationInventoryV2, error) {
	states, err := source.readStates(ctx, owner)
	if err != nil {
		return deviceplacement.SessionDeviceObservationInventoryV2{}, err
	}
	return deviceplacement.BuildPersistedInventoryObservationV2(states, deviceinventory.SnapshotOwner{
		Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
	}, source.evaluatedAtMS)
}

func (source persistedInventoryReadSource) ReadOwnedDeviceInventory(
	ctx context.Context,
	owner model.Owner,
) (deviceplacement.SessionDeviceObservationInventory, error) {
	if ctx == nil {
		return deviceplacement.SessionDeviceObservationInventory{}, fmt.Errorf("persisted inventory source requires a context")
	}
	if err := ctx.Err(); err != nil {
		return deviceplacement.SessionDeviceObservationInventory{}, err
	}
	return deviceplacement.BuildPersistedInventoryObservation(source.values, deviceinventory.SnapshotOwner{
		Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
	}, source.evaluatedAtMS)
}
