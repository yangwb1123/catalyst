package store

import (
	"errors"
	"testing"

	"forgeos/forge-core/internal/controlstore"
	"forgeos/forge-core/internal/workspace/application"
)

func TestMapErrorPreservesStableApplicationCategories(t *testing.T) {
	tests := []struct {
		name string
		from error
		to   error
	}{
		{"corrupt", controlstore.ErrCorruptStore, application.ErrInvalidHistory},
		{"sequence", controlstore.ErrSequenceConflict, application.ErrSourceSequenceConflict},
		{"version", controlstore.ErrVersionConflict, application.ErrConflict},
		{"idempotency", controlstore.ErrIdempotencyConflict, application.ErrConflict},
		{"identifier", controlstore.ErrIdentifierConflict, application.ErrConflict},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if err := mapError(test.from); !errors.Is(err, test.to) {
				t.Fatalf("mapError(%v) = %v, want %v", test.from, err, test.to)
			}
		})
	}
}
