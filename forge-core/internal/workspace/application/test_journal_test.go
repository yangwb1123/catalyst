package application

import (
	"context"
	"fmt"
	"sync"

	core "forgeos/forge-core/internal/platformcorecontract"
)

type memoryJournal struct {
	mu                sync.Mutex
	head              int64
	global            int64
	sequenceConflicts int
	globalRowsRead    int
	commits           []JournalCommit
	events            []JournalEvent
}

func (journal *memoryJournal) ControlSourceHead(context.Context, string) (int64, error) {
	journal.mu.Lock()
	defer journal.mu.Unlock()
	return journal.head, nil
}

func (journal *memoryJournal) Commit(
	_ context.Context,
	value JournalCommit,
) (JournalReceipt, error) {
	journal.mu.Lock()
	defer journal.mu.Unlock()
	journal.commits = append(journal.commits, cloneCommit(value))
	if journal.sequenceConflicts > 0 {
		journal.sequenceConflicts--
		journal.head++
		return JournalReceipt{}, ErrSourceSequenceConflict
	}
	event, err := core.DecodeCanonicalEventEnvelope(value.Event)
	if err != nil {
		return JournalReceipt{}, err
	}
	command, err := core.DecodeCanonicalCommandEnvelope(value.Command)
	if err != nil {
		return JournalReceipt{}, err
	}
	journal.head, journal.global = event.Sequence, journal.global+1
	journal.events = append(journal.events, JournalEvent{
		AggregateID:      event.AggregateRef.EntityID,
		AggregateType:    string(event.AggregateRef.EntityType),
		AggregateVersion: event.AggregateVersion, Body: cloneTestBytes(value.Event),
		CommandActorRef: command.ActorRef,
		GlobalSequence:  journal.global,
	})
	return JournalReceipt{
		AggregateVersion: event.AggregateVersion, Result: cloneTestBytes(value.Result),
	}, nil
}

func (journal *memoryJournal) AggregateEvents(
	_ context.Context, aggregateType, aggregateID string, after int64, limit int,
) ([]JournalEvent, error) {
	journal.mu.Lock()
	defer journal.mu.Unlock()
	var result []JournalEvent
	for _, event := range journal.events {
		if event.AggregateType == aggregateType && event.AggregateID == aggregateID &&
			event.AggregateVersion > after {
			result = append(result, cloneJournalEvent(event))
			if len(result) == limit {
				break
			}
		}
	}
	return result, nil
}

func (journal *memoryJournal) Events(
	_ context.Context, after int64, limit int,
) ([]JournalEvent, error) {
	journal.mu.Lock()
	defer journal.mu.Unlock()
	var result []JournalEvent
	for _, event := range journal.events {
		if event.GlobalSequence > after {
			result = append(result, cloneJournalEvent(event))
			if len(result) == limit {
				break
			}
		}
	}
	journal.globalRowsRead += len(result)
	return result, nil
}

func (journal *memoryJournal) appendHistory(event JournalEvent) {
	journal.mu.Lock()
	defer journal.mu.Unlock()
	journal.global++
	event.GlobalSequence = journal.global
	journal.events = append(journal.events, cloneJournalEvent(event))
}

func (journal *memoryJournal) commitCount() int {
	journal.mu.Lock()
	defer journal.mu.Unlock()
	return len(journal.commits)
}

func cloneCommit(value JournalCommit) JournalCommit {
	value.Command = cloneTestBytes(value.Command)
	value.Event = cloneTestBytes(value.Event)
	value.Result = cloneTestBytes(value.Result)
	return value
}

func cloneJournalEvent(value JournalEvent) JournalEvent {
	value.Body = cloneTestBytes(value.Body)
	return value
}

func cloneTestBytes(value []byte) []byte {
	return append([]byte(nil), value...)
}

func deterministicIdentity(serial *int) identitySource {
	return func(prefix string) (string, error) {
		(*serial)++
		return testID(prefix, 5000+*serial), nil
	}
}

func testID(prefix string, serial int) string {
	return fmt.Sprintf("%s_%026d", prefix, serial)
}
