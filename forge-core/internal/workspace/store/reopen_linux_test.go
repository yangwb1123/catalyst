//go:build linux && !android

package store

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"testing"

	"forgeos/forge-core/internal/workspace/application"
)

func TestWorkspaceCatalogPersistsReplaysAndReopensWithoutReadingProject(t *testing.T) {
	value := openIntegrationStore(t)
	missingPath := filepath.Join(filepath.Dir(value.path), "missing-project")
	spaceRequest := integrationSpace(1)
	projectRequest := integrationProject(2, 1, missingPath)
	snapshotRequest := integrationSnapshot(3, 2, 1)

	space, err := value.service.CreateSpace(context.Background(), spaceRequest)
	if err != nil || space.SpaceID != spaceRequest.SpaceID {
		t.Fatalf("Space = %+v, %v", space, err)
	}
	project, err := value.service.RegisterProject(context.Background(), projectRequest)
	if err != nil || project.RootPath != missingPath || project.RootPathStatus != "declared_unverified" {
		t.Fatalf("Project = %+v, %v", project, err)
	}
	snapshot, err := value.service.RecordProjectSnapshot(context.Background(), snapshotRequest)
	if err != nil || snapshot.ObservationStatus != "declared_unresolved" {
		t.Fatalf("Snapshot = %+v, %v", snapshot, err)
	}
	if _, err := os.Stat(missingPath); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("declared project path was touched: %v", err)
	}

	value.close()
	reopened := openIntegrationPath(t, value.path)
	defer reopened.close()
	assertExactReplay(t, reopened, spaceRequest, projectRequest, snapshotRequest)
	events, err := reopened.control.Events(context.Background(), 0, 10)
	if err != nil || len(events) != 3 {
		t.Fatalf("events after replay = %d, %v", len(events), err)
	}
}

func assertExactReplay(
	t *testing.T,
	value *integrationStore,
	spaceRequest application.CreateSpaceRequest,
	projectRequest application.RegisterProjectRequest,
	snapshotRequest application.RecordProjectSnapshotRequest,
) {
	t.Helper()
	if _, err := value.service.CreateSpace(context.Background(), spaceRequest); err != nil {
		t.Fatal(err)
	}
	if _, err := value.service.RegisterProject(context.Background(), projectRequest); err != nil {
		t.Fatal(err)
	}
	if _, err := value.service.RecordProjectSnapshot(context.Background(), snapshotRequest); err != nil {
		t.Fatal(err)
	}
	projects, err := value.service.ListProjects(context.Background(), spaceRequest.SpaceID, 0, 10)
	if err != nil || len(projects.Items) != 1 || projects.Items[0].ProjectID != projectRequest.ProjectID {
		t.Fatalf("reopened Projects = %+v, %v", projects, err)
	}
	snapshots, err := value.service.ListProjectSnapshots(context.Background(), projectRequest.ProjectID, 0, 10)
	if err != nil || len(snapshots.Items) != 1 || snapshots.Items[0].ProjectSnapshotID != snapshotRequest.ProjectSnapshotID {
		t.Fatalf("reopened Snapshots = %+v, %v", snapshots, err)
	}
}
