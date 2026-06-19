package test

import (
	"errors"
	"testing"

	"example/taskd/internal/domain"
)

func TestNewTaskValid(t *testing.T) {
	got, err := domain.NewTask("7", "  buy milk  ")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if got.ID != "7" {
		t.Errorf("ID = %q, want %q", got.ID, "7")
	}
	if got.Title != "buy milk" {
		t.Errorf("Title = %q, want trimmed %q", got.Title, "buy milk")
	}
	if got.Done {
		t.Error("new task must not be Done")
	}
}

func TestNewTaskEmptyTitleRejected(t *testing.T) {
	for _, title := range []string{"", "   ", "\t\n"} {
		if _, err := domain.NewTask("1", title); !errors.Is(err, domain.ErrEmptyTitle) {
			t.Errorf("NewTask(%q) err = %v, want ErrEmptyTitle", title, err)
		}
	}
}

func TestMarkDoneIsImmutable(t *testing.T) {
	orig, _ := domain.NewTask("1", "x")
	done := orig.MarkDone()
	if !done.Done {
		t.Error("MarkDone result must be Done")
	}
	if orig.Done {
		t.Error("MarkDone must not mutate the receiver")
	}
}
