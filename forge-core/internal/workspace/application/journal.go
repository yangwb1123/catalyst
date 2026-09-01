package application

import (
	"context"
	"errors"

	core "forgeos/forge-core/internal/platformcorecontract"
)

var (
	ErrConflict               = errors.New("workspace command conflicts with durable state")
	ErrInvalidHistory         = errors.New("workspace durable history is invalid")
	ErrInvalidInput           = errors.New("workspace command input is invalid")
	ErrNotFound               = errors.New("workspace entity was not found")
	ErrSourceSequenceConflict = errors.New("workspace event source sequence changed")
)

type JournalEvent struct {
	AggregateID      string
	AggregateType    string
	AggregateVersion int64
	Body             []byte
	CommandActorRef  core.ActorRef
	GlobalSequence   int64
}

type JournalCommit struct {
	Command           []byte
	CommittedAtUnixMS int64
	Event             []byte
	Result            []byte
}

type JournalReceipt struct {
	AggregateVersion int64
	Replayed         bool
	Result           []byte
}

// Journal is the application-owned port implemented by the Control Store adapter.
type Journal interface {
	AggregateEvents(context.Context, string, string, int64, int) ([]JournalEvent, error)
	Commit(context.Context, JournalCommit) (JournalReceipt, error)
	ControlSourceHead(context.Context, string) (int64, error)
	Events(context.Context, int64, int) ([]JournalEvent, error)
}
