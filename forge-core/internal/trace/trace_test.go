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

// CostUsdMicros is an opaque LLM-cost field: when set it must marshal under the
// stable `cost_usd_micros` json tag (an integer, not a float — jitter-free), and
// round-trip back to the same value. This is the on-disk contract the scorecard's
// --trace cost reader consumes.
func TestEmit_CostUsdMicrosMarshalsWhenSet(t *testing.T) {
	var buf bytes.Buffer
	tr := NewTracer(&buf)
	// 0.0544045 USD is the real claude billed cost; the caller stores it as 54404
	// microdollars (USD x 1e6, rounded) — exactly what avoids float-JSON drift.
	if err := tr.Emit(Event{Kind: "agent", Name: "implementer", Status: "ok", CostUsdMicros: 54404}); err != nil {
		t.Fatalf("Emit: %v", err)
	}
	if !strings.Contains(buf.String(), `"cost_usd_micros":54404`) {
		t.Errorf("cost must marshal under cost_usd_micros as an integer; got %q", buf.String())
	}
	var got Event
	if err := json.Unmarshal([]byte(strings.TrimSpace(buf.String())), &got); err != nil {
		t.Fatalf("not valid JSON: %v", err)
	}
	if got.CostUsdMicros != 54404 {
		t.Errorf("CostUsdMicros round-trip = %d, want 54404", got.CostUsdMicros)
	}
}

// CostUsdMicros is omitempty: an event WITHOUT a cost (every iteration/gate/converge
// event, and an echo/dry agent phase) must not emit the key at all — keeping those
// lines byte-for-byte identical to before this field existed, which is what preserves
// the existing iteration-event assertions.
func TestEmit_CostUsdMicrosOmittedWhenZero(t *testing.T) {
	var buf bytes.Buffer
	tr := NewTracer(&buf)
	if err := tr.Emit(Event{Kind: "iteration", Name: "1", Status: "ok", DurationMs: 4200}); err != nil {
		t.Fatalf("Emit: %v", err)
	}
	if strings.Contains(buf.String(), "cost_usd_micros") {
		t.Errorf("a zero cost must be omitted so iteration events are unchanged; got %q", buf.String())
	}
}

// RunID is omitempty: an Event with no RunID set (and a Tracer whose RunID was
// never set either) must not emit the run_id key at all — keeping every
// existing golden-byte trace-line assertion in this file byte-for-byte
// unchanged across this field's addition.
func TestEvent_RunIDOmitemptyWhenUnset(t *testing.T) {
	var buf bytes.Buffer
	tr := NewTracer(&buf) // RunID left at its zero value ""
	if err := tr.Emit(Event{Kind: "iteration", Name: "1", Status: "ok"}); err != nil {
		t.Fatalf("Emit: %v", err)
	}
	if strings.Contains(buf.String(), "run_id") {
		t.Errorf("empty RunID should be omitted, got %q", buf.String())
	}
}

// Emit must auto-stamp an event's RunID from the Tracer's RunID field when the
// event itself left RunID unset — this is what threads one process's run_id
// through every event it emits without every constructor (GateEvent,
// DecisionEvent, …) needing to know about RunID at all.
func TestTracer_StampsRunIDOnEmit(t *testing.T) {
	var buf bytes.Buffer
	tr := NewTracer(&buf)
	tr.RunID = "1234abcd-deadbeef"
	if err := tr.Emit(Event{Kind: "gate", Name: "lint", Status: "PASS"}); err != nil {
		t.Fatalf("Emit: %v", err)
	}
	var got Event
	if err := json.Unmarshal([]byte(strings.TrimSpace(buf.String())), &got); err != nil {
		t.Fatalf("not valid JSON: %v", err)
	}
	if got.RunID != tr.RunID {
		t.Errorf("RunID = %q, want tracer's RunID %q", got.RunID, tr.RunID)
	}
}

// An Event constructed with RunID already set must NOT be clobbered by Emit's
// auto-stamp — locks in the exact override contract (auto-stamp fills in only
// when the caller left RunID empty).
func TestTracer_DoesNotOverrideExplicitEventRunID(t *testing.T) {
	var buf bytes.Buffer
	tr := NewTracer(&buf)
	tr.RunID = "tracer-run-id"
	if err := tr.Emit(Event{Kind: "gate", Name: "lint", Status: "PASS", RunID: "explicit-run-id"}); err != nil {
		t.Fatalf("Emit: %v", err)
	}
	var got Event
	if err := json.Unmarshal([]byte(strings.TrimSpace(buf.String())), &got); err != nil {
		t.Fatalf("not valid JSON: %v", err)
	}
	if got.RunID != "explicit-run-id" {
		t.Errorf("RunID = %q, want the explicitly-set %q to survive Emit's auto-stamp", got.RunID, "explicit-run-id")
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

// ── Constructor helper tests (seventh-wave-data-realism.md §方向1) ──────────

func TestGateEvent_Fields(t *testing.T) {
	ev := GateEvent("lint", "PASS", "0 issues")
	if ev.Kind != "gate" || ev.Name != "lint" || ev.Status != "PASS" || ev.Detail != "0 issues" {
		t.Errorf("GateEvent = %+v, want kind=gate name=lint status=PASS detail=0 issues", ev)
	}
}

func TestGateEvent_EmptyDetail(t *testing.T) {
	ev := GateEvent("test", "NA", "")
	if ev.Kind != "gate" || ev.Name != "test" || ev.Status != "NA" || ev.Detail != "" {
		t.Errorf("GateEvent with empty detail = %+v", ev)
	}
}

func TestDecisionEvent_Fields(t *testing.T) {
	ev := DecisionEvent("implementer", "downtier to haiku (spend_ratio=0.85)")
	if ev.Kind != "decision" || ev.Name != "implementer" || ev.Status != "ok" {
		t.Errorf("DecisionEvent = %+v, want kind=decision name=implementer status=ok", ev)
	}
	if ev.Detail == "" {
		t.Error("DecisionEvent Detail must be non-empty")
	}
}

func TestOverloadEvent_Fields(t *testing.T) {
	ev := OverloadEvent("implementer", "backoff 4s attempt 1/3")
	if ev.Kind != "overload_backoff" || ev.Name != "implementer" || ev.Status != "retry" {
		t.Errorf("OverloadEvent = %+v, want kind=overload_backoff name=implementer status=retry", ev)
	}
}

func TestStaleEvent_Fields(t *testing.T) {
	ev := StaleEvent("iter 3", "roadmap_flat + gate_unchanged")
	if ev.Kind != "stale_increment" || ev.Name != "iter 3" || ev.Status != "stale" {
		t.Errorf("StaleEvent = %+v, want kind=stale_increment name=iter3 status=stale", ev)
	}
}

func TestErrorEvent_Fields(t *testing.T) {
	ev := ErrorEvent("implementer", "overload", "recovered", "529 overload, retried after 15s")
	if ev.Kind != "error" || ev.Name != "implementer" || ev.Status != "recovered" {
		t.Errorf("ErrorEvent = %+v, want kind=error name=implementer status=recovered", ev)
	}
	if !strings.Contains(ev.Detail, "[overload]") {
		t.Errorf("ErrorEvent Detail must carry the error type tag; got %q", ev.Detail)
	}
}

func TestErrorEvent_FatalDetail(t *testing.T) {
	ev := ErrorEvent("config", "config", "failed", "no agent executor configured")
	if !strings.Contains(ev.Detail, "[config]") {
		t.Errorf("ErrorEvent Detail must carry the error type tag; got %q", ev.Detail)
	}
}
