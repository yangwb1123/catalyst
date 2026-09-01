// Package application owns Workspace Catalog v1 use cases and journal ports.
package application

import (
	core "forgeos/forge-core/internal/platformcorecontract"
	"forgeos/forge-core/internal/workspace/domain"
)

// CommandMeta is caller-declared command metadata; it is not authentication.
type CommandMeta struct {
	ActorRef        core.ActorRef
	CommandID       string
	CorrelationID   string
	ExpectedVersion int64
	IdempotencyKey  string
	IssuedAtUnixMS  int64
	MessageID       string
}

type CreateSpaceRequest struct {
	Meta    CommandMeta
	Name    string
	SpaceID string
}

type RegisterProjectRequest struct {
	Meta      CommandMeta
	Alias     string
	ProjectID string
	RootPath  string
	SpaceID   string
}

type RecordProjectSnapshotRequest struct {
	Meta              CommandMeta
	CapturedAtUnixMS  int64
	ObservationRef    core.RecordRef
	ProjectID         string
	ProjectSnapshotID string
	SpaceID           string
}

// Page is a bounded global-journal cursor page.
type Page[T any] struct {
	Items                   []T
	More                    bool
	NextAfterGlobalSequence int64
}

type creationPlan struct {
	meta           CommandMeta
	target         core.EntityRef
	scope          core.ScopeRef
	commandSchema  string
	eventSchema    string
	payload        map[string]any
	sourceSnapshot *core.EntityRef
	validate       func(*core.EventEnvelope) error
}

func spacePlan(request CreateSpaceRequest) creationPlan {
	target := core.EntityRef{EntityID: request.SpaceID, EntityType: "space"}
	payload := map[string]any{
		"name": request.Name, "operation": "create_space", "space_id": request.SpaceID,
	}
	return creationPlan{
		meta: request.Meta, target: target, scope: core.ScopeRef{SpaceID: request.SpaceID},
		commandSchema: "forge.workspace.create_space",
		eventSchema:   "forge.workspace.space_created", payload: payload,
		validate: func(event *core.EventEnvelope) error {
			_, err := domain.FoldSpace([]*core.EventEnvelope{event})
			return err
		},
	}
}

func projectPlan(request RegisterProjectRequest) creationPlan {
	target := core.EntityRef{EntityID: request.ProjectID, EntityType: "project"}
	payload := map[string]any{
		"alias": request.Alias, "operation": "register_project", "project_id": request.ProjectID,
		"root_path": request.RootPath, "root_path_status": domain.RootPathDeclaredUnverified,
		"space_id": request.SpaceID,
	}
	return creationPlan{
		meta: request.Meta, target: target,
		scope:         core.ScopeRef{SpaceID: request.SpaceID, ProjectID: stringPointer(request.ProjectID)},
		commandSchema: "forge.workspace.register_project",
		eventSchema:   "forge.workspace.project_registered", payload: payload,
		validate: func(event *core.EventEnvelope) error {
			_, err := domain.FoldProject([]*core.EventEnvelope{event})
			return err
		},
	}
}

func snapshotPlan(request RecordProjectSnapshotRequest) creationPlan {
	target := core.EntityRef{EntityID: request.ProjectSnapshotID, EntityType: "project_snapshot"}
	payload := map[string]any{
		"captured_at_unix_ms": request.CapturedAtUnixMS,
		"observation_ref": map[string]any{
			"record_id": request.ObservationRef.RecordID, "record_sha256": request.ObservationRef.RecordSHA256,
			"record_type": request.ObservationRef.RecordType,
		},
		"observation_status": domain.ObservationDeclaredUnresolved,
		"operation":          "record_project_snapshot", "project_id": request.ProjectID,
		"project_snapshot_id": request.ProjectSnapshotID, "space_id": request.SpaceID,
	}
	return creationPlan{
		meta: request.Meta, target: target,
		scope: core.ScopeRef{
			SpaceID: request.SpaceID, ProjectID: stringPointer(request.ProjectID),
			ProjectSnapshotID: stringPointer(request.ProjectSnapshotID),
		},
		commandSchema: "forge.workspace.record_project_snapshot",
		eventSchema:   "forge.workspace.project_snapshot_recorded", payload: payload,
		sourceSnapshot: &target,
		validate: func(event *core.EventEnvelope) error {
			_, err := domain.FoldProjectSnapshot([]*core.EventEnvelope{event})
			return err
		},
	}
}

func stringPointer(value string) *string { return &value }
