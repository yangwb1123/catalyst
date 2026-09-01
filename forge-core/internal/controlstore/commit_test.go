//go:build linux && !android

package controlstore

import (
	"context"
	"errors"
	"strings"
	"sync"
	"testing"

	core "forgeos/forge-core/internal/platformcorecontract"
)

func TestCommitPersistsJournalOutboxAndIdempotentResultAcrossReopen(t *testing.T) {
	value := openTestStore(t)
	request := commitRequest(t, 0, 1, 1)
	request.Outbox = []OutboxMessage{{
		MessageID: testID("msg", 5001), SourceEvent: testID("evt", 1001),
		Destination: "runtime.local", Body: []byte(`{"start":true}`),
	}}
	receipt, err := value.store.Commit(context.Background(), request)
	if err != nil {
		t.Fatal(err)
	}
	if receipt.Replayed || receipt.AggregateVersion != 1 || receipt.FirstGlobalSequence != 1 || receipt.LastGlobalSequence != 1 {
		t.Fatalf("commit receipt = %+v", receipt)
	}
	request.Result[0] = 'x'
	receipt.Result[0] = 'x'
	assertFirstCommitState(t, value.store)
	value.close()
	store, root := reopenTestStore(t, value.path)
	defer func() { _ = store.Close(); _ = root.Close() }()
	replayRequest := commitRequest(t, 0, 1, 1)
	replayRequest.Outbox = []OutboxMessage{{
		MessageID: testID("msg", 5002), SourceEvent: testID("evt", 1001),
		Destination: "runtime.local", Body: []byte(`{"different":true}`),
	}}
	replayed, err := store.Commit(context.Background(), replayRequest)
	if err != nil || !replayed.Replayed || string(replayed.Result) != `{"accepted":1}` {
		t.Fatalf("replayed receipt = %+v, %v", replayed, err)
	}
	assertFirstCommitState(t, store)
}

func TestCommitReplayRejectsStoredRequestDigestDrift(t *testing.T) {
	value := openTestStore(t)
	request := commitRequest(t, 0, 1, 11)
	if _, err := value.store.Commit(context.Background(), request); err != nil {
		t.Fatal(err)
	}
	trigger := schemaDDL(t, "command_receipts_no_update")
	if _, err := value.store.db.Exec(`DROP TRIGGER command_receipts_no_update`); err != nil {
		t.Fatal(err)
	}
	if _, err := value.store.db.Exec(`UPDATE command_receipts SET request_bytes = X'78'`); err != nil {
		t.Fatal(err)
	}
	if _, err := value.store.db.Exec(trigger); err != nil {
		t.Fatal(err)
	}
	if _, err := value.store.Commit(context.Background(), request); !errors.Is(err, ErrCorruptStore) {
		t.Fatalf("stored request drift replay error = %v", err)
	}
}

func assertFirstCommitState(t *testing.T, store *Store) {
	t.Helper()
	head, err := store.ControlSourceHead(context.Background(), "control_plane")
	if err != nil || head != 1 {
		t.Fatalf("Control source head = %d, %v", head, err)
	}
	version, err := store.AggregateVersion(context.Background(), "space", testID("spc", 1))
	if err != nil || version != 1 {
		t.Fatalf("aggregate version = %d, %v", version, err)
	}
	events, err := store.Events(context.Background(), 0, 10)
	if err != nil || len(events) != 1 || events[0].GlobalSequence != 1 {
		t.Fatalf("events = %+v, %v", events, err)
	}
	if events[0].CommandActorID != testID("acr", 1) ||
		events[0].CommandActorType != "service" {
		t.Fatalf("originating command actor = %+v", events[0])
	}
	messages, err := store.PendingOutboxMessages(context.Background(), 0, 10)
	if err != nil || len(messages.Items) != 1 ||
		messages.Items[0].MessageID != testID("msg", 5001) ||
		messages.NextAfterOutboxSequence != 1 || messages.More {
		t.Fatalf("pending outbox = %+v, %v", messages, err)
	}
}

func TestCommitRejectsIdempotencyCommandVersionAndSequenceConflicts(t *testing.T) {
	value := openTestStore(t)
	first := commitRequest(t, 0, 1, 1)
	if _, err := value.store.Commit(context.Background(), first); err != nil {
		t.Fatal(err)
	}
	tests := []struct {
		name string
		req  CommitRequest
		want error
	}{
		{"idempotency-payload", changedCommand(t, first, 0, 1, "control-command-0001", "different"), ErrIdempotencyConflict},
		{"command-id", changedCommand(t, first, 1, 1, "control-command-0002", "advance"), ErrIdentifierConflict},
		{"version", commitRequest(t, 0, 2, 3), ErrVersionConflict},
		{"sequence-gap", commitRequest(t, 1, 3, 4), ErrSequenceConflict},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if _, err := value.store.Commit(context.Background(), test.req); !errors.Is(err, test.want) {
				t.Fatalf("commit error = %v, want %v", err, test.want)
			}
		})
	}
	events, err := value.store.Events(context.Background(), 0, 10)
	if err != nil || len(events) != 1 {
		t.Fatalf("failed commits changed journal: %d, %v", len(events), err)
	}
}

func TestCommitRejectsDurableEventAndOutboxIdentifierReuse(t *testing.T) {
	value := openTestStore(t)
	first := commitRequest(t, 0, 1, 2100)
	first.Outbox = []OutboxMessage{{
		MessageID: testID("msg", 9000), SourceEvent: testID("evt", 3100),
		Destination: "runtime", Body: []byte(`{"event":"first"}`),
	}}
	if _, err := value.store.Commit(context.Background(), first); err != nil {
		t.Fatal(err)
	}
	eventConflict := retargetCommit(t, commitRequest(t, 0, 2, 2101), "project")
	eventConflict.Events[0] = mutateCanonicalEvent(t, eventConflict.Events[0], func(event *core.EventEnvelope) {
		event.EventID = testID("evt", 3100)
		event.MessageID = testID("msg", 3100)
	})
	if _, err := value.store.Commit(context.Background(), eventConflict); !errors.Is(err, ErrIdentifierConflict) {
		t.Fatalf("event identifier reuse error = %v", err)
	}
	outboxConflict := retargetCommit(t, commitRequest(t, 0, 2, 2102), "objective")
	outboxConflict.Outbox = []OutboxMessage{{
		MessageID: testID("msg", 9000), SourceEvent: testID("evt", 3102),
		Destination: "runtime", Body: []byte(`{"event":"second"}`),
	}}
	if _, err := value.store.Commit(context.Background(), outboxConflict); !errors.Is(err, ErrIdentifierConflict) {
		t.Fatalf("outbox identifier reuse error = %v", err)
	}
}

func TestCommitRejectsEventMessageCollisionWithCommand(t *testing.T) {
	value := openTestStore(t)
	if _, err := value.store.Commit(context.Background(), commitRequest(t, 0, 1, 2150)); err != nil {
		t.Fatal(err)
	}
	request := retargetCommit(t, commitRequest(t, 0, 2, 2151), "project")
	request.Events[0] = mutateCanonicalEvent(t, request.Events[0], func(event *core.EventEnvelope) {
		event.EventID = testID("evt", 2150)
		event.MessageID = testID("msg", 2150)
	})
	if _, err := value.store.Commit(context.Background(), request); !errors.Is(err, ErrIdentifierConflict) {
		t.Fatalf("event message/command collision error = %v", err)
	}
}

func TestCommitRejectsOutboxMessageCollisionAcrossIdentityDomain(t *testing.T) {
	value := openTestStore(t)
	sameCommit := commitRequest(t, 0, 1, 2160)
	sameCommit.Outbox = []OutboxMessage{{
		MessageID: testID("msg", 2160), SourceEvent: testID("evt", 3160),
		Destination: "runtime", Body: []byte(`{"same":true}`),
	}}
	if _, err := value.store.Commit(context.Background(), sameCommit); err == nil {
		t.Fatal("outbox reused the command message identity in one commit")
	}
	eventCollision := commitRequest(t, 0, 1, 2164)
	eventCollision.Outbox = []OutboxMessage{{
		MessageID: testID("msg", 3164), SourceEvent: testID("evt", 3164),
		Destination: "runtime", Body: []byte(`{"event":true}`),
	}}
	if _, err := value.store.Commit(context.Background(), eventCollision); err == nil {
		t.Fatal("outbox reused an event message identity in one commit")
	}

	first := commitRequest(t, 0, 1, 2161)
	first.Outbox = []OutboxMessage{{
		MessageID: testID("msg", 2162), SourceEvent: testID("evt", 3161),
		Destination: "runtime", Body: []byte(`{"first":true}`),
	}}
	if _, err := value.store.Commit(context.Background(), first); err != nil {
		t.Fatal(err)
	}
	commandCollision := retargetCommit(t, commitRequest(t, 0, 2, 2162), "project")
	if _, err := value.store.Commit(context.Background(), commandCollision); !errors.Is(err, ErrIdentifierConflict) {
		t.Fatalf("command/outbox durable collision error = %v", err)
	}

	outboxCollision := retargetCommit(t, commitRequest(t, 0, 2, 2163), "objective")
	outboxCollision.Outbox = []OutboxMessage{{
		MessageID: testID("msg", 2161), SourceEvent: testID("evt", 3163),
		Destination: "runtime", Body: []byte(`{"second":true}`),
	}}
	if _, err := value.store.Commit(context.Background(), outboxCollision); !errors.Is(err, ErrIdentifierConflict) {
		t.Fatalf("outbox/command durable collision error = %v", err)
	}
}

func TestControlSourceSequencesAreIndependentFromGlobalJournal(t *testing.T) {
	value := openTestStore(t)
	requests := []CommitRequest{
		commitRequest(t, 0, 1, 51),
		commitRequest(t, 1, 1, 52),
		commitRequest(t, 2, 2, 53),
	}
	requests[1].Events = [][]byte{canonicalEvent(t, 2, 1, 52, "app_server", "advanced")}
	for _, request := range requests {
		if _, err := value.store.Commit(context.Background(), request); err != nil {
			t.Fatal(err)
		}
	}
	events, err := value.store.Events(context.Background(), 0, 10)
	if err != nil || len(events) != 3 {
		t.Fatalf("interleaved Control events = %+v, %v", events, err)
	}
	wantSources := []string{"control_plane", "app_server", "control_plane"}
	wantSequences := []int64{1, 1, 2}
	for index, event := range events {
		if event.GlobalSequence != int64(index+1) || event.SourceComponent != wantSources[index] ||
			event.SourceSequence != wantSequences[index] {
			t.Fatalf("event %d positions = %+v", index, event)
		}
	}
}

func TestCommitPersistsAndReplaysEmptyResultBlob(t *testing.T) {
	value := openTestStore(t)
	request := commitRequest(t, 0, 1, 54)
	request.Result = nil
	receipt, err := value.store.Commit(context.Background(), request)
	if err != nil || receipt.Result == nil || len(receipt.Result) != 0 {
		t.Fatalf("empty result receipt = %+v, %v", receipt, err)
	}
	value.close()
	store, root := reopenTestStore(t, value.path)
	defer func() { _ = store.Close(); _ = root.Close() }()
	replayed, err := store.Commit(context.Background(), request)
	if err != nil || !replayed.Replayed || replayed.Result == nil || len(replayed.Result) != 0 {
		t.Fatalf("empty result replay = %+v, %v", replayed, err)
	}
}

func TestCommitRejectsBrokenEventCausation(t *testing.T) {
	for _, test := range []struct {
		name   string
		mutate func(*core.EventEnvelope)
	}{
		{"missing", func(event *core.EventEnvelope) { event.CausationID = nil }},
		{"unknown", func(event *core.EventEnvelope) { cause := testID("msg", 9999); event.CausationID = &cause }},
		{"correlation", func(event *core.EventEnvelope) { event.CorrelationID = testID("cor", 9999) }},
		{"time", func(event *core.EventEnvelope) { event.OccurredAtUnixMS = testUnixMS - 1 }},
	} {
		t.Run(test.name, func(t *testing.T) {
			value := openTestStore(t)
			request := commitRequest(t, 0, 1, 60)
			request.Events[0] = mutateCanonicalEvent(t, request.Events[0], test.mutate)
			if _, err := value.store.Commit(context.Background(), request); err == nil {
				t.Fatal("broken event causation succeeded")
			}
		})
	}
}

func TestCommandCausationResolvesDurableMessageIndex(t *testing.T) {
	value := openTestStore(t)
	if _, err := value.store.Commit(context.Background(), commitRequest(t, 0, 1, 80)); err != nil {
		t.Fatal(err)
	}
	cause := testID("msg", 1080)
	correlation := testID("cor", 80)
	request := commitRequest(t, 1, 2, 81)
	request.Command = mutateCanonicalCommand(t, request.Command, func(command *core.CommandEnvelope) {
		command.CausationID = &cause
		command.CorrelationID = correlation
		command.IssuedAtUnixMS = testUnixMS + 2
	})
	request.Events[0] = mutateCanonicalEvent(t, request.Events[0], func(event *core.EventEnvelope) {
		event.CorrelationID = correlation
	})
	if _, err := value.store.Commit(context.Background(), request); err != nil {
		t.Fatalf("durably caused command: %v", err)
	}
	unknown := testID("msg", 9999)
	rejected := commitRequest(t, 2, 3, 82)
	rejected.Command = mutateCanonicalCommand(t, rejected.Command, func(command *core.CommandEnvelope) {
		command.CausationID = &unknown
		command.IssuedAtUnixMS = testUnixMS + 3
	})
	if _, err := value.store.Commit(context.Background(), rejected); err == nil {
		t.Fatal("command with unresolved durable cause succeeded")
	}
}

func TestCommitRejectsRustOwnedAggregateTargets(t *testing.T) {
	for index, aggregateType := range []string{"attempt", "session", "turn", "action"} {
		t.Run(aggregateType, func(t *testing.T) {
			value := openTestStore(t)
			request := retargetCommit(t, commitRequest(t, 0, 1, 70+index), aggregateType)
			_, err := value.store.Commit(context.Background(), request)
			if err == nil || !strings.Contains(err.Error(), "not owned by Go Control") {
				t.Fatalf("Rust-owned target error = %v", err)
			}
		})
	}
}

func changedCommand(
	t *testing.T,
	request CommitRequest,
	version int64,
	serial int,
	key, operation string,
) CommitRequest {
	t.Helper()
	request.Command = canonicalCommand(t, version, serial, key, operation)
	request.Events = [][]byte{canonicalEvent(t, version+1, 2, serial, "control_plane", "advanced")}
	return request
}

func TestCommitRejectsAggregateVersionAndBatchSequenceReorderBeforeDurability(t *testing.T) {
	value := openTestStore(t)
	key := "control-command-batch-1"
	command := canonicalCommand(t, 0, 10, key, "batch")
	commandCause := testID("msg", 10)
	correlation := testID("cor", 10)
	tests := []struct {
		name   string
		events [][]byte
		want   error
	}{
		{"aggregate-gap", [][]byte{canonicalEvent(t, 2, 1, 10, "control_plane", "bad")}, nil},
		{"source-gap", [][]byte{
			canonicalEventWithLinks(t, 1, 1, 10, "control_plane", "one", correlation, &commandCause),
			canonicalEventWithLinks(t, 2, 3, 11, "control_plane", "three", correlation, &commandCause),
		}, ErrSequenceConflict},
		{"source-reorder", [][]byte{
			canonicalEventWithLinks(t, 1, 2, 10, "control_plane", "two", correlation, &commandCause),
			canonicalEventWithLinks(t, 2, 1, 11, "control_plane", "one", correlation, &commandCause),
		}, ErrSequenceConflict},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			_, err := value.store.Commit(context.Background(), CommitRequest{
				Command: command, Events: test.events, Result: nil, CommittedAtUnixMS: testUnixMS,
			})
			if test.want == nil {
				if err == nil {
					t.Fatal("invalid aggregate version succeeded")
				}
			} else if !errors.Is(err, test.want) {
				t.Fatalf("batch error = %v, want %v", err, test.want)
			}
		})
	}
	events, err := value.store.Events(context.Background(), 0, 10)
	if err != nil || len(events) != 0 {
		t.Fatalf("rejected batch persisted events: %d, %v", len(events), err)
	}
}

func TestCommitRollsBackEventHeadAndReceiptWhenOutboxInsertFails(t *testing.T) {
	value := openTestStore(t)
	if _, err := value.store.db.Exec(`CREATE TEMP TRIGGER inject_outbox_failure BEFORE INSERT ON outbox_messages BEGIN SELECT RAISE(ABORT, 'injected'); END`); err != nil {
		t.Fatal(err)
	}
	request := commitRequest(t, 0, 1, 20)
	request.Outbox = []OutboxMessage{{
		MessageID: testID("msg", 5020), SourceEvent: testID("evt", 1020),
		Destination: "runtime.local", Body: []byte(`{"start":true}`),
	}}
	if _, err := value.store.Commit(context.Background(), request); err == nil {
		t.Fatal("injected outbox failure succeeded")
	}
	version, err := value.store.AggregateVersion(context.Background(), "space", testID("spc", 1))
	if err != nil || version != 0 {
		t.Fatalf("rolled-back aggregate version = %d, %v", version, err)
	}
	events, err := value.store.Events(context.Background(), 0, 10)
	if err != nil || len(events) != 0 {
		t.Fatalf("rolled-back events = %d, %v", len(events), err)
	}
	if _, err := value.store.db.Exec(`DROP TRIGGER temp.inject_outbox_failure`); err != nil {
		t.Fatal(err)
	}
	if _, err := value.store.Commit(context.Background(), request); err != nil {
		t.Fatalf("retry after rollback: %v", err)
	}
}

func TestConcurrentExpectedVersionHasOneWinner(t *testing.T) {
	value := openTestStore(t)
	requests := []CommitRequest{commitRequest(t, 0, 1, 31), commitRequest(t, 0, 1, 32)}
	errorsFound := make(chan error, len(requests))
	var wait sync.WaitGroup
	for _, request := range requests {
		request := request
		wait.Add(1)
		go func() {
			defer wait.Done()
			_, err := value.store.Commit(context.Background(), request)
			errorsFound <- err
		}()
	}
	wait.Wait()
	close(errorsFound)
	successes, conflicts := 0, 0
	for err := range errorsFound {
		if err == nil {
			successes++
		} else if errors.Is(err, ErrVersionConflict) {
			conflicts++
		} else {
			t.Fatalf("concurrent commit error = %v", err)
		}
	}
	if successes != 1 || conflicts != 1 {
		t.Fatalf("concurrent results success=%d conflict=%d", successes, conflicts)
	}
}

func TestOutboxAcknowledgementIsAppendOnlyAndIdempotent(t *testing.T) {
	value := openTestStore(t)
	request := commitRequest(t, 0, 1, 40)
	messageID := testID("msg", 5040)
	request.Outbox = []OutboxMessage{{
		MessageID: messageID, SourceEvent: testID("evt", 1040),
		Destination: "runtime.local", Body: []byte(`{"start":true}`),
	}}
	if _, err := value.store.Commit(context.Background(), request); err != nil {
		t.Fatal(err)
	}
	if replayed, err := value.store.AcknowledgeOutbox(context.Background(), messageID, testUnixMS+1); err != nil || replayed {
		t.Fatalf("first acknowledgement = %t, %v", replayed, err)
	}
	if replayed, err := value.store.AcknowledgeOutbox(context.Background(), messageID, testUnixMS+2); err != nil || !replayed {
		t.Fatalf("replayed acknowledgement = %t, %v", replayed, err)
	}
	pending, err := value.store.PendingOutboxMessages(context.Background(), 0, 10)
	if err != nil || len(pending.Items) != 0 ||
		pending.NextAfterOutboxSequence != 1 || pending.More {
		t.Fatalf("acknowledged pending page = %+v, %v", pending, err)
	}
	if _, err := value.store.db.Exec(`UPDATE outbox_acknowledgements SET delivered_at_unix_ms = 1`); err == nil {
		t.Fatal("append-only acknowledgement was updated")
	}
}
