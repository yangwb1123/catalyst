// Package domain owns immutable Workspace Catalog v1 state and replay rules.
package domain

import core "forgeos/forge-core/internal/platformcorecontract"

const (
	RootPathDeclaredUnverified    = "declared_unverified"
	ObservationDeclaredUnresolved = "declared_unresolved"
)

// Space is one immutable v1 workspace container reconstructed from history.
type Space struct {
	SpaceID         string
	Name            string
	CreatedBy       core.ActorRef
	CreatedAtUnixMS int64
	Version         int64
}

// Project is one immutable declared local project catalog entry.
type Project struct {
	ProjectID      string
	SpaceID        string
	Alias          string
	RootPath       string
	RootPathStatus string
	RegisteredBy   core.ActorRef
	RegisteredAtMS int64
	Version        int64
}

// ProjectSnapshot is an unresolved reference to caller-supplied observation.
type ProjectSnapshot struct {
	ProjectSnapshotID string
	ProjectID         string
	SpaceID           string
	ObservationRef    core.RecordRef
	ObservationStatus string
	CapturedAtUnixMS  int64
	RecordedBy        core.ActorRef
	RecordedAtUnixMS  int64
	Version           int64
}
