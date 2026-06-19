// Package service holds the use-case orchestration for the task list.
//
// Clean Architecture: this layer depends only on the domain (pure entities) and
// on a Store *port* — an interface declared HERE and satisfied by an injected
// adapter. It must NOT import the concrete store package; the composition root
// (main / tests) wires a real store in. Dependencies point inward only.
package service

import (
	"errors"
	"strconv"
	"sync/atomic"

	"example/taskd/internal/domain"
)

// ErrNotFound is returned by Get/Complete/Delete when no task has the given ID.
var ErrNotFound = errors.New("task not found")

// Store is the port the service depends on. The store package's *Memory value
// satisfies it, but the service never names that concrete type — inversion of
// control keeps infrastructure pluggable and the dependency arrow inward.
type Store interface {
	Save(t domain.Task) error
	Get(id string) (domain.Task, bool)
	List() []domain.Task
	Delete(id string) bool
}

// Service implements the task use cases over an injected Store port. IDs are
// generated from a monotonic counter so every created task gets a unique,
// stable identifier without colliding under concurrent Create calls.
type Service struct {
	store Store
	seq   atomic.Uint64
}

// New constructs a Service bound to the given Store port.
func New(store Store) *Service {
	return &Service{store: store}
}

// Create validates a title, assigns a fresh ID, persists the task and returns
// it. A blank title yields domain.ErrEmptyTitle (a 400-class error).
func (s *Service) Create(title string) (domain.Task, error) {
	id := strconv.FormatUint(s.seq.Add(1), 10)
	t, err := domain.NewTask(id, title)
	if err != nil {
		return domain.Task{}, err
	}
	if err := s.store.Save(t); err != nil {
		return domain.Task{}, err
	}
	return t, nil
}

// List returns all stored tasks (order unspecified).
func (s *Service) List() []domain.Task {
	return s.store.List()
}

// Get returns the task for id, or ErrNotFound when absent.
func (s *Service) Get(id string) (domain.Task, error) {
	t, ok := s.store.Get(id)
	if !ok {
		return domain.Task{}, ErrNotFound
	}
	return t, nil
}

// Complete marks the task done and persists it, returning the updated task.
// An unknown id yields ErrNotFound.
func (s *Service) Complete(id string) (domain.Task, error) {
	t, ok := s.store.Get(id)
	if !ok {
		return domain.Task{}, ErrNotFound
	}
	done := t.MarkDone()
	if err := s.store.Save(done); err != nil {
		return domain.Task{}, err
	}
	return done, nil
}

// Delete removes the task for id, or returns ErrNotFound when absent.
func (s *Service) Delete(id string) error {
	if !s.store.Delete(id) {
		return ErrNotFound
	}
	return nil
}
