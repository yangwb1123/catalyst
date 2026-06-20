// prompt_memory.go — the cross-session MEMORY lane of the prompt's Context Engine
// (split out of prompt_context.go to keep each file under the volume budget). memory's
// store is APPEND-ONLY: the evolve loop records one entry per iteration and never
// rewrites, so over a long run (the package doc's "a 24h run") it grows without bound.
// The old memoryContext injected EVERY entry, so the memory lane was the one prompt lane
// with no ceiling — on a marathon run it would eventually overrun the context window, the
// same "long-run inevitable" failure class as the 529 overload. This file bounds it:
// boundMemory caps the injected set with a recency-floor + relevance mix, reusing the
// internal/prompt BM25-lite Retrieve (the "everything → the relevant few" library) for the
// older entries while always keeping the freshest N so the latest lessons are never lost.
//
// Layering: selection lives HERE in cmd/forge (the prompt-building layer) and calls the
// prompt library DOWNWARD (prompt.Retrieve / prompt.Doc); internal/memory is untouched —
// it still only stores (append/load) and exposes its pure Query, with no notion of a cap.
package main

import (
	"fmt"
	"sort"
	"strconv"
	"strings"

	"forgeos/forge-core/internal/memory"
	"forgeos/forge-core/internal/prompt"
)

// memoryCap is the most cross-session memory entries memoryContext will inject into
// one prompt — the budget that stops an ever-growing store from blowing the context
// window on a long run. The store is APPEND-ONLY (one entry per evolve iteration,
// never rewritten), so on a 24h / dozens-of-iterations run it grows without bound;
// injecting every entry (the old memoryContext did) would eventually overrun the
// window — the same "long-run inevitable" failure class as the 529 overload. 32 is
// chosen as comfortably above a normal store (so small/typical projects inject whole,
// byte-for-byte the prior behavior — existing memoryContext tests are unaffected) yet
// a hard ceiling so a marathon run stays bounded. It is the memory lane's twin of
// adrTopK / taskCap / phaseOutputSummaryCap — the last prompt lane that lacked a bound.
const memoryCap = 32

// memoryRecencyFloor is how many of the most RECENT entries (largest Iteration) are
// always injected when the store exceeds memoryCap, regardless of query relevance.
// It exists because memory's whole point is "do not relearn what you just learned":
// the freshest gaps/decisions/lessons must never be dropped. Pure relevance would not
// guarantee this — its zero-match fallback ranks by input (append) order, biasing
// OLD; this floor pins the newest N in so the latest lessons are never amnesiacally
// lost. The remaining (memoryCap - memoryRecencyFloor) slots go to the most RELEVANT
// older entries (so a still-pertinent old decision is kept, not torn down). 8 leaves
// a healthy 24 relevance slots under the cap.
const memoryRecencyFloor = 8

// Compile-time invariant: the recency floor must leave at least one relevance slot
// (0 < memoryRecencyFloor < memoryCap). boundMemory's "returns exactly memoryCap over
// the cap" guarantee depends on it — if the floor ever met or exceeded the cap, the
// relevance pass would get k<=0 and the floor alone could overshoot the cap. This makes
// a future edit that breaks the relation FAIL TO COMPILE rather than silently overflow
// the prompt budget. (A negative array length is a Go compile error.)
const _ = uint(memoryCap - memoryRecencyFloor - 1) // fails to compile if floor >= cap
const _ = uint(memoryRecencyFloor - 1)             // fails to compile if floor <= 0

// boundMemory bounds an unbounded entry slice to at most memoryCap entries, returned
// in Iteration-ASCENDING order (chronological, so the agent reads them as a coherent
// timeline rather than a relevance-jumbled list).
//
// Strategy and its HONEST trade-offs:
//   - len <= memoryCap: returned UNCHANGED, in input order — byte-for-byte the old
//     unbounded behavior for any normal store (this is what keeps existing tests/small
//     projects identical).
//   - len > memoryCap: a RECENCY FLOOR + RELEVANCE mix. The memoryRecencyFloor newest
//     entries are always kept (freshest lessons never lost); the rest of the budget is
//     filled by prompt.Retrieve's top-K against the phase query (a still-relevant OLD
//     decision survives). Floor ∪ relevance is de-duplicated, then sorted by Iteration.
//
// Trade-off (stated, not hidden): over the cap, a MIDDLE entry that is neither recent
// nor relevant to this phase is dropped — an old gap unrelated to the current work may
// be omitted. That is the necessary cost of not overrunning the window. Retrieval is
// v1 keyword BM25-lite (prompt.Retrieve), NOT semantic — "vehicle" won't match "car";
// the memory lane inherits that limit (semantic retrieval is v3). The recency floor
// guarantees the newest memoryRecencyFloor entries are always present regardless.
func boundMemory(entries []memory.Entry, query string) []memory.Entry {
	if len(entries) <= memoryCap {
		return entries
	}
	keep := recentFloorSet(entries, memoryRecencyFloor)
	for _, e := range relevantOlder(entries, query, memoryCap-len(keep), keep) {
		keep[e] = struct{}{}
	}
	out := make([]memory.Entry, 0, len(keep))
	for i, e := range entries {
		if _, ok := keep[i]; ok {
			out = append(out, e)
		}
	}
	sort.SliceStable(out, func(a, b int) bool { return out[a].Iteration < out[b].Iteration })
	return out
}

// recentFloorSet returns the indices of the n entries with the largest Iteration — the
// always-in recency floor. The sort is STABLE on a descending-Iteration comparator, so
// among entries that TIE on Iteration the EARLIER input position is kept first (stable
// sort preserves input order on equal keys); in practice the evolve log's Iterations are
// distinct and monotonic, so ties do not arise. n is clamped to len(entries). Indices
// (not entries) are returned so the caller can de-dup against the relevance pick by
// original position.
func recentFloorSet(entries []memory.Entry, n int) map[int]struct{} {
	idx := make([]int, len(entries))
	for i := range entries {
		idx[i] = i
	}
	sort.SliceStable(idx, func(a, b int) bool { return entries[idx[a]].Iteration > entries[idx[b]].Iteration })
	if n > len(idx) {
		n = len(idx)
	}
	set := make(map[int]struct{}, n)
	for _, i := range idx[:n] {
		set[i] = struct{}{}
	}
	return set
}

// relevantOlder selects up to k indices of entries (excluding those already in `have`)
// most relevant to query via prompt.Retrieve — the BM25-lite library built for exactly
// this "everything → the relevant few" job. Each candidate becomes a prompt.Doc whose ID
// is its ORIGINAL index (so the ranked result maps straight back to entries). k<=0 or no
// candidates yields nil. Honest: keyword retrieval, not semantic (see boundMemory).
func relevantOlder(entries []memory.Entry, query string, k int, have map[int]struct{}) []int {
	if k <= 0 {
		return nil
	}
	var docs []prompt.Doc
	for i, e := range entries {
		if _, taken := have[i]; taken {
			continue
		}
		docs = append(docs, prompt.Doc{ID: strconv.Itoa(i), Text: e.Kind + " " + e.Topic + " " + e.Detail})
	}
	var out []int
	for _, d := range prompt.Retrieve(docs, query, k) {
		if i, err := strconv.Atoi(d.ID); err == nil {
			out = append(out, i)
		}
	}
	return out
}

// memoryContext renders the cross-session store as one BOUNDED context block so the
// agent sees what prior iterations learned WITHOUT the store (which grows every evolve
// iteration, unbounded over a long run) eventually blowing the context window. Selection
// is boundMemory's recency-floor + relevance mix keyed off `query` (the phase identity,
// aligned with Gather) — a change from the old "inject every entry" behavior. The honest
// trade-offs (a non-recent/non-relevant middle entry may be dropped over the cap; keyword
// not semantic retrieval; newest memoryRecencyFloor always kept) live on boundMemory.
// Below the cap, every entry is still injected in input order, byte-for-byte as before.
// Missing store = cold start (no block, no error); a malformed store is surfaced as a
// visible context line, not an aborted prompt.
func memoryContext(repoRoot, query string) []string {
	entries, err := memory.Load(memoryPath(repoRoot))
	if err != nil {
		return []string{"Project memory: UNREADABLE (" + err.Error() + ")"}
	}
	rel := boundMemory(entries, query)
	if len(rel) == 0 {
		return nil
	}
	var b strings.Builder
	b.WriteString("Project memory (gaps / decisions / lessons from prior iterations):")
	for _, e := range rel {
		fmt.Fprintf(&b, "\n- [%s] %s — %s (iter %d)", e.Kind, e.Topic, e.Detail, e.Iteration)
	}
	return []string{b.String()}
}
