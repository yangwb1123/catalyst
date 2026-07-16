package memory

import (
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"sort"
	"strings"
	"sync"
	"testing"
	"time"
)

// sampleEntries returns a small, ordered set spanning all three kinds and two
// topics; Confidence is 1.0 to match the decode default for old files.
func sampleEntries() []Entry {
	return []Entry{
		{Kind: KindGap, Topic: "auth", Detail: "no token refresh path", Iteration: 1, Confidence: 1.0, Format: "forgeos.memory.v1", CreatedAtUnix: 1_750_000_001},
		{Kind: KindDecision, Topic: "auth", Detail: "use short-lived JWTs", Iteration: 2, Confidence: 1.0, Format: "forgeos.memory.v1", CreatedAtUnix: 1_750_000_002},
		{Kind: KindLesson, Topic: "cache", Detail: "evict-all on schema bump is too slow", Iteration: 3, Confidence: 1.0, Format: "forgeos.memory.v1", CreatedAtUnix: 1_750_000_003},
		{Kind: KindGap, Topic: "cache", Detail: "no metrics on hit rate", Iteration: 4, Confidence: 1.0, Format: "forgeos.memory.v1", CreatedAtUnix: 1_750_000_004},
	}
}

func TestAppendLoad_RoundTrip(t *testing.T) {
	path := filepath.Join(t.TempDir(), "knowledge.jsonl")
	want := sampleEntries()
	for i, e := range want {
		if err := Append(path, e); err != nil {
			t.Fatalf("Append #%d: %v", i, err)
		}
	}
	got, err := Load(path)
	if err != nil {
		t.Fatalf("Load: %v", err)
	}
	// Entries must return in the exact append order, byte-for-byte identical.
	if !reflect.DeepEqual(got, want) {
		t.Errorf("round-trip mismatch:\n got  %+v\n want %+v", got, want)
	}
}

func TestLoad_Missing_NotAnError(t *testing.T) {
	// Cold start: absence must read as (nil, nil), never an error.
	path := filepath.Join(t.TempDir(), "does-not-exist.jsonl")
	got, err := Load(path)
	if err != nil {
		t.Fatalf("Load of missing file returned error: %v", err)
	}
	if got != nil {
		t.Errorf("entries = %+v, want nil for missing file", got)
	}
}

func TestLoad_MalformedLine_IsError(t *testing.T) {
	// A garbled line must surface as an explicit error, never be silently
	// skipped — a valid line precedes it to isolate the corruption.
	path := filepath.Join(t.TempDir(), "corrupt.jsonl")
	good, err := encode(sampleEntries()[0])
	if err != nil {
		t.Fatalf("encode seed: %v", err)
	}
	blob := append(good, []byte("{not valid json\n")...)
	if err := os.WriteFile(path, blob, 0o644); err != nil {
		t.Fatalf("seed corrupt file: %v", err)
	}
	got, err := Load(path)
	if err == nil {
		t.Fatal("Load of malformed line returned nil error, want error")
	}
	if got != nil {
		t.Errorf("entries = %+v, want nil on decode error", got)
	}
}

func TestLoad_BlankLinesSkipped(t *testing.T) {
	// Blank lines are framing, not records; Load must skip them, in order.
	path := filepath.Join(t.TempDir(), "blanks.jsonl")
	a, _ := encode(sampleEntries()[0])
	b, _ := encode(sampleEntries()[1])
	blob := append([]byte("\n"), a...)
	blob = append(blob, '\n') // extra blank line between records
	blob = append(blob, b...) // second record
	blob = append(blob, '\n') // trailing blank line
	if err := os.WriteFile(path, blob, 0o644); err != nil {
		t.Fatalf("seed file: %v", err)
	}
	got, err := Load(path)
	if err != nil {
		t.Fatalf("Load: %v", err)
	}
	want := sampleEntries()[:2]
	if !reflect.DeepEqual(got, want) {
		t.Errorf("blank-line handling mismatch:\n got  %+v\n want %+v", got, want)
	}
}

func TestQuery_FilterByKindAndTopic(t *testing.T) {
	es := sampleEntries()
	cases := []struct {
		name        string
		kind, topic string
		wantDetails []string // expected Detail values, in input order
	}{
		{"no constraint returns all", "", "", []string{
			"no token refresh path",
			"use short-lived JWTs",
			"evict-all on schema bump is too slow",
			"no metrics on hit rate",
		}},
		{"kind only", KindGap, "", []string{
			"no token refresh path",
			"no metrics on hit rate",
		}},
		{"topic only", "", "auth", []string{
			"no token refresh path",
			"use short-lived JWTs",
		}},
		{"kind and topic", KindGap, "cache", []string{
			"no metrics on hit rate",
		}},
		{"no match", KindDecision, "cache", []string{}},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			got := Query(es, tc.kind, tc.topic)
			gotDetails := make([]string, len(got))
			for i, e := range got {
				gotDetails[i] = e.Detail
			}
			// Compare by length then element so empty results match []string{} too.
			if len(gotDetails) != len(tc.wantDetails) {
				t.Fatalf("Query(%q,%q) returned %d entries, want %d",
					tc.kind, tc.topic, len(gotDetails), len(tc.wantDetails))
			}
			for i := range tc.wantDetails {
				if gotDetails[i] != tc.wantDetails[i] {
					t.Errorf("Query(%q,%q)[%d].Detail = %q, want %q",
						tc.kind, tc.topic, i, gotDetails[i], tc.wantDetails[i])
				}
			}
		})
	}
}

func TestQuery_DoesNotMutateInput(t *testing.T) {
	es := sampleEntries()       // Query is a pure filter: callers reuse the corpus.
	snapshot := sampleEntries() // independent copy to compare against
	_ = Query(es, KindGap, "")
	if !reflect.DeepEqual(es, snapshot) {
		t.Errorf("Query mutated its input:\n got  %+v\n want %+v", es, snapshot)
	}
}

func TestAppend_ConcurrentDoesNotLoseEntries(t *testing.T) {
	// O_APPEND makes each write atomic; parallel writers must not interleave
	// or drop entries (order across goroutines isn't guaranteed; compare as sets).
	path := filepath.Join(t.TempDir(), "concurrent.jsonl")
	const n = 50
	var wg sync.WaitGroup
	wg.Add(n)
	for i := 0; i < n; i++ {
		go func(i int) {
			defer wg.Done()
			e := Entry{Kind: KindLesson, Topic: "race", Detail: "entry", Iteration: i, Format: "forgeos.memory.v1", CreatedAtUnix: int64(i)}
			if err := Append(path, e); err != nil {
				t.Errorf("Append(%d): %v", i, err)
			}
		}(i)
	}
	wg.Wait()
	got, err := Load(path)
	if err != nil {
		t.Fatalf("Load after concurrent appends: %v", err)
	}
	if len(got) != n {
		t.Fatalf("loaded %d entries, want %d (lost or interleaved writes)", len(got), n)
	}
	// Every iteration 0..n-1 must appear exactly once.
	iters := make([]int, len(got))
	for i, e := range got {
		iters[i] = e.Iteration
	}
	sort.Ints(iters)
	for i := 0; i < n; i++ {
		if iters[i] != i {
			t.Fatalf("missing/duplicate iteration: sorted[%d] = %d, want %d", i, iters[i], i)
		}
	}
}

func TestAppend_SuccessiveDoesNotTruncate(t *testing.T) {
	// Appending must never rewrite history: each call adds one line, keeps the rest.
	path := filepath.Join(t.TempDir(), "successive.jsonl")
	es := sampleEntries()
	for i := range es {
		if err := Append(path, es[i]); err != nil {
			t.Fatalf("Append #%d: %v", i, err)
		}
		got, err := Load(path)
		if err != nil {
			t.Fatalf("Load after append #%d: %v", i, err)
		}
		if len(got) != i+1 {
			t.Fatalf("after append #%d store has %d entries, want %d", i, len(got), i+1)
		}
	}
}

func TestAppend_CreatesMissingParentDirs(t *testing.T) {
	// Append must materialize the parent path before the first finding can land.
	path := filepath.Join(t.TempDir(), "nested", "deeper", "knowledge.jsonl")
	want := sampleEntries()[0]
	if err := Append(path, want); err != nil {
		t.Fatalf("Append into missing dirs: %v", err)
	}
	got, err := Load(path)
	if err != nil {
		t.Fatalf("Load after nested Append: %v", err)
	}
	if len(got) != 1 || got[0] != want {
		t.Errorf("nested round-trip = %+v, want exactly [%+v]", got, want)
	}
}

func TestEncodeDecode_RoundTrip(t *testing.T) {
	// Exercise the pure serialization boundary directly, with no filesystem.
	want := sampleEntries()
	var blob []byte
	for _, e := range want {
		b, err := encode(e)
		if err != nil {
			t.Fatalf("encode: %v", err)
		}
		blob = append(blob, b...)
	}
	got, err := decode(blob)
	if err != nil {
		t.Fatalf("decode: %v", err)
	}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("encode/decode mismatch:\n got  %+v\n want %+v", got, want)
	}
}

func TestDecode_Malformed_IsError(t *testing.T) {
	if _, err := decode([]byte("}{garbage\n")); err == nil {
		t.Fatal("decode of garbage returned nil error, want error")
	}
}

func TestFilterSuperseded(t *testing.T) {
	tests := []struct {
		name  string
		input []Entry
		want  int // expected count after filtering
	}{
		{
			name: "no supersedes passes through",
			input: []Entry{
				{Kind: KindGap, Topic: "auth", Detail: "no token refresh", Iteration: 1, Confidence: 1.0, Format: "forgeos.memory.v1"},
				{Kind: KindDecision, Topic: "auth", Detail: "use JWT", Iteration: 2, Confidence: 1.0, Format: "forgeos.memory.v1"},
			},
			want: 2,
		},
		{
			name: "supersedes removes old entry",
			input: []Entry{
				{Kind: KindDecision, Topic: "db-choice", Detail: "use SQLite", Iteration: 1, Confidence: 1.0, Format: "forgeos.memory.v1", CreatedAtUnix: 0},
				{Kind: KindDecision, Topic: "db-choice", Detail: "use PostgreSQL", Iteration: 2, Confidence: 1.0, Format: "forgeos.memory.v1", Supersedes: "db-choice"},
			},
			want: 1, // first entry superseded, only second kept
		},
		{
			name: "multiple supersedes in chain",
			input: []Entry{
				{Kind: KindDecision, Topic: "cache-strat", Detail: "redis", Iteration: 1, Confidence: 1.0, Format: "forgeos.memory.v1", Supersedes: "old-cache"},
				{Kind: KindDecision, Topic: "old-cache", Detail: "memcached", Iteration: 1, Confidence: 1.0, Format: "forgeos.memory.v1"},
				{Kind: KindDecision, Topic: "cache-strat", Detail: "redis cluster", Iteration: 3, Confidence: 1.0, Format: "forgeos.memory.v1", Supersedes: "cache-strat"},
			},
			want: 1, // old-cache superseded by entry1, cache-strat (entry1) superseded by entry3; only entry3 kept
		},
		{
			name:  "empty input returns empty",
			input: []Entry{},
			want:  0,
		},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := filterSuperseded(tc.input)
			if len(got) != tc.want {
				t.Errorf("filterSuperseded returned %d entries, want %d\ngot: %+v", len(got), tc.want, got)
			}
		})
	}
}

// ── Compaction tests (seventh-wave-data-realism.md §方向2) ─────────────────

// TestCompact_BelowThresholdIsNoop: Compact is a no-op below threshold.
func TestCompact_BelowThresholdIsNoop(t *testing.T) {
	path := filepath.Join(t.TempDir(), "memory.jsonl")
	// Append 10 entries (well below default threshold of 500).
	for i := 0; i < 10; i++ {
		if err := Append(path, Entry{Kind: KindLesson, Topic: "evolve", Detail: fmt.Sprintf("iter %d", i+1), CreatedAtUnix: int64(1000 + i)}); err != nil {
			t.Fatalf("Append: %v", err)
		}
	}
	removed, compacted, err := Compact(path, 500, 20, CompactAgeSeconds)
	if err != nil {
		t.Fatalf("Compact: %v", err)
	}
	if compacted {
		t.Error("Compact returned compacted=true for below-threshold store")
	}
	if removed != 0 {
		t.Errorf("Compact returned removed=%d, want 0", removed)
	}
}

// TestCompact_GroupByKind: compaction groups by kind, keeps ≤keepPerKind each.
func TestCompact_GroupByKind(t *testing.T) {
	path := filepath.Join(t.TempDir(), "memory.jsonl")
	now := int64(1000000)
	// 10 lessons/gaps/decisions across 3+ topics, all >24h old (eligible).
	for i := 0; i < 10; i++ {
		if err := Append(path, Entry{Kind: KindLesson, Topic: "auth", Detail: fmt.Sprintf("lesson %d", i), CreatedAtUnix: now + int64(i)}); err != nil {
			t.Fatalf("Append: %v", err)
		}
		if err := Append(path, Entry{Kind: KindGap, Topic: "test", Detail: fmt.Sprintf("gap %d", i), CreatedAtUnix: now + int64(i)}); err != nil {
			t.Fatalf("Append: %v", err)
		}
		if err := Append(path, Entry{Kind: KindDecision, Topic: "db", Detail: fmt.Sprintf("decision %d", i), CreatedAtUnix: now + int64(i)}); err != nil {
			t.Fatalf("Append: %v", err)
		}
	}
	// 30 total entries, threshold=20 → trigger compaction, keep=5 per kind.
	removed, compacted, err := Compact(path, 20, 5, CompactAgeSeconds)
	if err != nil {
		t.Fatalf("Compact: %v", err)
	}
	if !compacted {
		t.Fatal("Compact returned compacted=false for store above threshold")
	}
	if removed <= 0 {
		t.Fatalf("Compact returned removed=%d, want > 0 (should have compressed old entries)", removed)
	}
	// After compaction: 5 kept per kind × 3 kinds = 15 + 3 summary entries = up to 18.
	entries, err := Load(path)
	if err != nil {
		t.Fatalf("Load after compact: %v", err)
	}
	if len(entries) > 25 {
		t.Errorf("Compact left %d entries, want ≤ 18 (5 kept×3 kinds + 3 summaries)", len(entries))
	}
	// Must have summary entries.
	summary := Query(entries, "compact_summary", "")
	if len(summary) < 3 {
		t.Errorf("Compact produced %d summary entries, want 3 (one per kind); got: %+v", len(summary), summary)
	}
}

// TestCompact_RecentPreserved: entries within the age window stay verbatim.
func TestCompact_RecentPreserved(t *testing.T) {
	path := filepath.Join(t.TempDir(), "memory.jsonl")
	now := time.Now().Unix()
	// Add 30 old entries (timestamp > 24h ago)
	for i := 0; i < 30; i++ {
		if err := Append(path, Entry{Kind: KindLesson, Topic: "old", Detail: fmt.Sprintf("old %d", i), CreatedAtUnix: now - 2*CompactAgeSeconds}); err != nil {
			t.Fatalf("Append: %v", err)
		}
	}
	// Add 10 recent entries (timestamp within the last hour)
	for i := 0; i < 10; i++ {
		if err := Append(path, Entry{Kind: KindLesson, Topic: "recent", Detail: fmt.Sprintf("recent %d", i), CreatedAtUnix: now - 600}); err != nil {
			t.Fatalf("Append: %v", err)
		}
	}
	removed, compacted, err := Compact(path, 20, 5, CompactAgeSeconds)
	if err != nil {
		t.Fatalf("Compact: %v", err)
	}
	if !compacted {
		t.Fatal("Compact returned compacted=false")
	}
	// All 10 recent entries must still be in the store.
	entries, err := Load(path)
	if err != nil {
		t.Fatalf("Load: %v", err)
	}
	recent := Query(entries, KindLesson, "recent")
	if len(recent) != 10 {
		t.Errorf("Compact preserved %d recent entries, want 10; got: %+v", len(recent), recent)
	}
	_ = removed
}

// TestCompact_EmptyStoreIsNoop verifies Compact on an empty or missing store.
func TestCompact_EmptyStoreIsNoop(t *testing.T) {
	path := filepath.Join(t.TempDir(), "nonexistent.jsonl")
	_, compacted, err := Compact(path, 500, 20, CompactAgeSeconds)
	if err != nil {
		t.Fatalf("Compact on missing file: %v", err)
	}
	if compacted {
		t.Error("Compact returned compacted=true for missing store")
	}
}

// TestCompact_ZeroThresholdTriggers: threshold=0 compacts any non-empty store.
func TestCompact_ZeroThresholdTriggers(t *testing.T) {
	path := filepath.Join(t.TempDir(), "memory.jsonl")
	if err := Append(path, Entry{Kind: KindLesson, Topic: "test", Detail: "entry", CreatedAtUnix: 1000}); err != nil {
		t.Fatalf("Append: %v", err)
	}
	// threshold=0, keepPerKind=1, ageSeconds=0 (all entries are old)
	_, compacted, err := Compact(path, 0, 1, 0)
	if err != nil {
		t.Fatalf("Compact: %v", err)
	}
	if !compacted {
		t.Error("Compact returned compacted=false with threshold=0")
	}
}

// TestSummarizeBlock_ProducesSummaryEntry verifies the summary entry content.
func TestSummarizeBlock_ProducesSummaryEntry(t *testing.T) {
	entries := []Entry{
		{Kind: KindLesson, Topic: "auth", Detail: "JWT best practice", CreatedAtUnix: 1000000},
		{Kind: KindLesson, Topic: "auth", Detail: "OAuth2 flow", CreatedAtUnix: 1000001},
		{Kind: KindLesson, Topic: "cache", Detail: "Redis config", CreatedAtUnix: 1000002},
	}
	summary := summarizeBlock(KindLesson, entries)
	if summary == nil {
		t.Fatal("summarizeBlock returned nil")
	}
	if summary.Kind != "compact_summary" {
		t.Errorf("summary kind = %q, want compact_summary", summary.Kind)
	}
	if summary.Topic != KindLesson {
		t.Errorf("summary topic = %q, want %q", summary.Topic, KindLesson)
	}
	if summary.Detail == "" {
		t.Error("summary detail is empty")
	}
	// Must contain the count and kind
	if !strings.Contains(summary.Detail, "3") || !strings.Contains(summary.Detail, KindLesson) {
		t.Errorf("summary detail = %q, should mention '3 lesson'", summary.Detail)
	}
}

func TestSummarizeBlock_EmptyReturnsNil(t *testing.T) {
	if summary := summarizeBlock(KindLesson, nil); summary != nil {
		t.Errorf("summarizeBlock(nil) = %v, want nil", summary)
	}
	if summary := summarizeBlock(KindLesson, []Entry{}); summary != nil {
		t.Errorf("summarizeBlock(empty) = %v, want nil", summary)
	}
}

// TestSummarizeBlock_TopicEqualsKindNoCollision: Topic==Kind must use separate
// counters for the per-topic tally and the grand total, never one shared map.
func TestSummarizeBlock_TopicEqualsKindNoCollision(t *testing.T) {
	entries := []Entry{
		{Kind: KindGap, Topic: KindGap, Detail: "a", CreatedAtUnix: 1},
		{Kind: KindGap, Topic: KindGap, Detail: "b", CreatedAtUnix: 2},
		{Kind: KindGap, Topic: "auth", Detail: "c", CreatedAtUnix: 3},
	}
	s := summarizeBlock(KindGap, entries)
	if s == nil || !strings.Contains(s.Detail, "compacted 3 gap") ||
		!strings.Contains(s.Detail, "gap:2") || !strings.Contains(s.Detail, "auth:1") {
		t.Errorf("summarizeBlock w/ Topic==Kind = %+v, want total 3, tallies gap:2 & auth:1", s)
	}
}

// TestCompact_NegativeKeepPerKindDoesNotPanic: Compact must clamp a negative
// keepPerKind to 0 (mirroring Prune's keepLast<=0 clamp), not panic on it.
func TestCompact_NegativeKeepPerKindDoesNotPanic(t *testing.T) {
	path := filepath.Join(t.TempDir(), "memory.jsonl")
	for i := 0; i < 3; i++ {
		e := Entry{Kind: KindLesson, Topic: "x", Detail: fmt.Sprintf("e%d", i), CreatedAtUnix: int64(1000 + i)}
		if err := Append(path, e); err != nil {
			t.Fatalf("Append: %v", err)
		}
	}
	removed, compacted, err := Compact(path, 0, -1, 0) // must not panic
	if err != nil {
		t.Fatalf("Compact(keepPerKind=-1): %v", err)
	}
	if !compacted || removed != 2 {
		t.Errorf("Compact(-1) = removed=%d compacted=%v, want removed=2 compacted=true (== keepPerKind=0)", removed, compacted)
	}
	got, err := Load(path)
	if err != nil || len(got) != 1 || got[0].Kind != "compact_summary" {
		t.Errorf("store after Compact(-1) = %+v (err=%v), want single compact_summary entry", got, err)
	}
}
