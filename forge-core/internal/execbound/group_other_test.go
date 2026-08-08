//go:build !unix

package execbound

import (
	"context"
	"strings"
	"testing"
	"time"
)

// On non-unix, group teardown is unavailable by construction: the capability
// query reports false, and the kill path emits the honest degradation Log line
// (only when a kill event actually fired — never on the happy path).
func TestExecbound_GroupKillAvailable_NonUnix(t *testing.T) {
	if GroupKillAvailable() {
		t.Error("GroupKillAvailable must be false on non-unix")
	}
	var logs []string
	opts := Options{Timeout: 300 * time.Millisecond, Log: func(s string) { logs = append(logs, s) }}
	res := Run(context.Background(), []string{"sleep", "30"}, opts, CaptureCombined, Spec{})
	if !res.TimedOut() {
		t.Fatalf("must report TimedOut; CtxErr=%v", res.CtxErr)
	}
	if len(logs) != 1 || !strings.Contains(logs[0], "process-group teardown unavailable") {
		t.Errorf("kill path must emit exactly one degradation log line; got %v", logs)
	}
	// Happy path: a clean exit must not log.
	logs = nil
	ok := Run(context.Background(), []string{"true"}, Options{Log: func(s string) { logs = append(logs, s) }}, CaptureCombined, Spec{})
	if ok.Err != nil {
		t.Fatalf("clean exit: %v", ok.Err)
	}
	if len(logs) != 0 {
		t.Errorf("clean exit must not log degradation; got %v", logs)
	}
}
