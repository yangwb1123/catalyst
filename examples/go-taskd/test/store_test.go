package test

import (
	"testing"

	"example/taskd/internal/domain"
	"example/taskd/internal/store"
)

func TestStoreRoundTrip(t *testing.T) {
	s := store.NewMemory()
	task := domain.Task{ID: "1", Title: "a", Done: false}
	if err := s.Save(task); err != nil {
		t.Fatalf("Save: %v", err)
	}
	got, ok := s.Get("1")
	if !ok || got != task {
		t.Fatalf("Get = %#v, %v; want %#v, true", got, ok, task)
	}
}

func TestStoreGetMissing(t *testing.T) {
	s := store.NewMemory()
	if _, ok := s.Get("nope"); ok {
		t.Error("Get of missing id should report ok=false")
	}
}

func TestStoreListAndDelete(t *testing.T) {
	s := store.NewMemory()
	_ = s.Save(domain.Task{ID: "1", Title: "a"})
	_ = s.Save(domain.Task{ID: "2", Title: "b"})
	if got := len(s.List()); got != 2 {
		t.Fatalf("List len = %d, want 2", got)
	}
	if !s.Delete("1") {
		t.Error("Delete existing should return true")
	}
	if s.Delete("1") {
		t.Error("Delete of already-removed id should return false")
	}
	if got := len(s.List()); got != 1 {
		t.Fatalf("List len after delete = %d, want 1", got)
	}
}

func TestStoreInstancesAreIsolated(t *testing.T) {
	a := store.NewMemory()
	b := store.NewMemory()
	_ = a.Save(domain.Task{ID: "1", Title: "x"})
	if _, ok := b.Get("1"); ok {
		t.Error("stores must not share state")
	}
}

func TestStoreListSnapshotIsolation(t *testing.T) {
	s := store.NewMemory()
	_ = s.Save(domain.Task{ID: "1", Title: "x"})
	snap := s.List()
	snap[0].Title = "mutated"
	got, _ := s.Get("1")
	if got.Title != "x" {
		t.Errorf("mutating List() snapshot leaked into store: %q", got.Title)
	}
}
