// Package store adapts the private Control Store to the Workspace application port.
package store

import (
	"context"
	"fmt"

	"forgeos/forge-core/internal/controlstore"
	"forgeos/forge-core/internal/workspace/application"
)

// Journal is the Workspace-specific adapter over one open Control Store.
type Journal struct {
	control *controlstore.Store
}

// New constructs a Workspace journal adapter without opening another database.
func New(control *controlstore.Store) *Journal {
	return &Journal{control: control}
}

func (journal *Journal) ControlSourceHead(ctx context.Context, source string) (int64, error) {
	if err := journal.ready(); err != nil {
		return 0, err
	}
	head, err := journal.control.ControlSourceHead(ctx, source)
	return head, mapError(err)
}

func (journal *Journal) AggregateEvents(
	ctx context.Context,
	aggregateType, aggregateID string,
	afterVersion int64,
	limit int,
) ([]application.JournalEvent, error) {
	if err := journal.ready(); err != nil {
		return nil, err
	}
	values, err := journal.control.AggregateEvents(
		ctx, aggregateType, aggregateID, afterVersion, limit,
	)
	return mapEvents(values), mapError(err)
}

func (journal *Journal) Events(
	ctx context.Context, afterGlobalSequence int64, limit int,
) ([]application.JournalEvent, error) {
	if err := journal.ready(); err != nil {
		return nil, err
	}
	values, err := journal.control.Events(ctx, afterGlobalSequence, limit)
	return mapEvents(values), mapError(err)
}

func (journal *Journal) Commit(
	ctx context.Context,
	value application.JournalCommit,
) (application.JournalReceipt, error) {
	if err := journal.ready(); err != nil {
		return application.JournalReceipt{}, err
	}
	receipt, err := journal.control.Commit(ctx, controlstore.CommitRequest{
		Command: value.Command, Events: [][]byte{value.Event}, Result: value.Result,
		CommittedAtUnixMS: value.CommittedAtUnixMS,
	})
	if err != nil {
		return application.JournalReceipt{}, mapError(err)
	}
	return application.JournalReceipt{
		AggregateVersion: receipt.AggregateVersion,
		Replayed:         receipt.Replayed,
		Result:           cloneBytes(receipt.Result),
	}, nil
}

func (journal *Journal) ready() error {
	if journal == nil || journal.control == nil {
		return fmt.Errorf("Workspace journal requires an open Control Store")
	}
	return nil
}
