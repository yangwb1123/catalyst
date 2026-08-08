package docker

import (
	"context"
	"errors"
	"fmt"
	"os/exec"
	"strings"
	"time"

	"forgeos/forge-core/internal/orchestrator/sandbox"
)

const cleanupTimeout = 2 * time.Second

// Runner executes a command inside a Docker container with no network. It is
// the wired implementation behind SandboxConfig.Type "docker": each run is a
// fresh `docker run --rm` container, output is captured from stdout, and the
// exit code is the container's. Failures classify exactly like the
// Firecracker runner: an unavailable Docker daemon or unknown image is a
// permanent config fault, a deadline is a retryable timeout, and a clean
// non-zero exit is the command's own verdict.
type Runner struct {
	// Binary is the docker executable (default "docker").
	Binary string
	// Image is the container image, e.g. "alpine:latest". Empty means the
	// runner is not ready.
	Image string
	// MemoryMB caps the container RAM; 0 uses the shared safe default.
	MemoryMB int
	// Logf receives runner diagnostics; nil disables them.
	Logf func(format string, args ...any)
	// MaxOutputBytes bounds captured output (0 = the executor's 10 MiB
	// default). Overflow fails explicitly after the stream is drained.
	MaxOutputBytes int
}

// cappedWriter retains at most cap bytes and drains the rest.
type cappedWriter struct {
	cap   int
	buf   []byte
	total int
}

func (w *cappedWriter) Write(p []byte) (int, error) {
	w.total += len(p)
	if room := w.cap - len(w.buf); room > 0 {
		if len(p) <= room {
			w.buf = append(w.buf, p...)
		} else {
			w.buf = append(w.buf, p[:room]...)
		}
	}
	return len(p), nil
}

func (w *cappedWriter) limitError() error {
	if w.total <= w.cap {
		return nil
	}
	return &sandbox.OutputLimitError{Limit: w.cap, Total: w.total}
}

// Run executes argv inside a fresh container. A nil error with a non-zero
// code means the command ran and failed; infrastructure failures (config,
// timeout) return an error for the executor to classify.
func (r *Runner) Run(
	ctx context.Context,
	argv []string,
	stdin string,
	timeout time.Duration,
) (string, int, error) {
	binary := r.Binary
	if binary == "" {
		binary = "docker"
	}
	runCtx, cancel := boundedRunContext(ctx, timeout)
	defer cancel()
	effective, err := r.effective()
	if err != nil {
		return "", 0, err
	}
	if err := r.checkReady(runCtx, binary, runCommand); err != nil {
		return "", 0, readinessError(err, runCtx.Err(), timeout)
	}
	invocation := invocationArgs(effective, argv, stdin, containerName())
	cmd, out := buildRunCommand(runCtx, binary, effective, invocation.run, stdin)
	if r.Logf != nil {
		r.Logf("docker: running %q in %s", strings.Join(argv, " "), r.Image)
	}
	started := time.Now()
	err = cmd.Run()
	if runCtx.Err() != nil {
		return interruptedResult(binary, invocation.cleanup, out, runCtx.Err(), timeout)
	}
	if r.Logf != nil {
		r.Logf("docker: container exited after %s", time.Since(started).Round(time.Millisecond))
	}
	if limitErr := out.limitError(); limitErr != nil {
		return string(out.buf), 0, limitErr
	}
	if err != nil {
		if exit, ok := err.(*exec.ExitError); ok {
			// Exit 125 is the docker daemon's own fault (client/daemon/pull
			// error), not the guest's verdict — it must never surface as a
			// guest exit code (review stage-06 Medium).
			if exit.ExitCode() == 125 {
				return string(out.buf), 0, configFault(fmt.Errorf("docker daemon fault (exit 125): %w", err))
			}
			return string(out.buf), exit.ExitCode(), nil
		}
		return string(out.buf), 0, configFault(fmt.Errorf("docker run: %w", err))
	}
	return string(out.buf), 0, nil
}

func interruptedResult(
	binary string,
	cleanup []string,
	out *cappedWriter,
	cause error,
	timeout time.Duration,
) (string, int, error) {
	// --rm does not stop a container when the client is killed, so address the
	// exact generated name without allowing cleanup to wait forever.
	interrupted := interruptionError("docker container", cause, timeout)
	if err := cleanupContainer(binary, cleanup, runCommand); err != nil {
		return string(out.buf), 0, fmt.Errorf("%w; cleanup failed: %v", interrupted, err)
	}
	return string(out.buf), 0, interrupted
}

func boundedRunContext(
	ctx context.Context,
	timeout time.Duration,
) (context.Context, context.CancelFunc) {
	if timeout > 0 {
		return context.WithTimeout(ctx, timeout)
	}
	return context.WithCancel(ctx)
}

func readinessError(probeError, cause error, timeout time.Duration) error {
	if cause != nil {
		return interruptionError("docker readiness probe", cause, timeout)
	}
	return probeError
}

func interruptionError(subject string, cause error, timeout time.Duration) error {
	if errors.Is(cause, context.DeadlineExceeded) {
		if timeout > 0 {
			return fmt.Errorf("%s timed out after %s: %w", subject, timeout, cause)
		}
		return fmt.Errorf("%s deadline exceeded: %w", subject, cause)
	}
	return fmt.Errorf("%s cancelled: %w", subject, cause)
}

func (r *Runner) effective() (*Runner, error) {
	memoryMB, err := sandbox.EffectiveMemoryMB(r.MemoryMB)
	if err != nil {
		return nil, configFault(err)
	}
	effective := *r
	effective.MemoryMB = memoryMB
	return &effective, nil
}

type commandRun func(context.Context, string, ...string) error

func runCommand(ctx context.Context, binary string, args ...string) error {
	return exec.CommandContext(ctx, binary, args...).Run()
}

// cleanupContainer uses a short context independent from the expired run
// context. A wedged daemon therefore cannot turn timeout cleanup into a second
// unbounded wait.
func cleanupContainer(binary string, args []string, run commandRun) error {
	ctx, cancel := context.WithTimeout(context.Background(), cleanupTimeout)
	defer cancel()
	return run(ctx, binary, args...)
}

// containerName derives a stable per-invocation container name so the
// timeout cleanup can address the exact container.
func containerName() string {
	return fmt.Sprintf("forge-sandbox-%d", time.Now().UnixNano())
}

// containerInvocation binds run and cleanup argv to one generated name.
type containerInvocation struct {
	run     []string
	cleanup []string
}

func invocationArgs(r *Runner, argv []string, stdin, name string) containerInvocation {
	return containerInvocation{
		run:     runArgs(r, argv, stdin, name),
		cleanup: []string{"rm", "-f", name},
	}
}

// buildRunCommand constructs the bounded docker command with stdin and
// capped output wiring.
func buildRunCommand(
	runCtx context.Context,
	binary string,
	r *Runner,
	args []string,
	stdin string,
) (*exec.Cmd, *cappedWriter) {
	cmd := exec.CommandContext(runCtx, binary, args...)
	if stdin != "" {
		cmd.Stdin = strings.NewReader(stdin)
	}
	cap := r.MaxOutputBytes
	if cap <= 0 {
		cap = sandbox.DefaultMaxOutputBytes
	}
	out := &cappedWriter{cap: cap}
	cmd.Stdout = out
	cmd.Stderr = out
	return cmd, out
}

// runArgs builds the docker run argv with a named, memory-capped,
// network-isolated container.
func runArgs(r *Runner, argv []string, stdin, name string) []string {
	args := []string{"run", "--rm", "--name", name, "--network", "none"}
	if stdin != "" {
		// -i keeps stdin attached; without it the guest's stdin is /dev/null.
		args = append(args, "-i")
	}
	memoryMB, _ := sandbox.EffectiveMemoryMB(r.MemoryMB)
	args = append(args, "--memory", fmt.Sprintf("%dm", memoryMB))
	return append(args, r.Image, "/bin/sh", "-c", shellJoin(argv))
}

// checkReady verifies the runner's prerequisites: an image is configured and
// the daemon answers.
func (r *Runner) checkReady(ctx context.Context, binary string, run commandRun) error {
	if r.Image == "" {
		return configFault(fmt.Errorf("docker runner: image must be configured"))
	}
	if err := run(ctx, binary, "info"); err != nil {
		if ctx.Err() != nil {
			return ctx.Err()
		}
		return configFault(fmt.Errorf("docker runner: daemon unavailable: %w", err))
	}
	return nil
}

// shellJoin quotes argv so it survives the container's /bin/sh.
func shellJoin(argv []string) string {
	var quoted []string
	for _, arg := range argv {
		quoted = append(quoted, "'"+strings.ReplaceAll(arg, "'", `'"'"'`)+"'")
	}
	return strings.Join(quoted, " ")
}

// configFault wraps an infrastructure fault so the executor classifies it as
// a permanent configuration error.
func configFault(err error) error {
	return fmt.Errorf("sandbox docker: %w", err)
}
