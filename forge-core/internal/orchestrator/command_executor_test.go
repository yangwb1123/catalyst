package orchestrator

import (
	"errors"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/asset"
)

// Uses a real subprocess (echo) to prove the executor actually runs a command
// and captures its output — not a mock.
func TestCommandExecutor_RunsRealProcess(t *testing.T) {
	rec := &recorder{}
	ex := CommandExecutor{
		Build: func(p asset.Phase, mode string) []string { return []string{"echo", p.Name, mode} },
		Log:   rec.log,
	}
	if err := ex.Execute(asset.Phase{Name: "planner"}, "balanced"); err != nil {
		t.Fatalf("Execute: %v", err)
	}
	if !containsLine(rec.logs, "planner balanced") {
		t.Errorf("expected the command's output to be captured; logs=%v", rec.logs)
	}
}

// A non-zero exit must surface as a typed Failed error — fail closed, never
// silent success — and a Failed is the agent's own verdict, so not retryable.
func TestCommandExecutor_FailingCommandErrors(t *testing.T) {
	ex := CommandExecutor{Build: func(asset.Phase, string) []string { return []string{"false"} }}
	err := ex.Execute(asset.Phase{Name: "x"}, "m")
	execErr := requireExecError(t, err)
	if execErr.Kind != KindFailed {
		t.Errorf("non-zero exit: want KindFailed, got %v", execErr.Kind)
	}
	if execErr.Retryable() {
		t.Error("a non-zero exit (agent's own failure) must not be retryable")
	}
}

// An empty argv is a misconfiguration: typed KindConfig, fail closed, not a no-op.
func TestCommandExecutor_EmptyArgvErrors(t *testing.T) {
	ex := CommandExecutor{Build: func(asset.Phase, string) []string { return nil }}
	err := ex.Execute(asset.Phase{Name: "x"}, "m")
	execErr := requireExecError(t, err)
	if execErr.Kind != KindConfig {
		t.Errorf("empty argv: want KindConfig, got %v", execErr.Kind)
	}
	if execErr.Retryable() {
		t.Error("a config fault must not be retryable")
	}
}

// P22: a nil Build must return a typed KindConfig error, never panic on the call.
func TestCommandExecutor_NilBuildFailsClosed(t *testing.T) {
	ex := CommandExecutor{Build: nil}
	defer func() {
		if r := recover(); r != nil {
			t.Fatalf("nil Build must not panic; got %v", r)
		}
	}()
	err := ex.Execute(asset.Phase{Name: "x"}, "m")
	execErr := requireExecError(t, err)
	if execErr.Kind != KindConfig {
		t.Errorf("nil Build: want KindConfig, got %v", execErr.Kind)
	}
}

// A command that exceeds its Timeout must be killed (the run finishes far short
// of the command's own 5s sleep) and surface as a retryable KindTimeout.
func TestCommandExecutor_TimeoutKillsAndClassifies(t *testing.T) {
	ex := CommandExecutor{
		Build:   func(asset.Phase, string) []string { return []string{"sleep", "5"} },
		Timeout: 50 * time.Millisecond,
	}
	start := time.Now()
	err := ex.Execute(asset.Phase{Name: "slow"}, "m")
	elapsed := time.Since(start)

	execErr := requireExecError(t, err)
	if execErr.Kind != KindTimeout {
		t.Errorf("timeout: want KindTimeout, got %v", execErr.Kind)
	}
	if !execErr.Retryable() {
		t.Error("a timeout must be retryable")
	}
	// The process was actually interrupted, not waited out: well under sleep's 5s.
	if elapsed >= 2*time.Second {
		t.Errorf("expected the process to be killed promptly; took %v", elapsed)
	}
}

// A command binary that does not exist is a permanent config fault (surfaced via
// exec.ErrNotFound), classified KindConfig and not retryable.
func TestCommandExecutor_MissingBinaryIsConfig(t *testing.T) {
	ex := CommandExecutor{
		Build: func(asset.Phase, string) []string {
			return []string{"forgeos-no-such-binary-xyzzy"}
		},
	}
	err := ex.Execute(asset.Phase{Name: "x"}, "m")
	execErr := requireExecError(t, err)
	if execErr.Kind != KindConfig {
		t.Errorf("missing binary: want KindConfig, got %v", execErr.Kind)
	}
	if execErr.Retryable() {
		t.Error("a missing binary must not be retryable")
	}
}

// The recursion guard refuses to spawn once the inherited agent-call depth has
// reached the cap: a real agent re-invoking `forge --executor=command` must not
// fork-bomb. The fault is a NON-retryable KindRecursionLimit (re-running recurses
// identically) and — critically — NO subprocess is spawned (the Build command here
// would log "ran ... SHOULD-NOT-RUN" if it executed; it must not).
func TestCommandExecutor_RecursionGuardBlocksAtCap(t *testing.T) {
	t.Setenv(agentDepthEnv, "2")
	rec := &recorder{}
	ex := CommandExecutor{
		Build:    func(asset.Phase, string) []string { return []string{"echo", "SHOULD-NOT-RUN"} },
		Log:      rec.log,
		MaxDepth: 2,
	}
	err := ex.Execute(asset.Phase{Name: "x"}, "m")
	execErr := requireExecError(t, err)
	if execErr.Kind != KindRecursionLimit {
		t.Errorf("at cap: want KindRecursionLimit, got %v", execErr.Kind)
	}
	if execErr.Retryable() {
		t.Error("a recursion-limit fault must not be retryable")
	}
	if containsLine(rec.logs, "SHOULD-NOT-RUN") {
		t.Error("guard must refuse BEFORE spawning; the command must not run")
	}
}

// Below the cap, the executor spawns AND propagates an incremented depth to the
// child, so a nested forge sees parent+1 and the guard composes across processes.
// printenv echoes the injected var back through CombinedOutput -> logf, proving
// the child's environment actually carries FORGE_AGENT_DEPTH=1 (parent 0 + 1).
func TestCommandExecutor_RecursionGuardInjectsIncrementedDepth(t *testing.T) {
	t.Setenv(agentDepthEnv, "") // top-level: no inherited depth -> reads 0
	rec := &recorder{}
	ex := CommandExecutor{
		Build: func(asset.Phase, string) []string { return []string{"printenv", agentDepthEnv} },
		Log:   rec.log,
	}
	if err := ex.Execute(asset.Phase{Name: "p"}, "m"); err != nil {
		t.Fatalf("below cap must spawn: %v", err)
	}
	if !containsLine(rec.logs, "-> 1") {
		t.Errorf("child must inherit depth 1 (parent 0 + 1); logs=%v", rec.logs)
	}
}

// A malformed inherited depth must NOT block a legitimate top-level run: garbage
// reads as 0 (fail-safe), so the executor still spawns rather than refusing.
func TestCommandExecutor_RecursionGuardFailsSafeOnGarbageDepth(t *testing.T) {
	t.Setenv(agentDepthEnv, "not-a-number")
	ex := CommandExecutor{Build: func(asset.Phase, string) []string { return []string{"true"} }}
	if err := ex.Execute(asset.Phase{Name: "p"}, "m"); err != nil {
		t.Fatalf("garbage depth must fail-safe to 0 and spawn, got: %v", err)
	}
}

// The default cap (2) applies when MaxDepth is unset: at depth 2 the guard fires
// with no per-executor configuration.
func TestCommandExecutor_RecursionGuardDefaultCap(t *testing.T) {
	t.Setenv(agentDepthEnv, "2")
	ex := CommandExecutor{Build: func(asset.Phase, string) []string { return []string{"echo", "x"} }}
	err := ex.Execute(asset.Phase{Name: "x"}, "m")
	execErr := requireExecError(t, err)
	if execErr.Kind != KindRecursionLimit {
		t.Errorf("default cap: want KindRecursionLimit at depth 2, got %v", execErr.Kind)
	}
}

// A runaway agent's output is bounded: the executor retains at most MaxOutputBytes
// and reports truncation, instead of OOMing on an unbounded CombinedOutput. seq
// emits ~600 KB deterministically; a 1 KiB cap must clip it.
func TestCommandExecutor_OutputCapTruncatesRunaway(t *testing.T) {
	rec := &recorder{}
	ex := CommandExecutor{
		Build:          func(asset.Phase, string) []string { return []string{"seq", "1", "100000"} },
		MaxOutputBytes: 1024,
		Log:            rec.log,
	}
	if err := ex.Execute(asset.Phase{Name: "x"}, "m"); err != nil {
		t.Fatalf("seq exits 0; Execute should succeed: %v", err)
	}
	last := rec.logs[len(rec.logs)-1]
	// Retained log is bounded near the 1 KiB cap, NOT the ~600 KB seq produced —
	// proving no OOM-sized buffer was held.
	if len(last) > 4096 {
		t.Errorf("retained output must be bounded near the cap; got %d bytes", len(last))
	}
	if !strings.Contains(last, "truncated") {
		t.Error("a clipped run must be reported as truncated (honest), not as full output")
	}
}

// Output under the cap is retained verbatim, with no truncation note — the common
// case (a phase log far under 10 MiB) is byte-for-byte unchanged.
func TestCommandExecutor_OutputUnderCapVerbatim(t *testing.T) {
	rec := &recorder{}
	ex := CommandExecutor{
		Build:          func(asset.Phase, string) []string { return []string{"echo", "small-output"} },
		MaxOutputBytes: 1024,
		Log:            rec.log,
	}
	if err := ex.Execute(asset.Phase{Name: "x"}, "m"); err != nil {
		t.Fatalf("Execute: %v", err)
	}
	last := rec.logs[len(rec.logs)-1]
	if !strings.Contains(last, "small-output") {
		t.Errorf("under-cap output must be retained verbatim; got %q", last)
	}
	if strings.Contains(last, "truncated") {
		t.Error("output under the cap must NOT be reported as truncated")
	}
}

// Execute runs the agent in CommandExecutor.Dir (the project --root), so a real
// agent writes/reads relative to the project, not forge's own cwd. pwd echoes its
// working directory; the captured output must be the configured Dir.
func TestCommandExecutor_RunsInDir(t *testing.T) {
	dir := t.TempDir()
	rec := &recorder{}
	ex := CommandExecutor{
		Build: func(asset.Phase, string) []string { return []string{"pwd"} },
		Dir:   dir,
		Log:   rec.log,
	}
	if err := ex.Execute(asset.Phase{Name: "p"}, "m"); err != nil {
		t.Fatalf("Execute: %v", err)
	}
	if last := rec.logs[len(rec.logs)-1]; !strings.Contains(last, dir) {
		t.Errorf("agent must run in Dir %q; got: %s", dir, last)
	}
}

// Observe, when set, receives the finished command's phase name and RAW captured
// output — the generic output sink the caller (e.g. the CLI) parses for an
// executor-specific structure like claude's cost JSON. The executor itself does NOT
// interpret the bytes; here echo's output is handed back verbatim under the phase name.
func TestCommandExecutor_ObserveReceivesPhaseAndOutput(t *testing.T) {
	var gotPhase, gotOutput string
	called := 0
	ex := CommandExecutor{
		Build:   func(p asset.Phase, mode string) []string { return []string{"echo", "hello-sink"} },
		Observe: func(phase, output string) { called++; gotPhase, gotOutput = phase, output },
	}
	if err := ex.Execute(asset.Phase{Name: "implementer"}, "balanced"); err != nil {
		t.Fatalf("Execute: %v", err)
	}
	if called != 1 {
		t.Errorf("Observe must be called exactly once, got %d", called)
	}
	if gotPhase != "implementer" {
		t.Errorf("Observe phase = %q, want implementer", gotPhase)
	}
	if !strings.Contains(gotOutput, "hello-sink") {
		t.Errorf("Observe must receive the raw command output; got %q", gotOutput)
	}
}

// RenderLog, when set, transforms the captured output before it is logged — letting
// the caller present a tidy view (e.g. unwrap claude JSON) WITHOUT this generic layer
// knowing that format. The raw output still flows to Observe unchanged; only the LOG
// line is rendered. Here the renderer replaces the output wholesale to prove logf used it.
func TestCommandExecutor_RenderLogCustomizesLogLine(t *testing.T) {
	rec := &recorder{}
	var observed string
	ex := CommandExecutor{
		Build:     func(p asset.Phase, mode string) []string { return []string{"echo", "RAW-PAYLOAD"} },
		Log:       rec.log,
		Observe:   func(_, output string) { observed = output },
		RenderLog: func(output string) string { return "RENDERED(" + output + ")" },
	}
	if err := ex.Execute(asset.Phase{Name: "p"}, "m"); err != nil {
		t.Fatalf("Execute: %v", err)
	}
	if !containsLine(rec.logs, "RENDERED(") {
		t.Errorf("the log line must use RenderLog's output; logs=%v", rec.logs)
	}
	// Observe still sees the RAW bytes — the renderer only affects the log view.
	if !strings.Contains(observed, "RAW-PAYLOAD") || strings.Contains(observed, "RENDERED") {
		t.Errorf("Observe must receive raw (unrendered) output; got %q", observed)
	}
}

// Honesty / backward-compat: with NEITHER Observe NOR RenderLog set (the default and
// every pre-existing caller), Execute logs the RAW command output verbatim, exactly as
// before these fields existed — a non-JSON echo output is logged unchanged and no sink
// fires. This is the byte-for-byte guarantee the dry/echo paths depend on.
func TestCommandExecutor_NoHooksIsByteForByteDefault(t *testing.T) {
	rec := &recorder{}
	ex := CommandExecutor{
		Build: func(p asset.Phase, mode string) []string { return []string{"echo", "plain output"} },
		Log:   rec.log,
	}
	if err := ex.Execute(asset.Phase{Name: "p"}, "m"); err != nil {
		t.Fatalf("Execute: %v", err)
	}
	last := rec.logs[len(rec.logs)-1]
	if !strings.Contains(last, "plain output") || strings.Contains(last, "RENDERED") {
		t.Errorf("default path must log raw output verbatim; got %q", last)
	}
}

// requireExecError asserts err is a non-nil *ExecError and returns it, so each
// test can then check Kind/Retryable. Fails the test (fatally) otherwise.
func requireExecError(t *testing.T, err error) *ExecError {
	t.Helper()
	if err == nil {
		t.Fatal("want a non-nil error (fail closed), got nil")
	}
	var execErr *ExecError
	if !errors.As(err, &execErr) {
		t.Fatalf("want a *ExecError, got %T: %v", err, err)
	}
	return execErr
}
