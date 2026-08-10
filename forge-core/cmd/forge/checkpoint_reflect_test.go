package main

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
	"forgeos/forge-core/internal/memory"
	"forgeos/forge-core/internal/persist"
	"forgeos/forge-core/internal/trace"
)

// checkpointHook is the per-iteration persistence+trace point. Invoking the
// returned closure must write the iteration's snapshot to <root>/.forge and emit
// a matching "iteration" trace event carrying the measured signals AND the
// iteration's measured wall-clock duration (the value scorecard p95_latency reads,
// which read 0 before the trace-latency fix). The duration is passed in by the loop
// here, so the test feeds a known value and asserts the trace event carries exactly it.
func TestCheckpointHook_PersistsAndTraces(t *testing.T) {
	root := t.TempDir()
	mkdir(t, filepath.Join(root, ".forge"))
	var buf bytes.Buffer
	var logs []string
	o := runOpts{root: root, mode: "balanced"}
	wf := asset.Workflow{Stage: "evolve"}
	// Unbudgeted run: the budget never bills, so the checkpoint's SpentUsdMicros must be 0
	// (asserted below) — proving the new field is byte-identical/back-compat absent a budget.
	hook := checkpointHook(o, wf, trace.NewTracer(&buf), &runBudget{}, func(s string) { logs = append(logs, s) }, nil, nil)

	const wantDurationMs int64 = 4200 // a known measured-iteration duration the loop would pass.
	if err := hook(2, converge.Signals{RoadmapCompletion: 0.75, GatesGreen: true}, wantDurationMs); err != nil {
		t.Fatalf("checkpoint hook: %v", err)
	}

	cp, found, err := persist.Load(checkpointPath(root))
	if err != nil || !found {
		t.Fatalf("checkpoint: found=%v err=%v", found, err)
	}
	if cp.Iteration != 2 || cp.RoadmapCompletion != 0.75 || !cp.GatesGreen || cp.Mode != "balanced" {
		t.Errorf("checkpoint = %+v, want iter 2 / 0.75 / green / balanced", cp)
	}
	if cp.SpentUsdMicros != 0 {
		t.Errorf("an unbudgeted run must checkpoint SpentUsdMicros=0 (back-compat); got %d", cp.SpentUsdMicros)
	}
	if cp.UpdatedAtUnix == 0 || cp.UpdatedAtUnix > time.Now().Unix()+5 {
		t.Errorf("UpdatedAtUnix = %d, want a recent main-injected timestamp", cp.UpdatedAtUnix)
	}
	ev := lastTraceEvent(t, buf.String())
	if ev.Kind != "iteration" || ev.Name != "2" || ev.Status != "ok" {
		t.Errorf("trace event = %+v, want iteration/2/ok", ev)
	}
	// The trace must record the iteration's REAL duration (not 0) — this is the
	// trace-latency fix: scorecard p95_latency reads duration_ms off these events.
	if ev.DurationMs != wantDurationMs {
		t.Errorf("trace DurationMs = %d, want %d (the measured iteration wall-clock the loop passed)", ev.DurationMs, wantDurationMs)
	}
	if !strings.Contains(ev.Detail, "roadmap=75%") {
		t.Errorf("trace detail = %q, want measured signals", ev.Detail)
	}
}

// The OnIteration hook must ALSO append one memory entry per round — the run
// trajectory recorded for cross-session recall. It is a KindLesson on the stage
// topic, carrying this round's measured signals and iteration number. (Honesty:
// this records the real dry-run trajectory, not a fabricated agent finding.)
// checkpointHook must append a trajectory KindLesson entry on every iteration, plus a
// KindGap when gates are not green (the Reflect step). With nil ledgers (no real agent
// output) and GatesGreen=false at iter 3 (>=2): trajectory + gap + recurring-failure decision = 3 entries.
func TestCheckpointHook_AppendsMemoryEntry(t *testing.T) {
	root := t.TempDir()
	mkdir(t, filepath.Join(root, ".forge"))
	var buf bytes.Buffer
	hook := checkpointHook(runOpts{root: root, mode: "balanced"}, asset.Workflow{Stage: "evolve"},
		trace.NewTracer(&buf), &runBudget{}, func(string) {}, nil, nil)

	if err := hook(3, converge.Signals{RoadmapCompletion: 0.4, GatesGreen: false}, 0); err != nil {
		t.Fatalf("checkpoint hook: %v", err)
	}

	entries, err := memory.Load(memoryPath(root))
	if err != nil {
		t.Fatalf("load memory: %v", err)
	}
	// iter>=2 with GatesGreen=false: trajectory (KindLesson) + gap (KindGap) + recurring-failure decision (KindDecision) = 3.
	if len(entries) != 3 {
		t.Fatalf("hook should append 3 entries (trajectory + gate-gap + recurring-decision), got %d: %+v", len(entries), entries)
	}
	traj := entries[0]
	if traj.Kind != memory.KindLesson || traj.Topic != "evolve" || traj.Iteration != 3 {
		t.Errorf("trajectory entry = %+v, want KindLesson / topic evolve / iter 3", traj)
	}
	if !strings.Contains(traj.Detail, "roadmap=40%") || !strings.Contains(traj.Detail, "gates_green=false") {
		t.Errorf("trajectory detail = %q, want measured signals", traj.Detail)
	}
	gap := entries[1]
	if gap.Kind != memory.KindGap || gap.Topic != "evolve" || gap.Iteration != 3 {
		t.Errorf("gap entry = %+v, want KindGap / topic evolve / iter 3", gap)
	}
	if !strings.Contains(gap.Detail, "gates not green") {
		t.Errorf("gap detail = %q, want 'gates not green'", gap.Detail)
	}
	dec := entries[2]
	if dec.Kind != memory.KindDecision || dec.Iteration != 3 {
		t.Errorf("recurring-decision entry = %+v, want KindDecision / iter 3", dec)
	}
	if !strings.Contains(dec.Detail, "RECURRING gate failure") {
		t.Errorf("recurring-decision detail = %q, want 'RECURRING gate failure'", dec.Detail)
	}
	if traj.CreatedAtUnix == 0 {
		t.Errorf("entry must carry a main-injected timestamp; got %d", traj.CreatedAtUnix)
	}
}

// iter 1 with gates not green: only trajectory + gap (no recurring-decision yet).
func TestCheckpointHook_FirstIterNoRecurringDecision(t *testing.T) {
	root := t.TempDir()
	mkdir(t, filepath.Join(root, ".forge"))
	var buf bytes.Buffer
	hook := checkpointHook(runOpts{root: root, mode: "balanced"}, asset.Workflow{Stage: "evolve"},
		trace.NewTracer(&buf), &runBudget{}, func(string) {}, nil, nil)

	if err := hook(1, converge.Signals{RoadmapCompletion: 0.3, GatesGreen: false}, 0); err != nil {
		t.Fatalf("checkpoint hook: %v", err)
	}

	entries, err := memory.Load(memoryPath(root))
	if err != nil {
		t.Fatalf("load memory: %v", err)
	}
	if len(entries) != 2 {
		t.Fatalf("iter 1 must yield 2 entries (trajectory + gap, no recurring-decision yet); got %d: %+v", len(entries), entries)
	}
	for _, e := range entries {
		if e.Kind == memory.KindDecision {
			t.Errorf("iter 1 must NOT emit KindDecision (recurring check needs i>=2); got %+v", e)
		}
	}
}

// Fail-closed honesty: when the checkpoint write fails, the loop must NOT pretend
// it succeeded — the trace event status flips to a failure marker and a loud
// warning is logged, rather than silently dropping the recovery state.
func TestCheckpointHook_WriteFailureIsSurfaced(t *testing.T) {
	root := t.TempDir()
	// Make checkpointPath unwritable by planting a DIRECTORY where the file (and
	// its .tmp sibling) must go, so persist.Save's open/rename fails.
	mkdir(t, filepath.Join(root, ".forge"))
	mkdir(t, checkpointPath(root))
	var buf bytes.Buffer
	var logs []string
	hook := checkpointHook(runOpts{root: root}, asset.Workflow{Stage: "evolve"},
		trace.NewTracer(&buf), &runBudget{}, func(s string) { logs = append(logs, s) }, nil, nil)

	hookErr := hook(1, converge.Signals{RoadmapCompletion: 0.1}, 0)
	if hookErr == nil || !strings.Contains(hookErr.Error(), "persist iteration checkpoint") {
		t.Fatalf("checkpoint write failure must stop the loop: %v", hookErr)
	}

	ev := lastTraceEvent(t, buf.String())
	if ev.Status != "checkpoint-write-failed" {
		t.Errorf("trace status = %q, want checkpoint-write-failed on a failed Save", ev.Status)
	}
	if !containsSub(logs, "checkpoint write failed") {
		t.Errorf("a failed checkpoint write must be logged loudly; logs=%v", logs)
	}
}

// openTracer must create <root>/.forge and APPEND to trace.jsonl across calls, so
// a --resume continues the same audit trail instead of truncating prior history.
func TestOpenTracer_CreatesDirAndAppends(t *testing.T) {
	root := t.TempDir()
	tr, closeFn, err := openTracer(root)
	if err != nil {
		t.Fatalf("openTracer: %v", err)
	}
	if err := tr.Emit(trace.Event{Kind: "iteration", Name: "1", Status: "ok"}); err != nil {
		t.Fatalf("emit: %v", err)
	}
	closeFn()
	// Second open (simulating a resume) must append, not truncate.
	tr2, closeFn2, err := openTracer(root)
	if err != nil {
		t.Fatalf("openTracer #2: %v", err)
	}
	if err := tr2.Emit(trace.Event{Kind: "iteration", Name: "2", Status: "ok"}); err != nil {
		t.Fatalf("emit #2: %v", err)
	}
	closeFn2()
	data, err := os.ReadFile(filepath.Join(root, ".forge", "trace.jsonl"))
	if err != nil {
		t.Fatalf("read trace: %v", err)
	}
	if n := bytes.Count(data, []byte("\n")); n != 2 {
		t.Errorf("trace has %d lines, want 2 (append preserved the first across reopen)", n)
	}
}

// --- test helpers (checkpoint group) -----------------------------------------

// lastTraceEvent decodes the final JSONL record from a trace buffer.
func lastTraceEvent(t *testing.T, jsonl string) trace.Event {
	t.Helper()
	lines := strings.Split(strings.TrimRight(jsonl, "\n"), "\n")
	if len(lines) == 0 || lines[0] == "" {
		t.Fatalf("no trace events emitted; buf=%q", jsonl)
	}
	var ev trace.Event
	if err := json.Unmarshal([]byte(lines[len(lines)-1]), &ev); err != nil {
		t.Fatalf("decode trace line: %v", err)
	}
	return ev
}

func containsSub(ss []string, sub string) bool {
	for _, s := range ss {
		if strings.Contains(s, sub) {
			return true
		}
	}
	return false
}
