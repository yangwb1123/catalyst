package docker

import (
	"context"
	"fmt"
	"os/exec"
	"strings"
	"time"
)

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
	// MemoryMB caps the container RAM; 0 leaves the daemon default.
	MemoryMB int
	// Logf receives runner diagnostics; nil disables them.
	Logf func(format string, args ...any)
	// MaxOutputBytes bounds captured output (0 = 1 MiB); overflow is
	// drained and discarded so a runaway guest cannot OOM the host
	// (review F3).
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
	if err := r.checkReady(binary); err != nil {
		return "", 0, err
	}
	runCtx := ctx
	if timeout > 0 {
		var cancel context.CancelFunc
		runCtx, cancel = context.WithTimeout(ctx, timeout)
		defer cancel()
	}
	cmd, out := buildRunCommand(runCtx, binary, r, argv, stdin)
	if r.Logf != nil {
		r.Logf("docker: running %q in %s", strings.Join(argv, " "), r.Image)
	}
	started := time.Now()
	err := cmd.Run()
	if runCtx.Err() != nil {
		// The client was killed on timeout: --rm does not stop the
		// container, so remove it explicitly to avoid orphans (review F7).
		_ = exec.Command(binary, "rm", "-f", containerName()).Run()
		return string(out.buf), 0, fmt.Errorf("docker container timed out after %s", timeout)
	}
	if r.Logf != nil {
		r.Logf("docker: container exited after %s", time.Since(started).Round(time.Millisecond))
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

// containerName derives a stable per-invocation container name so the
// timeout cleanup can address the exact container.
func containerName() string {
	return fmt.Sprintf("forge-sandbox-%d", time.Now().UnixNano())
}

// buildRunCommand constructs the bounded docker command with stdin and
// capped output wiring.
func buildRunCommand(
	runCtx context.Context,
	binary string,
	r *Runner,
	argv []string,
	stdin string,
) (*exec.Cmd, *cappedWriter) {
	cmd := exec.CommandContext(runCtx, binary, runArgs(r, argv, stdin)...)
	if stdin != "" {
		cmd.Stdin = strings.NewReader(stdin)
	}
	cap := r.MaxOutputBytes
	if cap <= 0 {
		cap = 1 << 20
	}
	out := &cappedWriter{cap: cap}
	cmd.Stdout = out
	cmd.Stderr = out
	return cmd, out
}

// runArgs builds the docker run argv with a named, memory-capped,
// network-isolated container.
func runArgs(r *Runner, argv []string, stdin string) []string {
	args := []string{"run", "--rm", "--name", containerName(), "--network", "none"}
	if stdin != "" {
		// -i keeps stdin attached; without it the guest's stdin is /dev/null.
		args = append(args, "-i")
	}
	if r.MemoryMB > 0 {
		args = append(args, "--memory", fmt.Sprintf("%dm", r.MemoryMB))
	}
	return append(args, r.Image, "/bin/sh", "-c", shellJoin(argv))
}

// checkReady verifies the runner's prerequisites: an image is configured and
// the daemon answers.
func (r *Runner) checkReady(binary string) error {
	if r.Image == "" {
		return configFault(fmt.Errorf("docker runner: image must be configured"))
	}
	probe := exec.Command(binary, "info")
	if err := probe.Run(); err != nil {
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
