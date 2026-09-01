//go:build linux && !android

package store

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"testing"

	"forgeos/forge-core/internal/controlstore"
	core "forgeos/forge-core/internal/platformcorecontract"
	"forgeos/forge-core/internal/workspace/application"
)

const integrationUnixMS = int64(1788091200000)

type integrationStore struct {
	control *controlstore.Store
	root    *os.Root
	path    string
	service *application.Service
}

func openIntegrationStore(t *testing.T) *integrationStore {
	t.Helper()
	parent := t.TempDir()
	if err := os.Chmod(parent, 0o700); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(parent, "state")
	if err := os.Mkdir(path, 0o700); err != nil {
		t.Fatal(err)
	}
	value := openIntegrationPath(t, path)
	t.Cleanup(value.close)
	return value
}

func openIntegrationPath(t *testing.T, path string) *integrationStore {
	t.Helper()
	root, err := os.OpenRoot(path)
	if err != nil {
		t.Fatal(err)
	}
	control, err := controlstore.OpenBound(context.Background(), root)
	if err != nil {
		_ = root.Close()
		t.Fatal(err)
	}
	service, err := application.NewService(New(control))
	if err != nil {
		_ = control.Close()
		_ = root.Close()
		t.Fatal(err)
	}
	return &integrationStore{control: control, root: root, path: path, service: service}
}

func (value *integrationStore) close() {
	if value == nil {
		return
	}
	if value.control != nil {
		_ = value.control.Close()
		value.control = nil
	}
	if value.root != nil {
		_ = value.root.Close()
		value.root = nil
	}
}

func integrationID(prefix string, serial int) string {
	return fmt.Sprintf("%s_%026d", prefix, serial)
}

func integrationMeta(serial int) application.CommandMeta {
	return application.CommandMeta{
		ActorRef:  core.ActorRef{ActorID: integrationID("acr", 1), ActorType: "human"},
		CommandID: integrationID("cmd", serial), CorrelationID: integrationID("cor", serial),
		ExpectedVersion: 0, IdempotencyKey: fmt.Sprintf("workspace-integration-%04d", serial),
		IssuedAtUnixMS: integrationUnixMS + int64(serial), MessageID: integrationID("msg", serial),
	}
}

func integrationSpace(serial int) application.CreateSpaceRequest {
	return application.CreateSpaceRequest{
		Meta: integrationMeta(serial), Name: fmt.Sprintf("Integration Space %d", serial),
		SpaceID: integrationID("spc", serial),
	}
}

func integrationProject(
	serial, spaceSerial int,
	rootPath string,
) application.RegisterProjectRequest {
	return application.RegisterProjectRequest{
		Meta: integrationMeta(serial), Alias: fmt.Sprintf("project-%d", serial),
		ProjectID: integrationID("prj", serial), RootPath: rootPath,
		SpaceID: integrationID("spc", spaceSerial),
	}
}

func integrationSnapshot(
	serial, projectSerial, spaceSerial int,
) application.RecordProjectSnapshotRequest {
	return application.RecordProjectSnapshotRequest{
		Meta: integrationMeta(serial), CapturedAtUnixMS: integrationUnixMS,
		ObservationRef: core.RecordRef{
			RecordID:     fmt.Sprintf("snapshot-observation-%d", serial),
			RecordSHA256: fmt.Sprintf("%064x", serial),
			RecordType:   "forge.observation.project_snapshot",
		},
		ProjectID:         integrationID("prj", projectSerial),
		ProjectSnapshotID: integrationID("psn", serial),
		SpaceID:           integrationID("spc", spaceSerial),
	}
}
