package test

import (
	"errors"
	"testing"

	"example/taskd/internal/domain"
	"example/taskd/internal/service"
	"example/taskd/internal/store"
)

// newService wires a real in-memory store into the service (no mocks) so the
// use-case logic is exercised against its actual collaborator.
func newService() *service.Service {
	return service.New(store.NewMemory())
}

func TestServiceCreateAssignsUniqueIDs(t *testing.T) {
	s := newService()
	a, err := s.Create("first")
	if err != nil {
		t.Fatalf("Create: %v", err)
	}
	b, _ := s.Create("second")
	if a.ID == b.ID {
		t.Errorf("ids not unique: %q == %q", a.ID, b.ID)
	}
	if a.Title != "first" || a.Done {
		t.Errorf("unexpected task %#v", a)
	}
}

func TestServiceCreateRejectsEmptyTitle(t *testing.T) {
	if _, err := newService().Create("   "); !errors.Is(err, domain.ErrEmptyTitle) {
		t.Errorf("err = %v, want ErrEmptyTitle", err)
	}
}

func TestServiceGetAndList(t *testing.T) {
	s := newService()
	created, _ := s.Create("task")
	got, err := s.Get(created.ID)
	if err != nil || got != created {
		t.Fatalf("Get = %#v, %v; want %#v", got, err, created)
	}
	if _, err := s.Get("missing"); !errors.Is(err, service.ErrNotFound) {
		t.Errorf("Get(missing) err = %v, want ErrNotFound", err)
	}
	if len(s.List()) != 1 {
		t.Errorf("List len = %d, want 1", len(s.List()))
	}
}

func TestServiceComplete(t *testing.T) {
	s := newService()
	created, _ := s.Create("task")
	done, err := s.Complete(created.ID)
	if err != nil {
		t.Fatalf("Complete: %v", err)
	}
	if !done.Done {
		t.Error("completed task must be Done")
	}
	persisted, _ := s.Get(created.ID)
	if !persisted.Done {
		t.Error("completion must persist through the store")
	}
	if _, err := s.Complete("missing"); !errors.Is(err, service.ErrNotFound) {
		t.Errorf("Complete(missing) err = %v, want ErrNotFound", err)
	}
}

func TestServiceDelete(t *testing.T) {
	s := newService()
	created, _ := s.Create("task")
	if err := s.Delete(created.ID); err != nil {
		t.Fatalf("Delete: %v", err)
	}
	if err := s.Delete(created.ID); !errors.Is(err, service.ErrNotFound) {
		t.Errorf("Delete(missing) err = %v, want ErrNotFound", err)
	}
}
