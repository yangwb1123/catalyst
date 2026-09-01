package application

import (
	"context"
	"errors"
	"testing"

	core "forgeos/forge-core/internal/platformcorecontract"
)

func TestListRejectsExtraHistoryBeforeReturningFirstItem(t *testing.T) {
	journal := &memoryJournal{}
	service := testService(journal)
	if _, err := service.CreateSpace(context.Background(), testSpaceRequest(60)); err != nil {
		t.Fatal(err)
	}
	extra := cloneJournalEvent(journal.events[0])
	extra.AggregateVersion = 2
	journal.appendHistory(extra)
	page, err := service.ListSpaces(context.Background(), 0, 1)
	if !errors.Is(err, ErrInvalidHistory) {
		t.Fatalf("list history error = %v", err)
	}
	assertZeroPage(t, page, 0)
}

func TestProjectReadRejectsMissingDurableSpace(t *testing.T) {
	journal := &memoryJournal{}
	service := testService(journal)
	if _, err := service.CreateSpace(context.Background(), testSpaceRequest(1)); err != nil {
		t.Fatal(err)
	}
	request := testProjectRequest(61, 1)
	if _, err := service.RegisterProject(context.Background(), request); err != nil {
		t.Fatal(err)
	}
	removeJournalType(journal, "space")
	if _, err := service.Project(context.Background(), request.ProjectID); !errors.Is(err, ErrInvalidHistory) {
		t.Fatalf("orphan Project error = %v", err)
	}
}

func TestSnapshotReadAndListRejectCrossSpaceHistory(t *testing.T) {
	journal := &memoryJournal{}
	service := testService(journal)
	for _, serial := range []int{1, 2} {
		if _, err := service.CreateSpace(context.Background(), testSpaceRequest(serial)); err != nil {
			t.Fatal(err)
		}
	}
	if _, err := service.RegisterProject(context.Background(), testProjectRequest(62, 1)); err != nil {
		t.Fatal(err)
	}
	request := testSnapshotRequest(63, 62, 1)
	if _, err := service.RecordProjectSnapshot(context.Background(), request); err != nil {
		t.Fatal(err)
	}
	rewriteSnapshotSpace(t, journal, request.ProjectSnapshotID, testID("spc", 2))
	if _, err := service.ProjectSnapshot(
		context.Background(), request.ProjectSnapshotID,
	); !errors.Is(err, ErrInvalidHistory) {
		t.Fatalf("cross-Space Snapshot error = %v", err)
	}
	page, err := service.ListProjectSnapshots(context.Background(), request.ProjectID, 0, 10)
	if !errors.Is(err, ErrInvalidHistory) {
		t.Fatalf("cross-Space Snapshot list error = %v", err)
	}
	assertZeroPage(t, page, 0)
}

func TestListErrorReturnsNoPartialPage(t *testing.T) {
	journal := &continuationFailureJournal{memoryJournal: &memoryJournal{}}
	service := testService(journal)
	if _, err := service.CreateSpace(context.Background(), testSpaceRequest(64)); err != nil {
		t.Fatal(err)
	}
	page, err := service.ListSpaces(context.Background(), 0, 1)
	if err == nil {
		t.Fatal("continuation failure succeeded")
	}
	assertZeroPage(t, page, 0)
}

func TestListReadsOnlyOneContinuationLookahead(t *testing.T) {
	journal := &lookaheadBoundaryJournal{memoryJournal: &memoryJournal{}}
	service := testService(journal)
	if _, err := service.CreateSpace(context.Background(), testSpaceRequest(67)); err != nil {
		t.Fatal(err)
	}
	page, err := service.ListSpaces(context.Background(), 0, 1)
	if err != nil || len(page.Items) != 1 || !page.More || journal.reads != 2 {
		t.Fatalf("lookahead page/reads = %+v/%d, %v", page, journal.reads, err)
	}
}

func TestListScanBudgetCountsAllGlobalEvents(t *testing.T) {
	journal := &memoryJournal{global: maxListScan}
	for sequence := 1; sequence <= maxListScan; sequence++ {
		journal.events = append(journal.events, JournalEvent{
			AggregateID: testID("run", sequence), AggregateType: "run",
			GlobalSequence: int64(sequence),
		})
	}
	service := testService(journal)
	if _, err := service.CreateSpace(context.Background(), testSpaceRequest(65)); err != nil {
		t.Fatal(err)
	}
	first, err := service.ListSpaces(context.Background(), 0, 1)
	if err != nil || len(first.Items) != 0 || !first.More ||
		first.NextAfterGlobalSequence != maxListScan {
		t.Fatalf("bounded scan page = %+v, %v", first, err)
	}
	if journal.globalRowsRead != maxListRead {
		t.Fatalf("global event rows read = %d", journal.globalRowsRead)
	}
	second, err := service.ListSpaces(context.Background(), first.NextAfterGlobalSequence, 1)
	if err != nil || len(second.Items) != 1 || second.More {
		t.Fatalf("post-budget page = %+v, %v", second, err)
	}
}

func TestParentListErrorsPreserveNonzeroCursor(t *testing.T) {
	service := testService(&memoryJournal{})
	const after = int64(73)
	projects, err := service.ListProjects(context.Background(), "bad", after, 1)
	if !errors.Is(err, ErrInvalidInput) {
		t.Fatalf("malformed parent error = %v", err)
	}
	assertZeroPage(t, projects, after)
	projects, err = service.ListProjects(context.Background(), testID("spc", 99), after, 1)
	if !errors.Is(err, ErrNotFound) {
		t.Fatalf("missing parent error = %v", err)
	}
	assertZeroPage(t, projects, after)
	snapshots, err := service.ListProjectSnapshots(
		context.Background(), testID("prj", 99), after, 1,
	)
	if !errors.Is(err, ErrNotFound) {
		t.Fatalf("missing Snapshot parent error = %v", err)
	}
	assertZeroPage(t, snapshots, after)
}

func TestCorruptParentListErrorPreservesNonzeroCursor(t *testing.T) {
	journal := &memoryJournal{}
	service := testService(journal)
	request := testSpaceRequest(66)
	if _, err := service.CreateSpace(context.Background(), request); err != nil {
		t.Fatal(err)
	}
	journal.mu.Lock()
	event, err := core.DecodeCanonicalEventEnvelope(journal.events[0].Body)
	if err != nil {
		journal.mu.Unlock()
		t.Fatal(err)
	}
	event.Payload["unexpected"] = true
	journal.events[0].Body, err = core.CanonicalEventEnvelopeJSON(event)
	journal.mu.Unlock()
	if err != nil {
		t.Fatal(err)
	}
	const after = int64(73)
	page, err := service.ListProjects(context.Background(), request.SpaceID, after, 1)
	if !errors.Is(err, ErrInvalidHistory) {
		t.Fatalf("corrupt parent error = %v", err)
	}
	assertZeroPage(t, page, after)
}

func TestListRejectsInvalidBounds(t *testing.T) {
	service := testService(&memoryJournal{})
	for _, value := range []struct {
		after int64
		limit int
	}{{-1, 1}, {0, 0}, {0, maxListLimit + 1}} {
		if _, err := service.ListSpaces(
			context.Background(), value.after, value.limit,
		); !errors.Is(err, ErrInvalidInput) {
			t.Fatalf("bounds %+v error = %v", value, err)
		}
	}
}

type continuationFailureJournal struct {
	*memoryJournal
	reads int
}

type lookaheadBoundaryJournal struct {
	*memoryJournal
	reads int
}

func (journal *lookaheadBoundaryJournal) Events(
	ctx context.Context, after int64, limit int,
) ([]JournalEvent, error) {
	journal.reads++
	if journal.reads > 2 {
		return nil, errors.New("read beyond one lookahead")
	}
	if journal.reads == 2 {
		return []JournalEvent{{
			AggregateID: testID("run", 1), AggregateType: "run", GlobalSequence: after + 1,
		}}, nil
	}
	return journal.memoryJournal.Events(ctx, after, limit)
}

func (journal *continuationFailureJournal) Events(
	ctx context.Context, after int64, limit int,
) ([]JournalEvent, error) {
	journal.reads++
	if journal.reads == 2 {
		return nil, errors.New("injected continuation failure")
	}
	stored, err := journal.memoryJournal.Events(ctx, after, limit)
	if err != nil || len(stored) == 0 {
		return stored, err
	}
	page := make([]JournalEvent, journalPageLimit)
	for index := 0; index < journalPageLimit-1; index++ {
		page[index] = JournalEvent{
			AggregateID: testID("run", index+1), AggregateType: "run",
			GlobalSequence: int64(index + 1),
		}
	}
	page[journalPageLimit-1] = stored[0]
	page[journalPageLimit-1].GlobalSequence = journalPageLimit
	return page, nil
}

func rewriteSnapshotSpace(
	t *testing.T, journal *memoryJournal, snapshotID, spaceID string,
) {
	t.Helper()
	journal.mu.Lock()
	defer journal.mu.Unlock()
	for index := range journal.events {
		stored := &journal.events[index]
		if stored.AggregateID != snapshotID {
			continue
		}
		event, err := core.DecodeCanonicalEventEnvelope(stored.Body)
		if err != nil {
			t.Fatal(err)
		}
		event.ScopeRef.SpaceID = spaceID
		event.Payload["space_id"] = spaceID
		stored.Body, err = core.CanonicalEventEnvelopeJSON(event)
		if err != nil {
			t.Fatal(err)
		}
		return
	}
	t.Fatal("Snapshot event not found")
}

func removeJournalType(journal *memoryJournal, aggregateType string) {
	journal.mu.Lock()
	defer journal.mu.Unlock()
	kept := journal.events[:0]
	for _, event := range journal.events {
		if event.AggregateType != aggregateType {
			kept = append(kept, event)
		}
	}
	journal.events = kept
}

func assertZeroPage[T any](t *testing.T, page Page[T], after int64) {
	t.Helper()
	if len(page.Items) != 0 || page.More || page.NextAfterGlobalSequence != after {
		t.Fatalf("nonzero error page = %+v", page)
	}
}
