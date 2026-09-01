package application

import (
	"context"
	"errors"
	"fmt"
	"strings"

	core "forgeos/forge-core/internal/platformcorecontract"
	"forgeos/forge-core/internal/workspace/domain"
)

func (service *Service) Space(ctx context.Context, spaceID string) (domain.Space, error) {
	events, err := service.aggregateHistory(ctx, "space", spaceID, "spc")
	if err != nil {
		return domain.Space{}, err
	}
	value, err := domain.FoldSpace(events)
	return value, mapFoldError(err)
}

func (service *Service) Project(ctx context.Context, projectID string) (domain.Project, error) {
	events, err := service.aggregateHistory(ctx, "project", projectID, "prj")
	if err != nil {
		return domain.Project{}, err
	}
	value, err := domain.FoldProject(events)
	if err != nil {
		return domain.Project{}, mapFoldError(err)
	}
	if _, err := service.Space(ctx, value.SpaceID); err != nil {
		return domain.Project{}, parentHistoryError("Project Space", err)
	}
	return value, nil
}

func (service *Service) ProjectSnapshot(
	ctx context.Context,
	snapshotID string,
) (domain.ProjectSnapshot, error) {
	events, err := service.aggregateHistory(ctx, "project_snapshot", snapshotID, "psn")
	if err != nil {
		return domain.ProjectSnapshot{}, err
	}
	value, err := domain.FoldProjectSnapshot(events)
	if err != nil {
		return domain.ProjectSnapshot{}, mapFoldError(err)
	}
	project, err := service.Project(ctx, value.ProjectID)
	if err != nil {
		return domain.ProjectSnapshot{}, parentHistoryError("Snapshot Project", err)
	}
	if project.SpaceID != value.SpaceID {
		return domain.ProjectSnapshot{}, fmt.Errorf(
			"%w: Snapshot and Project belong to different Spaces", ErrInvalidHistory,
		)
	}
	return value, nil
}

func (service *Service) aggregateHistory(
	ctx context.Context,
	aggregateType, aggregateID, prefix string,
) ([]*core.EventEnvelope, error) {
	if err := validateServiceContext(ctx, service); err != nil {
		return nil, err
	}
	if err := validateTypedID(aggregateID, prefix); err != nil {
		return nil, err
	}
	stored, err := service.journal.AggregateEvents(ctx, aggregateType, aggregateID, 0, 2)
	if err != nil {
		return nil, err
	}
	if len(stored) == 0 {
		return nil, ErrNotFound
	}
	if len(stored) != 1 {
		return nil, fmt.Errorf("%w: immutable v1 aggregate has extra events", ErrInvalidHistory)
	}
	event, err := decodeStoredEvent(stored[0])
	if err != nil {
		return nil, err
	}
	return []*core.EventEnvelope{event}, nil
}

func decodeStoredEvent(stored JournalEvent) (*core.EventEnvelope, error) {
	event, err := core.DecodeCanonicalEventEnvelope(stored.Body)
	if err != nil {
		return nil, fmt.Errorf("%w: canonical event: %v", ErrInvalidHistory, err)
	}
	if event.AggregateRef.EntityID != stored.AggregateID ||
		string(event.AggregateRef.EntityType) != stored.AggregateType ||
		event.AggregateVersion != stored.AggregateVersion ||
		event.ActorRef != stored.CommandActorRef {
		return nil, fmt.Errorf("%w: journal metadata differs from event", ErrInvalidHistory)
	}
	return event, nil
}

func validateTypedID(value, prefix string) error {
	if err := core.ValidatePlatformID(value); err != nil || !strings.HasPrefix(value, prefix+"_") {
		return invalidInput("entity ID uses the wrong Platform namespace", err)
	}
	return nil
}

func mapFoldError(err error) error {
	if err == nil {
		return nil
	}
	if errors.Is(err, domain.ErrInvalidHistory) {
		return fmt.Errorf("%w: %v", ErrInvalidHistory, err)
	}
	return err
}

func parentHistoryError(label string, err error) error {
	if errors.Is(err, ErrNotFound) {
		return fmt.Errorf("%w: %s is missing", ErrInvalidHistory, label)
	}
	return fmt.Errorf("%s: %w", label, err)
}
