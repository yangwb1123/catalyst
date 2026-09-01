// Package controlstore owns the Go control-plane SQLite journal boundary.
package controlstore

import (
	"database/sql"
	"errors"
	"os"
	"sync"
)

const (
	databaseName  = "control.db"
	schemaVersion = 1
	maxBatchItems = 32
	maxBodyBytes  = 256 * 1024
	maxPageItems  = 100
)

var (
	ErrSchemaIncompatible  = errors.New("control store schema is incompatible")
	ErrCorruptStore        = errors.New("control store is corrupt")
	ErrVersionConflict     = errors.New("aggregate version conflict")
	ErrIdempotencyConflict = errors.New("idempotency key conflicts with another command")
	ErrIdentifierConflict  = errors.New("durable identifier conflicts with existing history")
	ErrSequenceConflict    = errors.New("event sequence is not the next Control source sequence")
	ErrInboxConflict       = errors.New("inbox message conflicts with durable history")
	ErrInboxGap            = errors.New("inbox source sequence has a gap")
)

// Store owns one validated connection and the directory descriptor that binds
// every SQLite file to the App Server instance namespace.
type Store struct {
	db        *sql.DB
	directory *os.File
	closeOnce sync.Once
	closeErr  error
}

// CommitRequest atomically appends canonical control events, outbox messages,
// and one replayable command result.
type CommitRequest struct {
	Command           []byte
	Events            [][]byte
	Outbox            []OutboxMessage
	Result            []byte
	CommittedAtUnixMS int64
}

// OutboxMessage is opaque transport work bound to an event in the same commit.
type OutboxMessage struct {
	MessageID   string
	SourceEvent string
	Destination string
	Body        []byte
}

// CommitReceipt is returned both for the first commit and exact idempotent replay.
type CommitReceipt struct {
	Replayed            bool
	AggregateVersion    int64
	FirstGlobalSequence int64
	LastGlobalSequence  int64
	Result              []byte
	ResultSHA256        string
}

// StoredEvent is one verified row from the append-only control journal.
type StoredEvent struct {
	GlobalSequence   int64
	CommandID        string
	CommandActorID   string
	CommandActorType string
	EventID          string
	MessageID        string
	CorrelationID    string
	CausationID      *string
	SourceComponent  string
	SourceSequence   int64
	AggregateType    string
	AggregateID      string
	AggregateVersion int64
	OccurredAtUnixMS int64
	Body             []byte
	BodySHA256       string
}

// PendingOutbox is one undelivered, integrity-checked outbox message.
type PendingOutbox struct {
	OutboxSequence  int64
	MessageID       string
	SourceEvent     string
	Destination     string
	CreatedAtUnixMS int64
	Body            []byte
	BodySHA256      string
}

// PendingOutboxPage filters one bounded raw sequence window and exposes its cursor.
type PendingOutboxPage struct {
	Items                   []PendingOutbox
	More                    bool
	NextAfterOutboxSequence int64
}

// InboxReceipt reports whether a canonical source event advanced its stream.
type InboxReceipt struct {
	Replayed bool
	StreamID string
	Sequence int64
}

// StoredInboxEvent is one verified source event retained for projection replay.
type StoredInboxEvent struct {
	InboxSequence    int64
	StreamID         string
	SourceComponent  string
	SourceSequence   int64
	MessageID        string
	EventID          string
	CorrelationID    string
	CausationID      *string
	OccurredAtUnixMS int64
	ReceivedAtUnixMS int64
	Body             []byte
	BodySHA256       string
}
