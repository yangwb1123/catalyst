package store

import (
	"errors"
	"fmt"

	"forgeos/forge-core/internal/controlstore"
	core "forgeos/forge-core/internal/platformcorecontract"
	"forgeos/forge-core/internal/workspace/application"
)

func mapEvents(values []controlstore.StoredEvent) []application.JournalEvent {
	if values == nil {
		return nil
	}
	result := make([]application.JournalEvent, 0, len(values))
	for _, value := range values {
		result = append(result, application.JournalEvent{
			AggregateID: value.AggregateID, AggregateType: value.AggregateType,
			AggregateVersion: value.AggregateVersion, Body: cloneBytes(value.Body),
			CommandActorRef: core.ActorRef{
				ActorID: value.CommandActorID, ActorType: core.ActorType(value.CommandActorType),
			},
			GlobalSequence: value.GlobalSequence,
		})
	}
	return result
}

func mapError(err error) error {
	switch {
	case err == nil:
		return nil
	case errors.Is(err, controlstore.ErrSequenceConflict):
		return fmt.Errorf("%w: %w", application.ErrSourceSequenceConflict, err)
	case errors.Is(err, controlstore.ErrVersionConflict),
		errors.Is(err, controlstore.ErrIdempotencyConflict),
		errors.Is(err, controlstore.ErrIdentifierConflict):
		return fmt.Errorf("%w: %w", application.ErrConflict, err)
	case errors.Is(err, controlstore.ErrCorruptStore):
		return fmt.Errorf("%w: %w", application.ErrInvalidHistory, err)
	default:
		return err
	}
}

func cloneBytes(value []byte) []byte {
	result := make([]byte, len(value))
	copy(result, value)
	return result
}
