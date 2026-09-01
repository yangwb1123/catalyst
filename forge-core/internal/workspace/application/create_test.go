package application

import (
	"bytes"
	"context"
	"errors"
	"testing"

	core "forgeos/forge-core/internal/platformcorecontract"
)

func TestCreateSpaceEmitsExactCanonicalCommandAndEvent(t *testing.T) {
	journal := &memoryJournal{}
	service := testService(journal)
	request := testSpaceRequest(1)
	space, err := service.CreateSpace(context.Background(), request)
	if err != nil || space.SpaceID != request.SpaceID || space.Name != request.Name {
		t.Fatalf("CreateSpace = %+v, %v", space, err)
	}
	if journal.commitCount() != 1 {
		t.Fatalf("commit count = %d", journal.commitCount())
	}
	command, err := core.DecodeCanonicalCommandEnvelope(journal.commits[0].Command)
	if err != nil {
		t.Fatal(err)
	}
	event, err := core.DecodeCanonicalEventEnvelope(journal.commits[0].Event)
	if err != nil {
		t.Fatal(err)
	}
	if command.SchemaName != "forge.workspace.create_space" ||
		event.SchemaName != "forge.workspace.space_created" || event.Sequence != 1 ||
		event.CausationID == nil || *event.CausationID != command.MessageID ||
		event.CorrelationID != command.CorrelationID || event.ActorRef != command.ActorRef {
		t.Fatalf("command/event relation = %+v / %+v", command, event)
	}
	expectedResult, err := canonicalResult(command.TargetRef)
	if err != nil || !bytes.Equal(journal.commits[0].Result, expectedResult) {
		t.Fatalf("replayable result = %q, %v", journal.commits[0].Result, err)
	}
}

func TestCreateRetriesOnlySourceSequenceConflictWithStableEventIdentity(t *testing.T) {
	journal := &memoryJournal{sequenceConflicts: 1}
	service := testService(journal)
	if _, err := service.CreateSpace(context.Background(), testSpaceRequest(2)); err != nil {
		t.Fatal(err)
	}
	if journal.commitCount() != 2 {
		t.Fatalf("commit count = %d", journal.commitCount())
	}
	first, _ := core.DecodeCanonicalEventEnvelope(journal.commits[0].Event)
	second, _ := core.DecodeCanonicalEventEnvelope(journal.commits[1].Event)
	if first.EventID != second.EventID || first.Sequence != 1 || second.Sequence != 2 {
		t.Fatalf("retry events = %+v / %+v", first, second)
	}
}

func TestCreateStopsAfterBoundedSourceSequenceRetries(t *testing.T) {
	journal := &memoryJournal{sequenceConflicts: maxCommitAttempts}
	service := testService(journal)
	if _, err := service.CreateSpace(
		context.Background(), testSpaceRequest(3),
	); !errors.Is(err, ErrSourceSequenceConflict) {
		t.Fatalf("retry exhaustion error = %v", err)
	}
	if journal.commitCount() != maxCommitAttempts || len(journal.events) != 0 {
		t.Fatalf("commits/events = %d/%d", journal.commitCount(), len(journal.events))
	}
}

func TestInvalidWorkspaceCommandsCommitNothing(t *testing.T) {
	tests := []struct {
		name string
		run  func(*Service) error
	}{
		{"expected-version", func(service *Service) error {
			request := testSpaceRequest(10)
			request.Meta.ExpectedVersion = 1
			_, err := service.CreateSpace(context.Background(), request)
			return err
		}},
		{"relative-path", func(service *Service) error {
			request := testProjectRequest(11, 1)
			request.RootPath = "relative"
			_, err := service.RegisterProject(context.Background(), request)
			return err
		}},
		{"bad-observation", func(service *Service) error {
			request := testSnapshotRequest(12, 1, 1)
			request.ObservationRef.RecordSHA256 = "bad"
			_, err := service.RecordProjectSnapshot(context.Background(), request)
			return err
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			journal := &memoryJournal{}
			if err := test.run(testService(journal)); !errors.Is(err, ErrInvalidInput) {
				t.Fatalf("error = %v", err)
			}
			if journal.commitCount() != 0 {
				t.Fatal("invalid command reached commit")
			}
		})
	}
}

func TestMalformedIdentitySourceReturnsErrorWithoutPanic(t *testing.T) {
	journal := &memoryJournal{}
	service, err := newService(journal, func(string) (string, error) {
		return "bad", nil
	}, func() int64 { return testUnixMS })
	if err != nil {
		t.Fatal(err)
	}
	if _, err := service.CreateSpace(context.Background(), testSpaceRequest(20)); err == nil {
		t.Fatal("malformed identity source succeeded")
	}
	if journal.commitCount() != 0 {
		t.Fatal("malformed identity reached commit")
	}
}

func TestReceiptMustMatchExactReplayableResult(t *testing.T) {
	target := core.EntityRef{EntityID: testID("spc", 70), EntityType: "space"}
	expected, err := canonicalResult(target)
	if err != nil {
		t.Fatal(err)
	}
	if err := validateReceipt(JournalReceipt{
		AggregateVersion: 1, Result: expected,
	}, expected); err != nil {
		t.Fatalf("valid receipt = %v", err)
	}
	for _, receipt := range []JournalReceipt{
		{AggregateVersion: 2, Result: expected},
		{AggregateVersion: 1, Result: []byte("different")},
	} {
		if err := validateReceipt(receipt, expected); !errors.Is(err, ErrInvalidHistory) {
			t.Fatalf("mismatched receipt error = %v", err)
		}
	}
}
