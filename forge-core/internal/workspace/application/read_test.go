package application

import (
	"context"
	"errors"
	"testing"
)

func TestParentChecksAndFilteredCursorPages(t *testing.T) {
	journal := &memoryJournal{}
	service := testService(journal)
	if _, err := service.RegisterProject(
		context.Background(), testProjectRequest(30, 1),
	); !errors.Is(err, ErrNotFound) {
		t.Fatalf("missing Space error = %v", err)
	}
	for _, serial := range []int{1, 2} {
		if _, err := service.CreateSpace(context.Background(), testSpaceRequest(serial)); err != nil {
			t.Fatal(err)
		}
		if _, err := service.RegisterProject(
			context.Background(), testProjectRequest(30+serial, serial),
		); err != nil {
			t.Fatal(err)
		}
	}
	first, err := service.ListSpaces(context.Background(), 0, 1)
	if err != nil || len(first.Items) != 1 || !first.More {
		t.Fatalf("first Space page = %+v, %v", first, err)
	}
	second, err := service.ListSpaces(context.Background(), first.NextAfterGlobalSequence, 1)
	if err != nil || len(second.Items) != 1 || !second.More {
		t.Fatalf("second Space page = %+v, %v", second, err)
	}
	last, err := service.ListSpaces(context.Background(), second.NextAfterGlobalSequence, 1)
	if err != nil || len(last.Items) != 0 || last.More {
		t.Fatalf("terminal Space page = %+v, %v", last, err)
	}
	projects, err := service.ListProjects(context.Background(), testID("spc", 1), 0, 10)
	if err != nil || len(projects.Items) != 1 || projects.Items[0].SpaceID != testID("spc", 1) {
		t.Fatalf("filtered Projects = %+v, %v", projects, err)
	}
}

func TestSnapshotRequiresProjectInSameSpace(t *testing.T) {
	journal := &memoryJournal{}
	service := testService(journal)
	for _, serial := range []int{1, 2} {
		if _, err := service.CreateSpace(context.Background(), testSpaceRequest(serial)); err != nil {
			t.Fatal(err)
		}
	}
	if _, err := service.RegisterProject(
		context.Background(), testProjectRequest(40, 1),
	); err != nil {
		t.Fatal(err)
	}
	request := testSnapshotRequest(41, 40, 2)
	if _, err := service.RecordProjectSnapshot(context.Background(), request); !errors.Is(err, ErrInvalidInput) {
		t.Fatalf("cross-Space Snapshot error = %v", err)
	}
}

func TestReadFailsClosedOnExtraDurableHistory(t *testing.T) {
	journal := &memoryJournal{}
	service := testService(journal)
	request := testSpaceRequest(50)
	if _, err := service.CreateSpace(context.Background(), request); err != nil {
		t.Fatal(err)
	}
	stored := journal.events[0]
	stored.AggregateVersion = 2
	journal.appendHistory(stored)
	if _, err := service.Space(context.Background(), request.SpaceID); !errors.Is(err, ErrInvalidHistory) {
		t.Fatalf("extra history error = %v", err)
	}
}
