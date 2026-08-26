package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/trace"
)

func TestResumeTraceEntryPointsContinueBeforeDoctorEvents(t *testing.T) {
	tests := []struct {
		name string
		open func(string) (*trace.Tracer, func(), error)
	}{
		{
			name: "chain-run",
			open: func(root string) (*trace.Tracer, func(), error) {
				tracer, closeTrace, _, err := openRunResources(root, "", func(string) {}, "resumed-run")
				return tracer, closeTrace, err
			},
		},
		{
			name: "evolve",
			open: func(root string) (*trace.Tracer, func(), error) {
				return openResumeTracer(root, loopResumeState{runID: "resumed-run"}, func(string) {})
			},
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			root := resumeTraceEntryRoot(t)
			tracer, closeTrace, err := test.open(root)
			if err != nil {
				t.Fatal(err)
			}
			if tracer.RunID != "resumed-run" {
				t.Fatalf("run_id=%q, want resumed-run", tracer.RunID)
			}
			closeTrace()
			assertFirstNewTraceSequence(t, root, 42)
		})
	}
}

func TestResumeTraceFloorSurvivesBackupReplacementDuringOpen(t *testing.T) {
	root := resumeTraceEntryRoot(t)
	other := traceSequenceLine("other-run", 1, trace.FormatV1) + "\n"
	largeCurrent := strings.Repeat(other, (17<<20)/len(other)+1)
	if err := os.WriteFile(filepath.Join(root, ".forge", "trace.jsonl"), []byte(largeCurrent), 0o600); err != nil {
		t.Fatal(err)
	}
	tracer, closeTrace, _, err := openRunResources(root, "", func(string) {}, "resumed-run")
	if err != nil {
		t.Fatal(err)
	}
	if tracer.RunID != "resumed-run" {
		t.Fatalf("run_id=%q, want resumed-run", tracer.RunID)
	}
	closeTrace()
	assertFirstNewTraceSequence(t, root, 42)
}

func TestOpenTracerRejectsIncompleteAppendBoundary(t *testing.T) {
	root := t.TempDir()
	dir := filepath.Join(root, ".forge")
	if err := os.Mkdir(dir, 0o700); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(dir, "trace.jsonl")
	seed := traceSequenceLine("old-run", 1, trace.FormatV1)
	if err := os.WriteFile(path, []byte(seed), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, closeTrace, err := openTracer(root); err == nil {
		closeTrace()
		t.Fatal("incomplete final trace record must fail before append")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != seed {
		t.Fatal("framing failure mutated the trace")
	}
}

func resumeTraceEntryRoot(t *testing.T) string {
	t.Helper()
	root := t.TempDir()
	if err := os.Mkdir(filepath.Join(root, ".forge"), 0o700); err != nil {
		t.Fatal(err)
	}
	writeTraceEntrySeed(t, root, "trace.jsonl", 4)
	writeTraceEntrySeed(t, root, "trace.jsonl.1", 41)
	return root
}

func writeTraceEntrySeed(t *testing.T, root, name string, sequence int) {
	t.Helper()
	event := trace.Event{Format: trace.FormatV1, Seq: sequence, RunID: "resumed-run", Kind: "seed"}
	encoded, err := json.Marshal(event)
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, ".forge", name)
	if err := os.WriteFile(path, append(encoded, '\n'), 0o600); err != nil {
		t.Fatal(err)
	}
}

func assertFirstNewTraceSequence(t *testing.T, root string, want int) {
	t.Helper()
	data, err := os.ReadFile(filepath.Join(root, ".forge", "trace.jsonl"))
	if err != nil {
		t.Fatal(err)
	}
	for _, line := range strings.Split(strings.TrimSpace(string(data)), "\n") {
		var event trace.Event
		if json.Unmarshal([]byte(line), &event) != nil || event.RunID != "resumed-run" || event.Kind == "seed" {
			continue
		}
		if event.Seq != want {
			t.Fatalf("first resumed event seq=%d, want %d", event.Seq, want)
		}
		return
	}
	t.Fatal("resume entry point emitted no doctor event")
}
