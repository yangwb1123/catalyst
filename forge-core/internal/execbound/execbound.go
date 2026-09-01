// Package execbound provides ONE bounded subprocess run: a context deadline
// (or an explicit unbounded escape), cancellation-only process-group teardown
// on Linux (with a portable parent-reader backstop), and capped output retention with an honest
// truncation marker. It is the leaf extraction of the orchestrator's solved
// bounded-run pattern (CommandExecutor.Timeout + cappedBuffer +
// process-group cancel), shared by the orchestrator and the gate bridge.
//
// Zero dependencies beyond the Go standard library (the forge-core red line);
// it must never import an internal package, in particular not internal/asset.
package execbound

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"os"
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

const maxStdinBytes = 16 << 20

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
	// Log receives a bounded warning when cancellation lacks process-group
	// teardown or WaitDelay proves an incomplete output drain. Nil-safe and
	// silent for a clean, completely drained run.
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
	// CaptureCombined sends both child streams through one shared raw pipe,
	// retaining their pipe order in one bounded buffer and proving its EOF.
	CaptureCombined Capture = iota
	// CaptureSplit retains stdout and stderr separately — gate.ProbeAll needs
	// raw stdout for JSON plus stderr for error text.
	CaptureSplit
)

// Spec is the per-run command configuration beyond argv. Zero value = os/exec
// defaults (inherit cwd, inherit env, no stdin).
type Spec struct {
	Dir        string     // working directory; "" = inherit forge's cwd
	Env        []string   // child environment; nil = inherit parent (os/exec default)
	Stdin      []byte     // bounded in-memory stdin; nil = os/exec default
	ExtraFiles []*os.File // inherited descriptors beginning at fd 3; nil = none
	// ExecutablePath, when non-empty, is the already-resolved executable used
	// for this run while argv[0] remains the caller-declared process name. It
	// is primarily useful to producers that resolve and snapshot a tool before
	// execution. Clearing exec.Cmd.Err is required because exec.Command
	// may already have recorded a failed LookPath for argv[0]. Empty preserves
	// the exact legacy lookup behavior.
	ExecutablePath string
}

// Run executes argv with the bounded-run mechanics under ctx and opts:
//   - deadline: Timeout > 0 → context.WithTimeout(ctx, Timeout); Unbounded →
//     ctx as-is (NO deadline, but parent cancellation still propagates);
//     Timeout == 0 → DefaultTimeout.
//   - teardown: supported Linux → Setpgid + lifecycle-serialized SIGKILL(-pgid);
//     all other cases → direct-child kill. Every target also gets WaitDelay =
//     2s. Independent raw-pipe drains return within the same bound and expose
//     DrainIncomplete when a descendant keeps a writer; descendants may
//     survive on every platform.
//   - capture: capped retention with the honest truncation marker.
//
// A process group is a best-effort teardown handle, not containment: a child
// may deliberately escape it. DrainIncomplete reports when WaitDelay had to
// close output pipes. Kill errors (ESRCH etc.) are never a silent pass.
func Run(ctx context.Context, argv []string, opts Options, capture Capture, spec Spec) Result {
	if err := opts.Validate(); err != nil {
		return Result{Err: err}
	}
	if err := validateSpec(spec); err != nil {
		return Result{Err: err}
	}
	if len(argv) == 0 {
		return Result{Err: fmt.Errorf("empty argv")}
	}
	runCtx, runCancel := deadlineContext(ctx, opts)
	defer runCancel()
	cmd := newBoundedCommand(runCtx, argv, spec)
	cancelled := &cancelTracker{}
	wrapTrackedCancel(cmd, runCtx, cancelled)
	return runCapturedCommand(cmd, runCtx, cancelled, opts, capture)
}

func capturedResult(runErr, ctxErr error, cancelled cancelSnapshot) Result {
	return Result{
		Err: preferContextFailure(runErr, ctxErr), CtxErr: ctxErr,
		DrainIncomplete: errors.Is(runErr, exec.ErrWaitDelay),
		cancelApplied:   cancelled.called && cancelled.err == nil,
	}
}

func newBoundedCommand(ctx context.Context, argv []string, spec Spec) *boundedCommand {
	cmd := exec.Command(argv[0], argv[1:]...)
	if spec.ExecutablePath != "" {
		cmd.Path = spec.ExecutablePath
		cmd.Err = nil
	}
	lifecycle := setupProcessGroup(cmd)
	command := &boundedCommand{
		Cmd: cmd, runCtx: ctx, lifecycle: lifecycle, cancel: lifecycle.cancel,
		processDone: make(chan struct{}), cancelDone: make(chan struct{}),
	}
	cmd.Dir, cmd.Env = spec.Dir, spec.Env
	if spec.Stdin != nil {
		cmd.Stdin = bytes.NewReader(append([]byte(nil), spec.Stdin...))
	}
	cmd.ExtraFiles = append([]*os.File(nil), spec.ExtraFiles...)
	return command
}

func validateSpec(spec Spec) error {
	return ValidateStdinSize(len(spec.Stdin))
}

// ValidateStdinSize enforces the shared in-memory child-input ceiling before
// callers allocate, dispatch, or hand input to a host or sandbox runner.
func ValidateStdinSize(size int) error {
	if size < 0 || size > maxStdinBytes {
		return fmt.Errorf("stdin exceeds %d-byte bound", maxStdinBytes)
	}
	return nil
}

// logDegradation reports observed incomplete drain independently of platform,
// then the weaker direct-child cancellation teardown when applicable.
func (r Result) logDegradation(opts Options) {
	if opts.Log == nil {
		return
	}
	if r.DrainIncomplete {
		opts.Log(fmt.Sprintf("subprocess output drain incomplete on %s: descendants may have outlived the command", runtime.GOOS))
		return
	}
	if r.cancelApplied && !groupKillSupported() {
		opts.Log(fmt.Sprintf("process-group teardown unavailable on %s: cancelled command's descendants may outlive it", runtime.GOOS))
	}
}

func preferContextFailure(runErr, ctxErr error) error {
	if runErr == nil && ctxErr != nil {
		return ctxErr
	}
	return runErr
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
	Stdout          []byte // retained stdout bytes (CaptureSplit only)
	Stderr          []byte // retained stderr bytes (CaptureSplit only)
	Merged          []byte // retained merged bytes (CaptureCombined only)
	Err             error  // process/context failure; nil only for clean exit and drain
	CtxErr          error  // run ctx error at completion: DeadlineExceeded | Canceled | nil
	Total           int64  // observed bytes, saturated at MaxInt64 on count overflow
	Retained        int    // bytes retained in memory across captured stream(s)
	CountOverflow   bool   // exact total no longer fits signed int64
	DrainIncomplete bool   // WaitDelay proved the output drain incomplete
	cancelApplied   bool   // cancellation successfully reached the process/group
	renderTotal     int64  // bytes in the stream returned by retainedBytes
	renderOverflow  bool   // renderTotal no longer fits signed int64
	renderCountSet  bool   // split capture supplied an independent stdout count
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

func (r Result) renderedCount() (int64, bool) {
	if r.renderCountSet {
		return r.renderTotal, r.renderOverflow
	}
	return r.Total, r.CountOverflow
}

// Rendered returns the retained output trimmed, appending the truncation
// marker when Total > Retained — a clipped log is never mistaken for full
// output.
func (r Result) Rendered() string {
	b := r.retainedBytes()
	s := strings.TrimSpace(string(b))
	total, overflow := r.renderedCount()
	if overflow {
		s += countOverflowMarker(len(b))
	} else if total > int64(len(b)) {
		s += truncationMarker(len(b), total)
	}
	return s
}

// Observed returns the retained output UN-trimmed, with the same marker —
// machine parsers keep every byte.
func (r Result) Observed() string {
	b := r.retainedBytes()
	s := string(b)
	total, overflow := r.renderedCount()
	if overflow {
		s += countOverflowMarker(len(b))
	} else if total > int64(len(b)) {
		s += truncationMarker(len(b), total)
	}
	return s
}

// truncationMarker is the golden truncation note: byte-exact, shared by the
// orchestrator's rendered/observed semantics and the gate bridge's output.
func truncationMarker(retained int, total int64) string {
	return fmt.Sprintf(" …[output truncated: retained %d of %d bytes (--max-output-bytes)]", retained, total)
}

func countOverflowMarker(retained int) string {
	return fmt.Sprintf(" …[output truncated: retained %d bytes; total byte count overflowed (--max-output-bytes)]", retained)
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

// groupKillSupported reports whether race-free process-group teardown exists
// on this host. Linux requires waitid(WNOWAIT); other targets use safe direct-
// child termination plus the portable drain backstop.
func groupKillSupported() bool {
	return platformGroupKillSupported()
}
