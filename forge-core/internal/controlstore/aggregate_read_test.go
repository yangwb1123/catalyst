//go:build linux && !android

package controlstore

import (
	"context"
	"testing"
)

func TestAggregateEventReadsAreBoundedOrderedAndTyped(t *testing.T) {
	value := openTestStore(t)
	for _, request := range []CommitRequest{
		commitRequest(t, 0, 1, 2400),
		commitRequest(t, 1, 2, 2401),
		retargetCommit(t, commitRequest(t, 0, 3, 2402), "project"),
	} {
		if _, err := value.store.Commit(context.Background(), request); err != nil {
			t.Fatal(err)
		}
	}

	events, err := value.store.AggregateEvents(
		context.Background(), "space", testID("spc", 1), 0, 1,
	)
	if err != nil || len(events) != 1 || events[0].AggregateVersion != 1 {
		t.Fatalf("first aggregate page = %+v, %v", events, err)
	}
	events, err = value.store.AggregateEvents(
		context.Background(), "space", testID("spc", 1), 1, 10,
	)
	if err != nil || len(events) != 1 || events[0].AggregateVersion != 2 {
		t.Fatalf("second aggregate page = %+v, %v", events, err)
	}
}

func TestAggregateEventReadsRejectInvalidReferencesAndPages(t *testing.T) {
	value := openTestStore(t)
	tests := []struct {
		name string
		read func() error
	}{
		{"rust-owned", func() error {
			_, err := value.store.AggregateEvents(
				context.Background(), "attempt", testID("atm", 1), 0, 1,
			)
			return err
		}},
		{"mismatched-id", func() error {
			_, err := value.store.AggregateEvents(
				context.Background(), "space", testID("prj", 1), 0, 1,
			)
			return err
		}},
		{"bad-page", func() error {
			_, err := value.store.AggregateEvents(
				context.Background(), "space", testID("spc", 1), -1, 1,
			)
			return err
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if err := test.read(); err == nil {
				t.Fatal("invalid aggregate read succeeded")
			}
		})
	}
}
