//go:build linux && !android

package store

import (
	"context"
	"errors"
	"path/filepath"
	"sync"
	"testing"

	"forgeos/forge-core/internal/workspace/application"
)

func TestWorkspaceIdempotencyConflictAppendsNothing(t *testing.T) {
	value := openIntegrationStore(t)
	request := integrationSpace(10)
	if _, err := value.service.CreateSpace(context.Background(), request); err != nil {
		t.Fatal(err)
	}
	request.Name = "Different Space"
	if _, err := value.service.CreateSpace(context.Background(), request); !errors.Is(err, application.ErrConflict) {
		t.Fatalf("idempotency conflict error = %v", err)
	}
	events, err := value.control.Events(context.Background(), 0, 10)
	if err != nil || len(events) != 1 {
		t.Fatalf("events after conflict = %d, %v", len(events), err)
	}
}

func TestConcurrentProjectCreationHasOneDurableWinner(t *testing.T) {
	value := openIntegrationStore(t)
	if _, err := value.service.CreateSpace(context.Background(), integrationSpace(20)); err != nil {
		t.Fatal(err)
	}
	rootPath := filepath.Join(filepath.Dir(value.path), "declared-project")
	requests := []application.RegisterProjectRequest{
		integrationProject(21, 20, rootPath), integrationProject(22, 20, rootPath),
	}
	requests[1].ProjectID = requests[0].ProjectID
	var wait sync.WaitGroup
	results := make(chan error, len(requests))
	for _, request := range requests {
		wait.Add(1)
		go func(request application.RegisterProjectRequest) {
			defer wait.Done()
			_, err := value.service.RegisterProject(context.Background(), request)
			results <- err
		}(request)
	}
	wait.Wait()
	close(results)
	successes, conflicts := 0, 0
	for err := range results {
		if err == nil {
			successes++
		} else if errors.Is(err, application.ErrConflict) {
			conflicts++
		} else {
			t.Fatalf("unexpected concurrent error = %v", err)
		}
	}
	if successes != 1 || conflicts != 1 {
		t.Fatalf("successes=%d conflicts=%d", successes, conflicts)
	}
}
