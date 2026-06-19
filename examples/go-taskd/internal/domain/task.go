// Package domain holds the pure business entities for the task service.
//
// Clean Architecture: this is the innermost layer. It imports NOTHING outside
// the Go standard library (here: only `errors` and `strings`) and knows nothing
// about storage, transport, or serialization. All higher layers depend on it;
// it depends on no one.
package domain

import (
	"errors"
	"strings"
)

// ErrEmptyTitle is returned when a task is constructed with a blank title.
// Callers (the service layer) surface this as a 400-class validation error.
var ErrEmptyTitle = errors.New("task title must not be empty")

// Task is the core entity: an identified to-do item with a completion flag.
// IDs are assigned by the service layer, not the domain — the zero value of a
// freshly validated Task has an empty ID until persisted.
type Task struct {
	ID    string `json:"id"`
	Title string `json:"title"`
	Done  bool   `json:"done"`
}

// NewTask validates and constructs a Task with the given id and title.
// The title is trimmed of surrounding whitespace; a title that is empty (or
// only whitespace) is rejected with ErrEmptyTitle. A new task is never Done.
func NewTask(id, title string) (Task, error) {
	trimmed := strings.TrimSpace(title)
	if trimmed == "" {
		return Task{}, ErrEmptyTitle
	}
	return Task{ID: id, Title: trimmed, Done: false}, nil
}

// MarkDone returns a copy of the task with Done set to true. The receiver is
// not mutated, keeping the entity value-semantic and side-effect free.
func (t Task) MarkDone() Task {
	t.Done = true
	return t
}
