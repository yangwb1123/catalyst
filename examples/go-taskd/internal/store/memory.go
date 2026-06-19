// Package store provides an in-memory adapter that persists domain Tasks.
//
// Clean Architecture: this is an infrastructure adapter. It depends only on the
// domain layer (for the Task entity) and the Go standard library (a map + a
// mutex for safe concurrent access from net/http handlers). It imports neither
// the service nor the httpapi layer; higher layers inject it via a port
// interface they define themselves (dependency inversion).
package store

import (
	"sync"

	"example/taskd/internal/domain"
)

// Memory is a goroutine-safe, in-memory Task store backed by a map. Each
// instance owns isolated state, so separate stores never share tasks.
type Memory struct {
	mu    sync.RWMutex
	tasks map[string]domain.Task
}

// NewMemory constructs an empty, ready-to-use in-memory store.
func NewMemory() *Memory {
	return &Memory{tasks: make(map[string]domain.Task)}
}

// Save inserts or overwrites a task keyed by its ID.
func (m *Memory) Save(t domain.Task) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.tasks[t.ID] = t
	return nil
}

// Get returns the task for id. The bool is false when no such task exists.
func (m *Memory) Get(id string) (domain.Task, bool) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	t, ok := m.tasks[id]
	return t, ok
}

// List returns a snapshot copy of all stored tasks. The returned slice is owned
// by the caller; mutating it does not affect the store. Order is unspecified.
func (m *Memory) List() []domain.Task {
	m.mu.RLock()
	defer m.mu.RUnlock()
	out := make([]domain.Task, 0, len(m.tasks))
	for _, t := range m.tasks {
		out = append(out, t)
	}
	return out
}

// Delete removes the task for id. It reports whether a task was present.
func (m *Memory) Delete(id string) bool {
	m.mu.Lock()
	defer m.mu.Unlock()
	if _, ok := m.tasks[id]; !ok {
		return false
	}
	delete(m.tasks, id)
	return true
}
