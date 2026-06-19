package memory

import (
	"os"
	"path/filepath"
	"reflect"
	"sort"
	"sync"
	"testing"
)

// sampleEntries returns a small, ordered knowledge set spanning all three kinds
// and two topics, so order-preservation and kind/topic filtering are both
// exercised. Every field is non-zero so a round-trip that drops or zeroes one is
// caught.
func sampleEntries() []Entry {
	return []Entry{
		{Kind: KindGap, Topic: "auth", Detail: "no token refresh path", Iteration: 1, CreatedAtUnix: 1_750_000_001},
		{Kind: KindDecision, Topic: "auth", Detail: "use short-lived JWTs", Iteration: 2, CreatedAtUnix: 1_750_000_002},
		{Kind: KindLesson, Topic: "cache", Detail: "evict-all on schema bump is too slow", Iteration: 3, CreatedAtUnix: 1_750_000_003},
		{Kind: KindGap, Topic: "cache", Detail: "no metrics on hit rate", Iteration: 4, CreatedAtUnix: 1_750_000_004},
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
	// Entries must come back in the exact order they were appended (a log, not a
	// set) and byte-for-byte identical across every field.
	if !reflect.DeepEqual(got, want) {
		t.Errorf("round-trip mismatch:\n got  %+v\n want %+v", got, want)
	}
}

func TestLoad_Missing_NotAnError(t *testing.T) {
	// Cold start: the loop has recorded nothing yet. Absence must read as "no
	// knowledge" — (nil, nil) — not as a failure that would stall startup.
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
	// A garbled line must surface as an explicit error, never be silently skipped:
	// dropping it would make Load under-report the store and let the loop forget a
	// finding it once recorded. A valid line precedes the bad one to prove the
	// failure is the corruption, not an empty-file artifact.
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
	// A trailing newline (and stray blank lines) are framing, not records. Load
	// must tolerate them and return only the real entries, in order.
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
			// Compare lengths then elements so an empty result matches []string{}
			// regardless of nil-vs-empty; Query intentionally returns a non-nil
			// empty slice (a valid "new slice") when nothing matches.
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
	// Query is a pure filter: callers reuse the loaded corpus across many queries,
	// so it must neither reorder nor alter the backing slice.
	es := sampleEntries()
	snapshot := sampleEntries() // independent copy to compare against

	_ = Query(es, KindGap, "")
	if !reflect.DeepEqual(es, snapshot) {
		t.Errorf("Query mutated its input:\n got  %+v\n want %+v", es, snapshot)
	}
}

func TestAppend_ConcurrentDoesNotLoseEntries(t *testing.T) {
	// O_APPEND makes each one-line write its own atomic record, so parallel writers
	// must not interleave into corrupt lines or drop entries. Every appended entry
	// must be present and parseable afterward (order across goroutines is not
	// guaranteed, so compare as sets).
	path := filepath.Join(t.TempDir(), "concurrent.jsonl")
	const n = 50

	var wg sync.WaitGroup
	wg.Add(n)
	for i := 0; i < n; i++ {
		go func(i int) {
			defer wg.Done()
			e := Entry{Kind: KindLesson, Topic: "race", Detail: "entry", Iteration: i, CreatedAtUnix: int64(i)}
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
	// Appending must never rewrite history: each call adds exactly one line and
	// leaves all prior lines intact.
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
	// Append must materialize the parent path; the loop should not have to pre-make
	// a .forge/memory/ tree before its first finding can land.
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
