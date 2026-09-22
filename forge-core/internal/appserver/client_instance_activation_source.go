package appserver

import (
	"bytes"
	"context"
	"fmt"
	"path/filepath"

	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/deviceplacement"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"forgeos/forge-core/internal/statefs"
)

// activatedClientInstanceSessionViewFileSource is the read-only production
// adapter for the accepted device-fabric assembly. The file is an explicit
// owner-private declaration image; it is never treated as an installation
// registry, authentication proof, or Prompt/session authority.
type activatedClientInstanceSessionViewFileSource struct {
	path string
}

func newActivatedClientInstanceSessionViewFileSource(
	path string,
) activatedClientInstanceSessionViewFileSource {
	return activatedClientInstanceSessionViewFileSource{path: filepath.Clean(path)}
}

func (source activatedClientInstanceSessionViewFileSource) ReadOwnedClientInstanceSessionView(
	ctx context.Context,
	owner model.Owner,
) (deviceplacement.ClientInstanceSessionViewObservation, error) {
	if ctx == nil {
		return deviceplacement.ClientInstanceSessionViewObservation{}, fmt.Errorf("client-instance declaration source requires a context")
	}
	if err := ctx.Err(); err != nil {
		return deviceplacement.ClientInstanceSessionViewObservation{}, err
	}
	data, err := readPrivateClientInstanceSessionViewFile(source.path)
	if err != nil {
		return deviceplacement.ClientInstanceSessionViewObservation{}, err
	}
	declared, err := deviceplacement.DecodeClientInstanceSessionView(bytes.NewReader(data))
	if err != nil {
		return deviceplacement.ClientInstanceSessionViewObservation{}, fmt.Errorf("client-instance declaration image is invalid: %w", err)
	}
	expected := deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}
	if declared.Owner != expected {
		return deviceplacement.ClientInstanceSessionViewObservation{}, fmt.Errorf("client-instance declaration owner does not match the authenticated owner")
	}
	instances := cloneClientInstanceSessionViewInstances(declared.Instances)
	return deviceplacement.ObserveClientInstanceSessionView(deviceplacement.ClientInstanceSessionViewRequest{
		Owner: expected, Instances: instances,
	})
}

type activatedClientInstanceResourceViewSource struct {
	inventory clientInstanceResourceViewStateReader
	clients   clientInstanceSessionViewReadSource
}

func (source activatedClientInstanceResourceViewSource) ReadOwnedClientInstanceResourceView(
	ctx context.Context,
	owner model.Owner,
) (deviceplacement.ClientInstanceResourceViewObservation, error) {
	if ctx == nil {
		return deviceplacement.ClientInstanceResourceViewObservation{}, fmt.Errorf("client-instance resource source requires a context")
	}
	if err := ctx.Err(); err != nil {
		return deviceplacement.ClientInstanceResourceViewObservation{}, err
	}
	if source.inventory == nil || source.clients == nil {
		return deviceplacement.ClientInstanceResourceViewObservation{}, fmt.Errorf("client-instance resource source is unavailable")
	}
	clients, err := source.clients.ReadOwnedClientInstanceSessionView(ctx, owner)
	if err != nil {
		return deviceplacement.ClientInstanceResourceViewObservation{}, err
	}
	states, err := source.inventory.readStates(ctx, owner)
	if err != nil {
		return deviceplacement.ClientInstanceResourceViewObservation{}, err
	}
	return deviceplacement.BuildPersistedInventoryClientInstanceResourceView(
		states,
		deviceinventory.SnapshotOwner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
		clients.Instances,
	)
}

func readPrivateClientInstanceSessionViewFile(path string) ([]byte, error) {
	file, present, err := statefs.InspectRegular(path)
	if err != nil {
		return nil, fmt.Errorf("client-instance declaration image is invalid: %w", err)
	}
	if !present {
		return nil, fmt.Errorf("client-instance declaration image is missing")
	}
	directory, present, err := statefs.InspectDir(filepath.Dir(path))
	if err != nil {
		return nil, fmt.Errorf("client-instance declaration image is invalid: %w", err)
	}
	if !present || directory.Mode().Perm()&0o077 != 0 {
		return nil, fmt.Errorf("client-instance declaration image parent is not private")
	}
	if file.Mode().Perm() != 0o600 {
		return nil, fmt.Errorf("client-instance declaration image mode must be 0600")
	}
	data, present, err := statefs.ReadRegularUnmodified(path, deviceplacement.MaxRequestBytes)
	if err != nil {
		return nil, fmt.Errorf("client-instance declaration image is invalid: %w", err)
	}
	if !present {
		return nil, fmt.Errorf("client-instance declaration image is missing")
	}
	return data, nil
}
