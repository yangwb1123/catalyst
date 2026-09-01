//go:build linux && !android

package store

import (
	"context"
	"path/filepath"
	"testing"
)

func TestWorkspaceListsFilterParentsAndAdvanceGlobalCursor(t *testing.T) {
	value := openIntegrationStore(t)
	for _, serial := range []int{30, 31} {
		if _, err := value.service.CreateSpace(context.Background(), integrationSpace(serial)); err != nil {
			t.Fatal(err)
		}
		root := filepath.Join(filepath.Dir(value.path), "project", integrationID("prj", serial))
		if _, err := value.service.RegisterProject(
			context.Background(), integrationProject(serial+10, serial, root),
		); err != nil {
			t.Fatal(err)
		}
	}
	first, err := value.service.ListSpaces(context.Background(), 0, 1)
	if err != nil || len(first.Items) != 1 || !first.More {
		t.Fatalf("first page = %+v, %v", first, err)
	}
	second, err := value.service.ListSpaces(
		context.Background(), first.NextAfterGlobalSequence, 1,
	)
	if err != nil || len(second.Items) != 1 || !second.More {
		t.Fatalf("second page = %+v, %v", second, err)
	}
	last, err := value.service.ListSpaces(
		context.Background(), second.NextAfterGlobalSequence, 1,
	)
	if err != nil || len(last.Items) != 0 || last.More {
		t.Fatalf("terminal page = %+v, %v", last, err)
	}
	projects, err := value.service.ListProjects(
		context.Background(), integrationID("spc", 30), 0, 10,
	)
	if err != nil || len(projects.Items) != 1 || projects.Items[0].SpaceID != integrationID("spc", 30) {
		t.Fatalf("filtered Projects = %+v, %v", projects, err)
	}
}
