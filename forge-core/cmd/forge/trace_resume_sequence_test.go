package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/trace"
)

func TestResumeTraceSequenceUsesCurrentAndRotatedMaximum(t *testing.T) {
	root := newTraceSequenceRoot(t)
	writeTraceSequenceFile(t, root, "trace.jsonl", strings.Join([]string{
		traceSequenceLine("target", 4, trace.FormatV1),
		traceSequenceLine("other", 90, trace.FormatV1),
	}, "\n"))
	writeTraceSequenceFile(t, root, "trace.jsonl.1", strings.Join([]string{
		traceSequenceLine("target", 12, trace.FormatV1),
		traceSequenceLine("other", 70, "forgeos.trace.v2"),
	}, "\n"))
	tracer, output := newResumeSequenceTracer("target")
	if err := resumeTraceSequence(root, tracer); err != nil {
		t.Fatal(err)
	}
	assertNextTraceSequence(t, tracer, output, 13)
}

func TestResumeTraceSequenceMatchesExactRunID(t *testing.T) {
	root := newTraceSequenceRoot(t)
	writeTraceSequenceFile(t, root, "trace.jsonl", strings.Join([]string{
		traceSequenceLine("run", 3, trace.FormatV1),
		traceSequenceLine("run-child", 99, trace.FormatV1),
	}, "\n"))
	tracer, output := newResumeSequenceTracer("run")
	if err := resumeTraceSequence(root, tracer); err != nil {
		t.Fatal(err)
	}
	assertNextTraceSequence(t, tracer, output, 4)
}

func TestResumeTraceSequenceAcceptsLegacyFormatAndIgnoresOtherRunDamage(t *testing.T) {
	root := newTraceSequenceRoot(t)
	writeTraceSequenceFile(t, root, "trace.jsonl", strings.Join([]string{
		"{not-json}",
		traceSequenceLine("other", -8, trace.FormatV1),
		traceSequenceLine("target", 7, ""),
		traceSequenceLine("target", 5, trace.FormatV1),
	}, "\n"))
	tracer, output := newResumeSequenceTracer("target")
	if err := resumeTraceSequence(root, tracer); err != nil {
		t.Fatal(err)
	}
	assertNextTraceSequence(t, tracer, output, 8)
}

func TestResumeTraceSequenceFailsClosedOnNonPositiveSequence(t *testing.T) {
	for _, sequence := range []int{-8, 0} {
		t.Run(fmt.Sprint(sequence), func(t *testing.T) {
			root := newTraceSequenceRoot(t)
			writeTraceSequenceFile(t, root, "trace.jsonl", traceSequenceLine("target", sequence, trace.FormatV1))
			tracer, output := newResumeSequenceTracer("target")
			if err := resumeTraceSequence(root, tracer); err == nil {
				t.Fatal("non-positive sequence must fail")
			}
			assertNextTraceSequence(t, tracer, output, 1)
		})
	}
}

func TestResumeTraceSequenceFailsClosedOnFutureFormatForRun(t *testing.T) {
	root := newTraceSequenceRoot(t)
	writeTraceSequenceFile(t, root, "trace.jsonl", traceSequenceLine("target", 70, "forgeos.trace.v2"))
	tracer, output := newResumeSequenceTracer("target")
	if err := resumeTraceSequence(root, tracer); err == nil {
		t.Fatal("unsupported format for resumed run must fail")
	}
	assertNextTraceSequence(t, tracer, output, 1)
}

func TestResumeTraceSequenceFailsClosedAtIntegerLimit(t *testing.T) {
	root := newTraceSequenceRoot(t)
	writeTraceSequenceFile(t, root, "trace.jsonl", traceSequenceLine("target", math.MaxInt, trace.FormatV1))
	tracer, output := newResumeSequenceTracer("target")
	if err := resumeTraceSequence(root, tracer); err == nil {
		t.Fatal("exhausted sequence must fail")
	}
	assertNextTraceSequence(t, tracer, output, 1)
}

func TestResumeTraceSequenceFailsClosedAboveIntegerLimit(t *testing.T) {
	root := newTraceSequenceRoot(t)
	line := `{"_format":"forgeos.trace.v1","seq":9223372036854775808,"run_id":"target"}`
	writeTraceSequenceFile(t, root, "trace.jsonl", line)
	tracer, output := newResumeSequenceTracer("target")
	if err := resumeTraceSequence(root, tracer); err == nil {
		t.Fatal("out-of-range sequence must fail")
	}
	assertNextTraceSequence(t, tracer, output, 1)
}

func TestResumeTraceSequenceFailsClosedOnUnsafeBackup(t *testing.T) {
	root := newTraceSequenceRoot(t)
	writeTraceSequenceFile(t, root, "trace.jsonl", traceSequenceLine("target", 8, trace.FormatV1))
	outside := filepath.Join(root, "outside")
	if err := os.WriteFile(outside, []byte("outside"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, filepath.Join(forgeDir(root), "trace.jsonl.1")); err != nil {
		t.Fatal(err)
	}
	tracer, output := newResumeSequenceTracer("target")
	if err := resumeTraceSequence(root, tracer); err == nil {
		t.Fatal("unsafe rotated trace must fail")
	}
	assertNextTraceSequence(t, tracer, output, 1)
}

func TestResumeTraceSequenceStreamsFilesBeyondLegacyWholeFileLimit(t *testing.T) {
	root := newTraceSequenceRoot(t)
	other := traceSequenceLine("other", 1, trace.FormatV1) + "\n"
	body := strings.Repeat(other, (17<<20)/len(other)+1)
	body += traceSequenceLine("target", 9, trace.FormatV1)
	writeTraceSequenceFile(t, root, "trace.jsonl", body)
	tracer, output := newResumeSequenceTracer("target")
	if err := resumeTraceSequence(root, tracer); err != nil {
		t.Fatal(err)
	}
	assertNextTraceSequence(t, tracer, output, 10)
}

func newTraceSequenceRoot(t *testing.T) string {
	t.Helper()
	root := t.TempDir()
	if err := os.Mkdir(filepath.Join(root, ".forge"), 0o700); err != nil {
		t.Fatal(err)
	}
	writeTraceSequenceFile(t, root, "trace.jsonl", "")
	return root
}

func writeTraceSequenceFile(t *testing.T, root, name, body string) {
	t.Helper()
	if err := os.WriteFile(filepath.Join(forgeDir(root), name), []byte(body), 0o600); err != nil {
		t.Fatal(err)
	}
}

func traceSequenceLine(runID string, sequence int, format string) string {
	encoded, _ := json.Marshal(trace.Event{Format: format, Seq: sequence, RunID: runID})
	return string(encoded)
}

func newResumeSequenceTracer(runID string) (*trace.Tracer, *bytes.Buffer) {
	output := &bytes.Buffer{}
	tracer := trace.NewTracer(output)
	tracer.RunID = runID
	return tracer, output
}

func assertNextTraceSequence(t *testing.T, tracer *trace.Tracer, output *bytes.Buffer, want int) {
	t.Helper()
	if err := tracer.Emit(trace.Event{Kind: "iteration", Status: "ok"}); err != nil {
		t.Fatal(err)
	}
	var event trace.Event
	if err := json.Unmarshal(bytes.TrimSpace(output.Bytes()), &event); err != nil {
		t.Fatal(err)
	}
	if event.Seq != want {
		t.Fatalf("next sequence = %d, want %d", event.Seq, want)
	}
}
