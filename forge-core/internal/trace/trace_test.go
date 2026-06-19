package trace

import (
	"bytes"
	"encoding/json"
	"strings"
	"sync"
	"testing"
	"time"
)

// nonEmptyLines splits a JSONL buffer into its records, dropping the trailing
// empty element after the final '\n'. Every kept line should be one complete
// JSON object, which is exactly the property the JSONL format must guarantee.
func nonEmptyLines(s string) []string {
	var out []string
	for _, ln := range strings.Split(s, "\n") {
		if ln != "" {
			out = append(out, ln)
		}
	}
	return out
}

// Emitting several events must yield one JSON object per line with the fields
// preserved and Seq auto-incrementing 1,2,3… without the caller setting it.
func TestEmit_SeqAndFields(t *testing.T) {
	var buf bytes.Buffer
	tr := NewTracer(&buf)

	in := []Event{
		{Kind: "iteration", Name: "1", Status: "ok"},
		{Kind: "gate", Name: "lint", Status: "PASS", Detail: "0 issues"},
		{Kind: "converge", Name: "build.yml", Status: "FAIL"},
	}
	for _, ev := range in {
		if err := tr.Emit(ev); err != nil {
			t.Fatalf("Emit: %v", err)
		}
	}

	lines := nonEmptyLines(buf.String())
	if len(lines) != len(in) {
		t.Fatalf("got %d lines, want %d: %q", len(lines), len(in), buf.String())
	}
	for i, ln := range lines {
		var got Event
		if err := json.Unmarshal([]byte(ln), &got); err != nil {
			t.Fatalf("line %d not valid JSON (%v): %q", i, err, ln)
		}
		if want := i + 1; got.Seq != want {
			t.Errorf("line %d: Seq=%d, want %d", i, got.Seq, want)
		}
		if got.Kind != in[i].Kind || got.Name != in[i].Name || got.Status != in[i].Status {
			t.Errorf("line %d: round-trip mismatch: got %+v, want %+v", i, got, in[i])
		}
	}
}

// Detail is tagged omitempty: a blank Detail must not appear as a JSON key, so
// the on-disk record stays minimal for the (common) no-context events.
func TestEmit_DetailOmittedWhenEmpty(t *testing.T) {
	var buf bytes.Buffer
	tr := NewTracer(&buf)
	if err := tr.Emit(Event{Kind: "iteration", Name: "1", Status: "ok"}); err != nil {
		t.Fatalf("Emit: %v", err)
	}
	if strings.Contains(buf.String(), "detail") {
		t.Errorf("empty Detail should be omitted, got %q", buf.String())
	}
}

// Span must measure elapsed time via the injectable clock. With a fake clock
// that jumps t0 -> t0+250ms between start and finish, the emitted event's
// DurationMs is exactly 250 — fully deterministic, no real sleeping.
func TestSpan_DeterministicDuration(t *testing.T) {
	var buf bytes.Buffer
	tr := NewTracer(&buf)

	t0 := time.Date(2026, 6, 19, 0, 0, 0, 0, time.UTC)
	times := []time.Time{t0, t0.Add(250 * time.Millisecond)}
	var i int
	tr.Now = func() time.Time { // start reads times[0], finish reads times[1]
		ti := times[i]
		i++
		return ti
	}

	finish := tr.Span("gate", "build")
	finish("PASS", "compiled")

	var got Event
	if err := json.Unmarshal([]byte(strings.TrimSpace(buf.String())), &got); err != nil {
		t.Fatalf("span event not valid JSON (%v): %q", err, buf.String())
	}
	if got.DurationMs != 250 {
		t.Errorf("DurationMs=%d, want 250", got.DurationMs)
	}
	if got.Kind != "gate" || got.Name != "build" || got.Status != "PASS" || got.Detail != "compiled" {
		t.Errorf("span event fields wrong: %+v", got)
	}
	if got.Seq != 1 {
		t.Errorf("span event Seq=%d, want 1", got.Seq)
	}
}

// Concurrency is the load-bearing reason Emit holds a lock: many goroutines
// emitting at once must produce N intact lines (no torn/interleaved JSON) and
// N distinct sequence numbers covering exactly 1..N.
func TestEmit_ConcurrentLinesIntact(t *testing.T) {
	var buf bytes.Buffer
	tr := NewTracer(&buf)

	const n = 200
	var wg sync.WaitGroup
	wg.Add(n)
	for k := 0; k < n; k++ {
		go func(k int) {
			defer wg.Done()
			// A non-trivial Detail widens the window for a torn write to show up.
			_ = tr.Emit(Event{Kind: "agent", Name: "impl", Status: "ok", Detail: strings.Repeat("x", 64)})
		}(k)
	}
	wg.Wait()

	lines := nonEmptyLines(buf.String())
	if len(lines) != n {
		t.Fatalf("got %d lines, want %d", len(lines), n)
	}
	seen := make(map[int]bool, n)
	for i, ln := range lines {
		var got Event
		if err := json.Unmarshal([]byte(ln), &got); err != nil {
			t.Fatalf("line %d corrupt (interleaved write?): %v: %q", i, err, ln)
		}
		if got.Seq < 1 || got.Seq > n {
			t.Errorf("Seq %d out of range 1..%d", got.Seq, n)
		}
		if seen[got.Seq] {
			t.Errorf("duplicate Seq %d", got.Seq)
		}
		seen[got.Seq] = true
	}
	if len(seen) != n {
		t.Errorf("got %d distinct seqs, want %d", len(seen), n)
	}
}

// encode is pure: it must yield a compact single-line JSON object terminated by
// exactly one newline (the JSONL framing), and that line must parse back to the
// same Event. Tested without a Tracer, writer, or lock.
func TestEncode_PureFraming(t *testing.T) {
	ev := Event{Seq: 7, Kind: "gate", Name: "security", Status: "NA", DurationMs: 5, Detail: "no scanner"}
	b, err := encode(ev)
	if err != nil {
		t.Fatalf("encode: %v", err)
	}
	if n := bytes.Count(b, []byte("\n")); n != 1 || b[len(b)-1] != '\n' {
		t.Fatalf("want exactly one trailing newline, got %d in %q", n, b)
	}
	if bytes.Contains(b[:len(b)-1], []byte("\n")) {
		t.Errorf("JSON body must be single-line, got %q", b)
	}
	var got Event
	if err := json.Unmarshal(b, &got); err != nil {
		t.Fatalf("encoded line not valid JSON: %v", err)
	}
	if got != ev {
		t.Errorf("round-trip mismatch: got %+v, want %+v", got, ev)
	}
}
