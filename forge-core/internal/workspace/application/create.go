package application

import (
	"context"
	"errors"
	"fmt"

	"forgeos/forge-core/internal/workspace/domain"
)

func (service *Service) CreateSpace(
	ctx context.Context,
	request CreateSpaceRequest,
) (domain.Space, error) {
	prepared, err := service.prepare(ctx, spacePlan(request))
	if err != nil {
		return domain.Space{}, err
	}
	if err := service.commitCreation(ctx, prepared); err != nil {
		return domain.Space{}, err
	}
	return service.Space(ctx, request.SpaceID)
}

func (service *Service) RegisterProject(
	ctx context.Context,
	request RegisterProjectRequest,
) (domain.Project, error) {
	prepared, err := service.prepare(ctx, projectPlan(request))
	if err != nil {
		return domain.Project{}, err
	}
	if _, err := service.Space(ctx, request.SpaceID); err != nil {
		return domain.Project{}, fmt.Errorf("register Project parent: %w", err)
	}
	if err := service.commitCreation(ctx, prepared); err != nil {
		return domain.Project{}, err
	}
	return service.Project(ctx, request.ProjectID)
}

func (service *Service) RecordProjectSnapshot(
	ctx context.Context,
	request RecordProjectSnapshotRequest,
) (domain.ProjectSnapshot, error) {
	prepared, err := service.prepare(ctx, snapshotPlan(request))
	if err != nil {
		return domain.ProjectSnapshot{}, err
	}
	project, err := service.Project(ctx, request.ProjectID)
	if err != nil {
		return domain.ProjectSnapshot{}, fmt.Errorf("record Snapshot parent: %w", err)
	}
	if project.SpaceID != request.SpaceID {
		return domain.ProjectSnapshot{}, invalidInput("Project belongs to another Space", nil)
	}
	if err := service.commitCreation(ctx, prepared); err != nil {
		return domain.ProjectSnapshot{}, err
	}
	return service.ProjectSnapshot(ctx, request.ProjectSnapshotID)
}

func (service *Service) prepare(
	ctx context.Context,
	plan creationPlan,
) (preparedCreation, error) {
	if err := validateServiceContext(ctx, service); err != nil {
		return preparedCreation{}, err
	}
	return service.prepareCreation(plan)
}

func (service *Service) commitCreation(ctx context.Context, value preparedCreation) error {
	for attempt := 0; attempt < maxCommitAttempts; attempt++ {
		head, err := service.journal.ControlSourceHead(ctx, "control_plane")
		if err != nil {
			return err
		}
		_, event, err := value.canonicalEvent(head + 1)
		if err != nil {
			return invalidInput("rebuild event sequence", err)
		}
		receipt, err := service.journal.Commit(ctx, JournalCommit{
			Command: value.command, Event: event, Result: value.result,
			CommittedAtUnixMS: service.now(),
		})
		if errors.Is(err, ErrSourceSequenceConflict) {
			continue
		}
		if err != nil {
			return err
		}
		return validateReceipt(receipt, value.result)
	}
	return fmt.Errorf("%w: retry limit reached", ErrSourceSequenceConflict)
}
