package application

import (
	"context"
	"errors"
	"fmt"

	"forgeos/forge-core/internal/workspace/domain"
)

func (service *Service) ListSpaces(
	ctx context.Context, after int64, limit int,
) (Page[domain.Space], error) {
	return listType(ctx, service, "space", after, limit,
		func(ctx context.Context, aggregateID string) (domain.Space, error) {
			return service.Space(ctx, aggregateID)
		},
		func(domain.Space) bool { return true })
}

func (service *Service) ListProjects(
	ctx context.Context, spaceID string, after int64, limit int,
) (Page[domain.Project], error) {
	initial := Page[domain.Project]{NextAfterGlobalSequence: after}
	if err := validateList(ctx, service, after, limit); err != nil {
		return initial, err
	}
	if err := validateTypedID(spaceID, "spc"); err != nil {
		return initial, err
	}
	if _, err := service.Space(ctx, spaceID); err != nil {
		return initial, fmt.Errorf("list Projects parent: %w", err)
	}
	return listType(ctx, service, "project", after, limit,
		func(ctx context.Context, aggregateID string) (domain.Project, error) {
			return service.Project(ctx, aggregateID)
		},
		func(value domain.Project) bool { return value.SpaceID == spaceID })
}

func (service *Service) ListProjectSnapshots(
	ctx context.Context, projectID string, after int64, limit int,
) (Page[domain.ProjectSnapshot], error) {
	initial := Page[domain.ProjectSnapshot]{NextAfterGlobalSequence: after}
	if err := validateList(ctx, service, after, limit); err != nil {
		return initial, err
	}
	if err := validateTypedID(projectID, "prj"); err != nil {
		return initial, err
	}
	project, err := service.Project(ctx, projectID)
	if err != nil {
		return initial, fmt.Errorf("list Snapshots parent: %w", err)
	}
	return listType(ctx, service, "project_snapshot", after, limit,
		func(ctx context.Context, aggregateID string) (domain.ProjectSnapshot, error) {
			return service.ProjectSnapshot(ctx, aggregateID)
		},
		func(value domain.ProjectSnapshot) bool {
			return value.ProjectID == projectID && value.SpaceID == project.SpaceID
		})
}

func listType[T any](
	ctx context.Context, service *Service, aggregateType string,
	after int64, limit int,
	load func(context.Context, string) (T, error),
	include func(T) bool,
) (Page[T], error) {
	initial := Page[T]{NextAfterGlobalSequence: after}
	if err := validateList(ctx, service, after, limit); err != nil {
		return initial, err
	}
	page := initial
	scanned := 0
	for len(page.Items) < limit && scanned < maxListScan {
		batchLimit := min(journalPageLimit, maxListScan-scanned)
		batch, err := service.journal.Events(ctx, page.NextAfterGlobalSequence, batchLimit)
		if err != nil {
			return initial, err
		}
		if len(batch) == 0 {
			return page, nil
		}
		stop, err := consumeBatch(ctx, &page, batch, aggregateType, limit, load, include)
		if err != nil {
			return initial, err
		}
		scanned += stop.consumed
		if stop.stopped && stop.consumed < len(batch) {
			page.More = true
			return page, nil
		}
		if len(batch) < batchLimit {
			return page, nil
		}
		if stop.stopped {
			more, err := service.hasMore(ctx, page.NextAfterGlobalSequence)
			if err != nil {
				return initial, err
			}
			page.More = more
			return page, nil
		}
	}
	more, err := service.hasMore(ctx, page.NextAfterGlobalSequence)
	if err != nil {
		return initial, err
	}
	page.More = more
	return page, nil
}

type batchStop struct {
	consumed int
	stopped  bool
}

func consumeBatch[T any](
	ctx context.Context,
	page *Page[T],
	batch []JournalEvent,
	aggregateType string,
	limit int,
	load func(context.Context, string) (T, error),
	include func(T) bool,
) (batchStop, error) {
	for index, stored := range batch {
		page.NextAfterGlobalSequence = stored.GlobalSequence
		if stored.AggregateType != aggregateType {
			continue
		}
		if _, err := decodeStoredEvent(stored); err != nil {
			return batchStop{}, err
		}
		value, err := load(ctx, stored.AggregateID)
		if errors.Is(err, ErrNotFound) {
			return batchStop{}, fmt.Errorf("%w: listed aggregate disappeared", ErrInvalidHistory)
		}
		if err != nil {
			return batchStop{}, err
		}
		if include(value) {
			page.Items = append(page.Items, value)
		}
		if len(page.Items) == limit {
			return batchStop{consumed: index + 1, stopped: true}, nil
		}
	}
	return batchStop{consumed: len(batch)}, nil
}

func validateList(ctx context.Context, service *Service, after int64, limit int) error {
	if err := validateServiceContext(ctx, service); err != nil {
		return err
	}
	if after < 0 || limit < 1 || limit > maxListLimit {
		return invalidInput("list requires after >= 0 and limit in 1..100", nil)
	}
	return nil
}

func (service *Service) hasMore(ctx context.Context, after int64) (bool, error) {
	values, err := service.journal.Events(ctx, after, 1)
	if err != nil {
		return false, fmt.Errorf("inspect Workspace continuation: %w", err)
	}
	return len(values) != 0, nil
}
