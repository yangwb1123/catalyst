package docker

import (
	"bytes"
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
}

// Run executes argv inside a fresh container. A nil error with a non-zero
// code means the command ran and failed; infrastructure failures (config,
// timeout) return an error for the executor to classify.
func (r *Runner) Run(
	ctx context.Context,
	argv []string,
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
	args := []string{"run", "--rm", "--network", "none"}
	if r.MemoryMB > 0 {
		args = append(args, "--memory", fmt.Sprintf("%dm", r.MemoryMB))
	}
	args = append(args, r.Image, "/bin/sh", "-c", shellJoin(argv))
	cmd := exec.CommandContext(runCtx, binary, args...)
	var out bytes.Buffer
	cmd.Stdout = &out
	cmd.Stderr = &out
	if r.Logf != nil {
		r.Logf("docker: running %q in %s", strings.Join(argv, " "), r.Image)
	}
	started := time.Now()
	err := cmd.Run()
	if r.Logf != nil {
		r.Logf("docker: container exited after %s", time.Since(started).Round(time.Millisecond))
	}
	if err != nil {
		if runCtx.Err() != nil {
			return out.String(), 0, fmt.Errorf("docker container timed out after %s", timeout)
		}
		if exit, ok := err.(*exec.ExitError); ok {
			return out.String(), exit.ExitCode(), nil
		}
		return out.String(), 0, configFault(fmt.Errorf("docker run: %w", err))
	}
	return out.String(), 0, nil
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
