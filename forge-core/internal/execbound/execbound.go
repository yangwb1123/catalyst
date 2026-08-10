// Package execbound provides ONE bounded subprocess run: a context deadline
// (or an explicit unbounded escape), process-group teardown on unix (with a
// portable WaitDelay backstop), and capped output retention with an honest
// truncation marker. It is the leaf extraction of the orchestrator's solved
// bounded-run pattern (CommandExecutor.Timeout + cappedBuffer +
// process-group cancel), shared by the orchestrator and the gate bridge so the
// grandchild-pipe logic can never drift between the two consumers.
//
// Zero dependencies beyond the Go standard library (the forge-core red line);
// it must never import an internal package, in particular not internal/asset.
package execbound

import (
	"context"
	"fmt"
	"io"
	"os/exec"
	"runtime"
	"strings"
	"time"
)

// DefaultTimeout is the safe default deadline when Options.Timeout is zero:
// 10 minutes. A gate spawn past this now FAILS instead of hanging-then-
// succeeding — the one intentional break, with explicit escapes:
// Options.Unbounded, or CLI/env "0" (see gate.ResolveOptions).
const DefaultTimeout = 10 * time.Minute

// DefaultMaxOutputBytes is the safe default retention cap when
// Options.MaxOutputBytes is zero: 10 MiB (10 << 20).
const DefaultMaxOutputBytes = 10 << 20

// Options controls one bounded subprocess run. The zero value selects the safe
// defaults (DefaultTimeout / DefaultMaxOutputBytes). Validate must pass before
// any fork.
type Options struct {
	// Timeout > 0: explicit deadline. 0: safe default. < 0: config error.
	Timeout time.Duration
	// MaxOutputBytes > 0: explicit cap. 0: safe default. < 0: config error.
	MaxOutputBytes int
	// Unbounded: EXPLICIT no-deadline escape. Conflicts with Timeout > 0.
	// Never derivable by arithmetic or sign — a named, greppable bool (the
	// orchestrator's documented zero = no-deadline maps to this; see
	// CommandExecutor.execboundOptions).
	Unbounded bool
	// Knob is the human-facing config source ("--timeout" |
	// "FORGE_GATE_TIMEOUT" | ""), set by gate.ResolveOptions, consumed by the
	// gate layer's honest timeout text. "" = omit the knob clause.
	Knob string
	// Log, when set, receives one line per kill event on a platform without
	// process-group teardown: "process-group teardown unavailable on <GOOS>:
	// timed-out command's descendants may outlive it". Nil-safe; never called
	// on the happy path.
	Log func(string)
}

// Validate fails fast on: Timeout < 0, MaxOutputBytes < 0, Unbounded &&
// Timeout > 0 (ambiguous). Runs before any fork (gate entry points AND inside
// Run — defense in depth, zero cost). The zero value passes.
func (o Options) Validate() error {
	if o.Timeout < 0 {
		return fmt.Errorf("timeout must be >= 0 (got %s)", o.Timeout)
	}
	if o.MaxOutputBytes < 0 {
		return fmt.Errorf("max output bytes must be >= 0 (got %d)", o.MaxOutputBytes)
	}
	if o.Unbounded && o.Timeout > 0 {
		return fmt.Errorf("unbounded and a positive timeout are mutually exclusive")
	}
	return nil
}

// Capture selects how the command's streams are retained.
type Capture int

const (
	// CaptureCombined retains both streams in ONE merged buffer (os/exec
	// same-pointer serialization) — the orchestrator's current behavior.
	CaptureCombined Capture = iota
	// CaptureSplit retains stdout and stderr separately — gate.ProbeAll needs
	// raw stdout for JSON plus stderr for error text.
	CaptureSplit
)

// Spec is the per-run command configuration beyond argv. Zero value = os/exec
// defaults (inherit cwd, inherit env, no stdin).
type Spec struct {
	Dir   string    // working directory; "" = inherit forge's cwd
	Env   []string  // child environment; nil = inherit parent (os/exec default)
	Stdin io.Reader // child stdin; nil = os/exec default
	// ExecutablePath, when non-empty, is the already-resolved executable used
	// for this run while argv[0] remains the caller-declared process name. It
	// is primarily useful to producers that resolve and snapshot a tool before
	// execution. Clearing exec.Cmd.Err is required because exec.CommandContext
	// may already have recorded a failed LookPath for argv[0]. Empty preserves
	// the exact legacy lookup behavior.
	ExecutablePath string
}

// Run executes argv with the bounded-run mechanics under ctx and opts:
//   - deadline: Timeout > 0 → context.WithTimeout(ctx, Timeout); Unbounded →
//     ctx as-is (NO deadline, but parent cancellation still propagates);
//     Timeout == 0 → DefaultTimeout.
//   - teardown: unix → Setpgid + Cancel = SIGKILL(-pgid) + WaitDelay = 2s;
//     non-unix → direct-child kill + WaitDelay = 2s (the portable backstop:
//     Run returns ≤ deadline + 2s even when descendants hold the pipes;
//     descendants leak on non-unix and one Log line is emitted).
//   - capture: capped retention with the honest truncation marker.
//
// Kill errors (ESRCH etc.) are best-effort — never fatal, never a pass.
func Run(ctx context.Context, argv []string, opts Options, capture Capture, spec Spec) Result {
	if err := opts.Validate(); err != nil {
		return Result{Err: err}
	}
	if len(argv) == 0 {
		return Result{Err: fmt.Errorf("empty argv")}
	}
	runCtx, runCancel := deadlineContext(ctx, opts)
	defer runCancel()
	cmd := exec.CommandContext(runCtx, argv[0], argv[1:]...)
	if spec.ExecutablePath != "" {
		cmd.Path = spec.ExecutablePath
		cmd.Err = nil
	}
	setupProcessGroup(cmd)
	if spec.Dir != "" {
		cmd.Dir = spec.Dir
	}
	if spec.Env != nil {
		cmd.Env = spec.Env
	}
	if spec.Stdin != nil {
		cmd.Stdin = spec.Stdin
	}
	if capture == CaptureSplit {
		stdout := &cappedBuffer{cap: maxBytes(opts.MaxOutputBytes)}
		stderr := &cappedBuffer{cap: maxBytes(opts.MaxOutputBytes)}
		cmd.Stdout, cmd.Stderr = stdout, stderr
		runErr := cmd.Run()
		res := Result{
			Stdout: stdout.buf, Stderr: stderr.buf, Err: runErr,
			CtxErr:   runCtx.Err(),
			Total:    int64(stdout.total) + int64(stderr.total),
			Retained: len(stdout.buf) + len(stderr.buf),
		}
		res.logDegradation(opts)
		return res
	}
	merged := &cappedBuffer{cap: maxBytes(opts.MaxOutputBytes)}
	cmd.Stdout, cmd.Stderr = merged, merged
	runErr := cmd.Run()
	res := Result{
		Merged: merged.buf, Err: runErr, CtxErr: runCtx.Err(),
		Total: int64(merged.total), Retained: len(merged.buf),
	}
	res.logDegradation(opts)
	return res
}

// logDegradation emits the honest non-unix kill note exactly when a kill event
// fired without process-group teardown available — never on the happy path.
func (r Result) logDegradation(opts Options) {
	if r.CtxErr == nil || GroupKillAvailable() || opts.Log == nil {
		return
	}
	opts.Log(fmt.Sprintf("process-group teardown unavailable on %s: timed-out command's descendants may outlive it", runtime.GOOS))
}

// deadlineContext derives the run context: the configured deadline, or a plain
// cancelable context when Unbounded (parent cancellation still propagates).
// Timeout == 0 selects the safe default (Validate has already rejected < 0).
func deadlineContext(parent context.Context, opts Options) (context.Context, context.CancelFunc) {
	if opts.Unbounded {
		return context.WithCancel(parent)
	}
	timeout := opts.Timeout
	if timeout <= 0 {
		timeout = DefaultTimeout
	}
	return context.WithTimeout(parent, timeout)
}

// maxBytes is the effective retention cap: the configured MaxOutputBytes, or
// the safe default when unset/non-positive.
func maxBytes(n int) int {
	if n > 0 {
		return n
	}
	return DefaultMaxOutputBytes
}

// Result is the outcome of one bounded subprocess run.
type Result struct {
	Stdout   []byte // retained stdout bytes (CaptureSplit only)
	Stderr   []byte // retained stderr bytes (CaptureSplit only)
	Merged   []byte // retained merged bytes (CaptureCombined only)
	Err      error  // cmd.Run() error; nil iff the command exited 0
	CtxErr   error  // run ctx error at completion: DeadlineExceeded | Canceled | nil
	Total    int64  // total bytes written to the captured stream(s)
	Retained int    // bytes retained (== cap EXACTLY whenever Total >= cap)
}

// TimedOut reports whether the run ended because the deadline fired. A spawn
// already past its own deadline when the parent ctx cancels reports timeout
// (the stronger verdict), never a silent success.
func (r Result) TimedOut() bool { return r.CtxErr == context.DeadlineExceeded }

// retainedBytes returns the retained stream the rendering reads: the merged
// buffer when present, else stdout (FromBytes sets Stdout; a split caller
// reads Stdout/Stderr directly).
func (r Result) retainedBytes() []byte {
	if r.Merged != nil {
		return r.Merged
	}
	return r.Stdout
}

// Rendered returns the retained output trimmed, appending the truncation
// marker when Total > Retained — a clipped log is never mistaken for full
// output.
func (r Result) Rendered() string {
	b := r.retainedBytes()
	s := strings.TrimSpace(string(b))
	if r.Total > int64(len(b)) {
		s += truncationMarker(len(b), r.Total)
	}
	return s
}

// Observed returns the retained output UN-trimmed, with the same marker —
// machine parsers keep every byte.
func (r Result) Observed() string {
	b := r.retainedBytes()
	s := string(b)
	if r.Total > int64(len(b)) {
		s += truncationMarker(len(b), r.Total)
	}
	return s
}

// truncationMarker is the golden truncation note: byte-exact, shared by the
// orchestrator's rendered/observed semantics and the gate bridge's output.
func truncationMarker(retained int, total int64) string {
	return fmt.Sprintf(" …[output truncated: retained %d of %d bytes (--max-output-bytes)]", retained, total)
}

// FromBytes builds a Result from a pre-captured byte string (sandboxed runs)
// with the same cap+marker semantics as a pipe capture. Sets Stdout; Merged
// stays nil (Observed/Rendered read Stdout then). Total reflects the full
// pre-captured size, so truncation is reported honestly.
func FromBytes(p []byte, max int) Result {
	cap := maxBytes(max)
	total := int64(len(p))
	retained := len(p)
	if retained > cap {
		retained = cap
	}
	return Result{Stdout: p[:retained], Total: total, Retained: retained}
}

// GroupKillAvailable reports whether process-group teardown exists on this
// platform: true on unix, false elsewhere. Consulted ONLY on the kill path.
func GroupKillAvailable() bool {
	return groupKillAvailable
}
