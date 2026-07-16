// Package trace is forge-core's structured-observability writer: it turns the
// runtime's event stream into machine-readable JSONL so a 24h autonomous run is
// auditable after the fact, not flying blind on free-text logs. The existing
// runtime only has `Log func(string)` (prose, ungreppable, unparseable); this
// package records the SAME milestones — iteration boundaries, gate verdicts,
// agent phases, convergence checks — as one self-describing JSON object per
// line, so a tail/jq/replay tool can reconstruct exactly what happened and when.
//
// Two deliberate design choices:
//   - Every Emit is serialized under a mutex even though today's runtime is
//     single-threaded. A trace line MUST be a complete, well-formed JSON object;
//     interleaving two concurrent writes would corrupt the stream. Locking now
//     keeps the format safe when future phases run in parallel, at the cost of a
//     single uncontended lock per event (negligible against agent/gate latency).
//   - Time is taken through an injectable Now func, not time.Now directly. Span
//     measures wall-clock duration, and duration that can only be observed via
//     the real clock is untestable; a fake clock makes Span's output exactly
//     determinable in tests (and lets a replay harness control time if needed).
//
// Fail-closed posture: the writer surfaces marshal/write errors to the caller
// rather than silently dropping an event, because a missing trace line is a
// blind spot in the audit record — the one thing this package exists to prevent.
package trace

import (
	"encoding/json"
	"fmt"
	"io"
	"sync"
	"time"
)

// Event is one structured record in the trace stream. It is intentionally flat
// and string-typed so it round-trips through JSON without bespoke decoders and
// stays greppable by tools that do not know forge-core's internal types. Seq is
// assigned by the Tracer (callers never set it), giving every line a total order
// independent of any wall-clock timestamp. The json tags are the on-disk
// contract that downstream tooling reads, so they are stable and lower_snake.
//
// Format carries the on-disk format version identifier (e.g. "forgeos.trace.v1"),
// so downstream tooling can detect format changes. An empty value (pre-format
// versioning) is treated as "forgeos.trace.v1" for backward compatibility.
// omitempty keeps old events without the field byte-for-byte identical on disk.
//
// Kind identifies the event family. The canonical kinds are:
//
//	"iteration"       — one full loop iteration (measured signals, duration)
//	"agent"           — one LLM agent phase (with cost+model attribution)
//	"gate"            — one harness gate verdict (PASS/FAIL/NA)
//	"decision"        — a runtime decision (tier up/down, cost guard trip)
//	"converge"        — convergence check verdict
//	"error"           — recoverable or fatal error (overload, timeout, config)
//	"overload_backoff" — 529/overload retry pause
//	"stale_increment" — no-progress increment on the doom-loop guard
//	"doctor"          — forge doctor diagnostic result
//	"memory_compact"  — memory compaction event
type Event struct {
	Format     string `json:"_format,omitempty"`
	Seq        int    `json:"seq"`         // monotonic 1,2,3… assigned by the Tracer
	Kind       string `json:"kind"`        // event family: see kind constants above
	Name       string `json:"name"`        // the specific phase/gate name within the kind
	Status     string `json:"status"`      // verdict/outcome: PASS|FAIL|NA|ok|timeout|…
	DurationMs int64  `json:"duration_ms"` // wall-clock span in ms; 0 for instantaneous events
	// CostUsdMicros is the LLM dollar cost of this event in integer MICRO-dollars
	// (USD x 1e6), OPAQUE to this package — duration_ms is a generic wall-clock
	// measurement every event carries, whereas cost is supplied ONLY by an LLM
	// executor that actually billed (the claude JSON's total_cost_usd, converted by
	// the caller). Integer microdollars avoid the float-JSON jitter a raw USD double
	// would print (0.0544035 round-trips exactly as 54403); omitempty keeps every
	// event WITHOUT a cost (iteration/gate/converge, and echo/dry agent phases) byte-
	// for-byte identical on disk, so existing iteration-event assertions are intact.
	CostUsdMicros int64 `json:"cost_usd_micros,omitempty"`
	// Model is the LLM model/tier this event was billed against, OPAQUE to this
	// package exactly like CostUsdMicros — trace has no notion of what a "model" is
	// or where the string comes from; it only carries it. It is supplied ONLY by the
	// same real LLM-executor cost path that fills CostUsdMicros (a routed claude
	// tier), so a non-LLM event (iteration/gate/converge) and an echo/dry agent phase
	// leave it empty. omitempty keeps every event WITHOUT a model byte-for-byte
	// identical on disk, so existing iteration/gate/cost-event assertions stay intact;
	// downstream the scorecard producer reads it (json tag `model`) to attribute a
	// billed cost to the model that incurred it.
	Model  string `json:"model,omitempty"`
	Detail string `json:"detail,omitempty"` // free-text context, omitted from JSON when empty
	// RunID correlates every event in one trace.jsonl to the process that
	// wrote it (forge-core/internal/runlock.NewRunID). A caller normally
	// leaves this empty and lets Emit auto-stamp it from the Tracer's RunID
	// field; omitempty keeps every event predating this field, and every
	// event from a Tracer whose RunID was never set, byte-for-byte
	// identical on disk (back-compat).
	RunID string `json:"run_id,omitempty"`
}

// Tracer serializes Events to an io.Writer as JSONL (one JSON object per line).
// It owns Seq assignment and write ordering; callers just describe what
// happened. The zero value is NOT ready — use NewTracer so the writer is set.
type Tracer struct {
	mu  sync.Mutex // guards seq and the writer so each line is emitted atomically
	w   io.Writer  // destination; one '\n'-terminated JSON object is written per Emit
	seq int        // last assigned sequence number; pre-incremented on each Emit

	// Now supplies the current time for Span's duration measurement. It is a
	// field (not a hardcoded time.Now call) purely so tests can inject a
	// deterministic clock; production leaves it nil and NewTracer defaults it to
	// time.Now. Keep all time reads going through this so duration stays testable.
	Now func() time.Time

	// RunID is this process's run-correlation id (see Event.RunID), set once
	// by the caller right after NewTracer (cmd/forge's openTracer stamps it
	// via runlock.NewRunID). The zero value "" is back-compat: Emit only
	// stamps an event's RunID when the event itself left it empty, so a
	// Tracer that never sets this behaves exactly as before this field
	// existed.
	RunID string
}

// NewTracer returns a Tracer writing JSONL to w. Now defaults to time.Now so the
// common path needs no configuration; tests overwrite the exported Now field
// with a fake clock to get deterministic Span durations. Seq starts at 0 so the
// first emitted event is 1 (humans count runs from one, and 0 reads as "unset").
func NewTracer(w io.Writer) *Tracer {
	return &Tracer{w: w, Now: time.Now}
}

// Emit assigns the next sequence number to ev and writes it as a single JSONL
// line. It is the only place that touches seq or the writer, and it does so
// under the lock, so concurrent callers can never interleave half-written
// objects or duplicate a sequence number. Marshalling is delegated to the pure
// encode function; Emit is just the IO/locking shell around it. A marshal or
// write failure is returned (fail-closed) rather than swallowed, because a
// dropped trace line is exactly the audit gap this package guards against.
func (t *Tracer) Emit(ev Event) error {
	t.mu.Lock()
	defer t.mu.Unlock()

	t.seq++
	ev.Seq = t.seq
	if ev.Format == "" {
		ev.Format = "forgeos.trace.v1"
	}
	if ev.RunID == "" {
		ev.RunID = t.RunID // auto-stamp; never clobbers a caller-supplied RunID
	}
	line, err := encode(ev)
	if err != nil {
		return fmt.Errorf("trace: encoding event seq=%d kind=%q: %w", ev.Seq, ev.Kind, err)
	}
	if _, err := t.w.Write(line); err != nil {
		return fmt.Errorf("trace: writing event seq=%d: %w", ev.Seq, err)
	}
	return nil
}

// Span starts timing a kind/name and returns a closure that, when called,
// computes the elapsed wall-clock time and Emits the finished Event. This is the
// ergonomic way to record a phase: `defer t.Span("gate", "lint")("PASS", out)`
// captures start-now and emits one event with the measured DurationMs at the
// close. Timing uses the injectable Now (see the field doc) so the duration is
// deterministic under a fake clock. The returned closure swallows Emit's error:
// a span finisher is meant to be deferred at a call site that has no error path,
// and a lost trace line must never mask the real work's outcome — use Emit
// directly when the caller needs to observe a write failure.
func (t *Tracer) Span(kind, name string) func(status, detail string) {
	start := t.Now()
	return func(status, detail string) {
		dur := t.Now().Sub(start)
		_ = t.Emit(Event{
			Kind:       kind,
			Name:       name,
			Status:     status,
			DurationMs: dur.Milliseconds(),
			Detail:     detail,
		})
	}
}

// encode is the pure Event→JSONL-bytes step, factored out of Emit so the wire
// format is unit-testable without a Tracer, a writer, or the lock. It returns
// one compact JSON object followed by a single '\n' — the JSONL line framing —
// so callers append nothing and the newline is part of the encoded record. It
// performs no IO and reads no shared state, so it is safe to call concurrently.
func encode(ev Event) ([]byte, error) {
	b, err := json.Marshal(ev)
	if err != nil {
		return nil, err
	}
	return append(b, '\n'), nil
}

// ── Constructor helpers (seventh-wave-data-realism.md §方向1) ────────────────

// GateEvent builds a trace event for one harness gate result.
// name is the gate name (e.g. "lint", "test"), status is "PASS"|"FAIL"|"NA",
// and detail carries the gate's output/verdict text.
func GateEvent(name, status, detail string) Event {
	return Event{Kind: "gate", Name: name, Status: status, Detail: detail}
}

// DecisionEvent builds a trace event for a runtime decision (tier up/down,
// cost guard trip, or adaptive behavior). name identifies the decision context
// (e.g. the phase name), detail describes what was decided and why.
func DecisionEvent(name, detail string) Event {
	return Event{Kind: "decision", Name: name, Status: "ok", Detail: detail}
}

// OverloadEvent builds a trace event for a 529/overload retry pause. name
// identifies the affected phase, detail describes the backoff duration and
// the attempt number (e.g. "backoff 4s attempt 1/3").
func OverloadEvent(name, detail string) Event {
	return Event{Kind: "overload_backoff", Name: name, Status: "retry", Detail: detail}
}

// StaleEvent builds a trace event for a no-progress increment on the
// doom-loop guard. name identifies the iteration (e.g. "iter 3"), detail
// describes why progress was flat (e.g. "roadmap_flat + gate_unchanged").
func StaleEvent(name, detail string) Event {
	return Event{Kind: "stale_increment", Name: name, Status: "stale", Detail: detail}
}

// ErrorEvent builds a trace event for a recoverable or fatal error.
// name identifies the source (phase/gate), errorType classifies the kind
// (e.g. "overload", "timeout", "config"), status is the outcome
// ("recovered"|"failed"), and detail carries the error message.
func ErrorEvent(name, errorType, status, detail string) Event {
	return Event{Kind: "error", Name: name, Status: status, Detail: fmt.Sprintf("[%s] %s", errorType, detail)}
}
