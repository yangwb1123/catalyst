package main

import (
	"fmt"
	"strings"
	"testing"

	"forgeos/forge-core/internal/memory"
)

// seedMemory writes entries to a temp repo's store and returns the repo root. Each
// entry's Iteration is its 1-based position so recency (largest Iteration) is the last
// appended — the realistic shape of the append-only evolve log.
func seedMemory(t *testing.T, entries []memory.Entry) string {
	t.Helper()
	root := t.TempDir()
	for _, e := range entries {
		if err := memory.Append(memoryPath(root), e); err != nil {
			t.Fatalf("seed memory: %v", err)
		}
	}
	return root
}

// countMemoryLines counts the rendered "- [kind] ..." entry lines in a memoryContext
// block, the observable proxy for "how many entries were injected into the prompt".
func countMemoryLines(block string) int {
	return strings.Count(block, "\n- [")
}

// ★ THE unbounded-gap proof ★ (symmetric to D3's stash proof). A store of memoryCap+EXTRA
// entries is the long-run shape: the append-only evolve log grows past any fixed ceiling.
// The OLD memoryContext called memory.Query(entries,"","") and injected EVERY entry — so
// this same input would have put ALL memoryCap+EXTRA lines in the prompt (verified here
// against the exact old expression), eventually overrunning the context window. The NEW
// path injects at most memoryCap. The gap between the two counts is the bug this PR fixes:
// the bound DISCARDS entries the old code would have force-injected.
func TestMemoryContext_BoundsAnUnboundedStore(t *testing.T) {
	const extra = 10
	var entries []memory.Entry
	for i := 1; i <= memoryCap+extra; i++ {
		entries = append(entries, memory.Entry{
			Kind:      memory.KindLesson,
			Topic:     "evolve",
			Detail:    fmt.Sprintf("iteration %d trajectory", i),
			Iteration: i, CreatedAtUnix: int64(i),
		})
	}
	root := seedMemory(t, entries)

	// What the OLD unbounded code path would inject: literally memory.Query(es,"","")
	// (a copy of ALL entries) — the pre-fix behavior, reproduced to prove the gap was real.
	loaded, err := memory.Load(memoryPath(root))
	if err != nil {
		t.Fatalf("load: %v", err)
	}
	unboundedWouldInject := len(memory.Query(loaded, "", ""))
	if unboundedWouldInject != memoryCap+extra {
		t.Fatalf("precondition: the old unbounded path would inject all %d entries; got %d", memoryCap+extra, unboundedWouldInject)
	}

	// The NEW bounded path: at most memoryCap lines, strictly fewer than the unbounded set.
	got := memoryContext(root, "evolve implementer")
	if len(got) != 1 {
		t.Fatalf("a non-empty store must yield exactly one memory block; got %d blocks", len(got))
	}
	n := countMemoryLines(got[0])
	if n != memoryCap {
		t.Errorf("bounded memory must inject exactly memoryCap=%d entries over the cap; got %d", memoryCap, n)
	}
	if n >= unboundedWouldInject {
		t.Errorf("the bound must DISCARD entries the old path injected: bounded=%d, unbounded=%d", n, unboundedWouldInject)
	}
}

// Byte-compat: a store AT/UNDER the cap is injected WHOLE, in input order, byte-for-byte
// identical to the pre-fix render. This is the back-compat guarantee — small/typical
// projects see exactly the old prompt. Asserted by reproducing the exact old format.
func TestMemoryContext_UnderCapIsByteForByteUnchanged(t *testing.T) {
	entries := []memory.Entry{
		{Kind: memory.KindGap, Topic: "build", Detail: "missing retry on flaky gate", Iteration: 1, CreatedAtUnix: 1},
		{Kind: memory.KindDecision, Topic: "build", Detail: "chose JSONL for the memory store", Iteration: 2, CreatedAtUnix: 2},
		{Kind: memory.KindLesson, Topic: "evolve", Detail: "backoff beats abort on 529", Iteration: 3, CreatedAtUnix: 3},
	}
	root := seedMemory(t, entries)

	// Reconstruct the EXACT pre-fix output: header + every entry in input order.
	var want strings.Builder
	want.WriteString("Project memory (gaps / decisions / lessons from prior iterations):")
	for _, e := range entries {
		fmt.Fprintf(&want, "\n- [%s] %s — %s (iter %d)", e.Kind, e.Topic, e.Detail, e.Iteration)
	}

	got := memoryContext(root, "anything goes here")
	if len(got) != 1 || got[0] != want.String() {
		t.Errorf("under-cap render must be byte-for-byte the old output.\n got: %q\nwant: %q", got, want.String())
	}
}

// Recency floor: over the cap, the memoryRecencyFloor newest entries (largest Iteration)
// are ALWAYS present — even when their Detail shares NO term with the query, so pure
// relevance would have dropped them. This pins "the latest lessons are never lost".
func TestBoundMemory_RecencyFloorAlwaysKeepsNewest(t *testing.T) {
	var entries []memory.Entry
	// Old entries (1..cap) match the query term "alpha"; newest ones use a disjoint term.
	for i := 1; i <= memoryCap; i++ {
		entries = append(entries, memory.Entry{Kind: memory.KindGap, Topic: "alpha", Detail: "alpha topic", Iteration: i})
	}
	for i := memoryCap + 1; i <= memoryCap+memoryRecencyFloor; i++ {
		entries = append(entries, memory.Entry{Kind: memory.KindLesson, Topic: "omega", Detail: "omega lesson", Iteration: i})
	}
	got := boundMemory(entries, "alpha") // query favors the OLD entries on relevance.

	kept := map[int]bool{}
	for _, e := range got {
		kept[e.Iteration] = true
	}
	for i := memoryCap + 1; i <= memoryCap+memoryRecencyFloor; i++ {
		if !kept[i] {
			t.Errorf("recency floor breached: newest iteration %d (query-irrelevant) was dropped", i)
		}
	}
	if len(got) != memoryCap {
		t.Errorf("bounded set must be exactly memoryCap=%d; got %d", memoryCap, len(got))
	}
}

// Relevance: over the cap, a strongly-matching OLD entry (not in the recency floor) is
// still SELECTED — the bound keeps a still-pertinent old decision, it does not blindly
// drop everything old. This is the "hour 8 must still see hour 2's relevant wall" case.
func TestBoundMemory_RelevanceKeepsMatchingOldEntry(t *testing.T) {
	var entries []memory.Entry
	// One distinctive old entry at iteration 1, then filler, then a recent block.
	entries = append(entries, memory.Entry{Kind: memory.KindDecision, Topic: "zookeeper", Detail: "quorum sizing rationale", Iteration: 1})
	for i := 2; i <= memoryCap+memoryRecencyFloor; i++ {
		entries = append(entries, memory.Entry{Kind: memory.KindLesson, Topic: "filler", Detail: "unrelated note", Iteration: i})
	}
	got := boundMemory(entries, "zookeeper quorum") // strongly matches the iteration-1 entry.

	var foundOld bool
	for _, e := range got {
		if e.Iteration == 1 {
			foundOld = true
		}
	}
	if !foundOld {
		t.Error("a strongly query-relevant OLD entry must survive the bound (not be dropped for being old)")
	}
}

// Dedup: when a recent entry ALSO matches the query (so it is in both the recency floor
// AND the relevance pick), it appears exactly ONCE — the floor∪relevance union is
// de-duplicated, never double-injected.
func TestBoundMemory_DedupsFloorAndRelevanceOverlap(t *testing.T) {
	var entries []memory.Entry
	for i := 1; i <= memoryCap; i++ {
		entries = append(entries, memory.Entry{Kind: memory.KindGap, Topic: "filler", Detail: "noise", Iteration: i})
	}
	// Newest entries ALSO carry the query term, so they are both recent AND relevant.
	for i := memoryCap + 1; i <= memoryCap+memoryRecencyFloor; i++ {
		entries = append(entries, memory.Entry{Kind: memory.KindLesson, Topic: "overlap", Detail: "overlap signal", Iteration: i})
	}
	got := boundMemory(entries, "overlap signal")

	seen := map[int]int{}
	for _, e := range got {
		seen[e.Iteration]++
	}
	for it, c := range seen {
		if c != 1 {
			t.Errorf("iteration %d injected %d times; floor∪relevance must dedup to exactly 1", it, c)
		}
	}
	if len(got) != memoryCap {
		t.Errorf("a deduped bounded set must still be exactly memoryCap=%d; got %d", memoryCap, len(got))
	}
}

// Display order: the bounded set is rendered in Iteration-ASCENDING order (a coherent
// timeline), NOT relevance order — so the agent reads memory chronologically even though
// selection mixed recency and relevance.
func TestBoundMemory_RendersInIterationAscendingOrder(t *testing.T) {
	var entries []memory.Entry
	for i := 1; i <= memoryCap+memoryRecencyFloor; i++ {
		entries = append(entries, memory.Entry{Kind: memory.KindGap, Topic: "t", Detail: "d", Iteration: i})
	}
	got := boundMemory(entries, "t")
	for i := 1; i < len(got); i++ {
		if got[i-1].Iteration > got[i].Iteration {
			t.Errorf("entries must render in Iteration-ascending order; pos %d (iter %d) > pos %d (iter %d)",
				i-1, got[i-1].Iteration, i, got[i].Iteration)
		}
	}
}

// boundMemory passthrough: a store strictly UNDER the cap is returned UNCHANGED — same
// length, same order, same elements (no sort, no filter). The pure-function twin of the
// byte-compat memoryContext test.
func TestBoundMemory_UnderCapReturnsInputUnchanged(t *testing.T) {
	entries := []memory.Entry{
		{Kind: memory.KindGap, Topic: "b", Detail: "first", Iteration: 5},
		{Kind: memory.KindDecision, Topic: "a", Detail: "second", Iteration: 2}, // out-of-order Iteration
		{Kind: memory.KindLesson, Topic: "c", Detail: "third", Iteration: 9},
	}
	got := boundMemory(entries, "query terms")
	if len(got) != len(entries) {
		t.Fatalf("under-cap must return all %d entries; got %d", len(entries), len(got))
	}
	for i := range entries {
		if got[i] != entries[i] {
			t.Errorf("under-cap must preserve input order/content at %d: got %+v want %+v", i, got[i], entries[i])
		}
	}
}

// UNREADABLE path is unchanged: a malformed store surfaces as a visible context line, not
// an aborted prompt — the honesty contract the bound must not regress.
func TestMemoryContext_MalformedStoreSurfacedNotAborted(t *testing.T) {
	root := t.TempDir()
	mkdir(t, forgeDir(root))
	writeFile(t, memoryPath(root), "{not valid json\n")
	got := memoryContext(root, "q")
	if len(got) != 1 || !strings.Contains(got[0], "UNREADABLE") {
		t.Errorf("a malformed store must surface UNREADABLE as a context line; got %v", got)
	}
}

// Cold start / empty store path is unchanged: no store => no block, no error, regardless
// of query.
func TestMemoryContext_ColdStartYieldsNoBlock(t *testing.T) {
	if got := memoryContext(t.TempDir(), "q"); got != nil {
		t.Errorf("a missing store must yield no memory block; got %v", got)
	}
}
