package main

import (
	"fmt"
	"os"

	"forgeos/forge-core/internal/runlock"
	"forgeos/forge-core/internal/trace"
)

// acquireRunLock claims the process-level advisory lock on root's .forge/
// (see internal/runlock) so two concurrent `forge run`/`forge evolve`
// invocations against the same root can never race on shared .forge/ state.
// On contention (or any other Acquire failure) it prints an actionable error
// and returns nil — callers must treat a nil return as "exit 1", never retry.
// cmd names the calling command ("forge run"/"forge evolve") for the error
// prefix, matching every other top-level error message in this package.
func acquireRunLock(root, cmd string) *runlock.Lock {
	lock, err := runlock.Acquire(root)
	if err != nil {
		fmt.Fprintf(os.Stderr, "%s: %v\n", cmd, err)
		return nil
	}
	return lock
}

// stampRunID sets t's process-correlation RunID (internal/runlock.NewRunID,
// trace.Tracer.RunID) so every trace.jsonl line this process emits can be
// attributed to this run. Called once by openTracer, the single shared
// constructor for both `forge run` (via openRunResources) and `forge evolve`.
func stampRunID(t *trace.Tracer) {
	t.RunID = runlock.NewRunID()
}
