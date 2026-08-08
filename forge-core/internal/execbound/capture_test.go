package execbound

import (
	"context"
	"strconv"
	"strings"
	"testing"
)

// T16 — exact-count truncation: a stub emitting exactly cap+delta must retain
// EXACTLY cap bytes and report the exact total. Deterministic: cappedBuffer
// retains exactly cap whenever total >= cap.
func TestExecbound_Truncation_ExactCounts(t *testing.T) {
	const cap = 64 << 10
	const delta = 4096
	res := Run(context.Background(),
		[]string{"sh", "-c", "head -c " + strconv.Itoa(cap+delta) + " /dev/zero | tr '\\0' x"},
		Options{MaxOutputBytes: cap}, CaptureCombined, Spec{})
	if res.Err != nil {
		t.Fatalf("printf stub must exit 0: %v", res.Err)
	}
	if res.Retained != cap {
		t.Errorf("Retained = %d, want exactly %d", res.Retained, cap)
	}
	if res.Total != int64(cap+delta) {
		t.Errorf("Total = %d, want exactly %d", res.Total, cap+delta)
	}
	if res.Merged == nil || len(res.Merged) != cap {
		t.Errorf("Merged retained %d bytes, want %d", len(res.Merged), cap)
	}
}

// T15 — golden string: the truncation marker literal, byte-exact (closes the
// orchestrator net's weak Contains("truncated") gap). Under the cap, the same
// buffer must produce NO marker.
func TestExecbound_Marker_Golden(t *testing.T) {
	const cap = 64 << 10
	const delta = 4096
	res := Run(context.Background(),
		[]string{"sh", "-c", "head -c " + strconv.Itoa(cap+delta) + " /dev/zero | tr '\\0' x"},
		Options{MaxOutputBytes: cap}, CaptureCombined, Spec{})
	if res.Err != nil {
		t.Fatalf("printf stub must exit 0: %v", res.Err)
	}
	want := " …[output truncated: retained 65536 of 69632 bytes (--max-output-bytes)]"
	if !strings.HasSuffix(res.Rendered(), want) {
		t.Errorf("Rendered() must end with the golden marker:\n got  %q\n want suffix %q", res.Rendered(), want)
	}
	if !strings.HasSuffix(res.Observed(), want) {
		t.Errorf("Observed() must end with the golden marker (untrimmed): %q", res.Observed())
	}
	// Under-cap: no marker anywhere.
	small := Run(context.Background(), []string{"echo", "hi"}, Options{MaxOutputBytes: cap}, CaptureCombined, Spec{})
	if strings.Contains(small.Rendered(), "truncated") {
		t.Errorf("under-cap run must not mark truncation; got %q", small.Rendered())
	}
}

// CaptureCombined merges both streams into one buffer; CaptureSplit keeps them
// separate with the same retention semantics.
func TestRun_CaptureSplit_SeparatesStreams(t *testing.T) {
	res := Run(context.Background(),
		[]string{"sh", "-c", "echo out; echo err >&2"},
		Options{MaxOutputBytes: 4096}, CaptureSplit, Spec{})
	if res.Err != nil {
		t.Fatalf("stub must exit 0: %v", res.Err)
	}
	if strings.TrimSpace(string(res.Stdout)) != "out" {
		t.Errorf("Stdout = %q, want out", res.Stdout)
	}
	if strings.TrimSpace(string(res.Stderr)) != "err" {
		t.Errorf("Stderr = %q, want err", res.Stderr)
	}
	if res.Merged != nil {
		t.Error("CaptureSplit must not populate Merged")
	}
	// Split retention is capped per stream: total counts both.
	if res.Total != int64(len(res.Stdout)+len(res.Stderr)) {
		t.Errorf("Total = %d, want %d", res.Total, len(res.Stdout)+len(res.Stderr))
	}
}

// Observed preserves every retained byte (no trimming) — machine parsers keep
// the exact payload, marker appended when truncated.
func TestResult_Observed_Untrimmed(t *testing.T) {
	res := Run(context.Background(), []string{"sh", "-c", "printf '  padded  '"}, Options{}, CaptureCombined, Spec{})
	if res.Err != nil {
		t.Fatalf("stub must exit 0: %v", res.Err)
	}
	if got := res.Observed(); got != "  padded  " {
		t.Errorf("Observed() = %q, want untrimmed %q", got, "  padded  ")
	}
	if got := res.Rendered(); got != "padded" {
		t.Errorf("Rendered() = %q, want trimmed %q", got, "padded")
	}
}

// Run with an invalid Options fails before any fork: Validate error surfaces
// as Err and no process is started.
func TestRun_InvalidOptions_NoFork(t *testing.T) {
	res := Run(context.Background(), []string{"true"}, Options{Timeout: -1}, CaptureCombined, Spec{})
	if res.Err == nil || !strings.Contains(res.Err.Error(), "timeout must be >= 0") {
		t.Errorf("invalid options must fail before fork; got %v", res.Err)
	}
}
