package orchestrator

import (
	"context"
	"errors"
	"fmt"
	"os/exec"
	"strings"
	"time"

	"forgeos/forge-core/internal/orchestrator/docker"
	"forgeos/forge-core/internal/orchestrator/firecracker"
	"forgeos/forge-core/internal/orchestrator/sandbox"
)

// SandboxConfig describes how to isolate an agent command. It is the v3
// extension point; with a Runner wired (or auto-wired from Type + Image) the
// command executes inside the isolated runtime, otherwise every non-"none"
// Type fails closed rather than accidentally executing on the host.
type SandboxConfig struct {
	Type       string // "" (none) | "firecracker" | "docker"
	Image      string // docker image; firecracker rootdir template
	Kernel     string // firecracker vmlinux.bin path (Type "firecracker")
	MemoryMB   int    // RAM limit; 0 = use default
	TimeoutSec int    // session timeout; 0 = use command's Timeout
	// Runner is the wired isolation implementation for Type. nil keeps the
	// fail-closed contract: a declared sandbox without a runner is a
	// permanent config error and never falls back to host execution. When
	// nil, Execute auto-wires a runner from Type/Image/Kernel.
	Runner sandbox.Runner
}

func (c CommandExecutor) sandboxNone() bool {
	if c.Sandbox == nil {
		return true
	}
	runtime := strings.TrimSpace(c.Sandbox.Type)
	return runtime == "" || strings.EqualFold(runtime, "none")
}

// sandboxConfigError enforces the isolation boundary. A declared sandbox is a
// safety requirement, not a hint: until a runtime is wired, falling back to the
// host would violate the workflow contract and must be a permanent config error.
func (c CommandExecutor) sandboxConfigError(phase string) error {
	if c.Sandbox == nil {
		return nil
	}
	runtime := strings.TrimSpace(c.Sandbox.Type)
	if runtime == "" || strings.EqualFold(runtime, "none") {
		return nil
	}
	runner, err := c.sandboxRunner()
	if err != nil {
		return configErr(phase, err)
	}
	c.Sandbox.Runner = runner
	return nil
}

// sandboxRunner returns the wired runner for the declared Type, auto-wiring
// from configuration when no runner was injected. Unknown Types and
// incomplete firecracker configuration fail closed with a permanent config
// fault.
func (c CommandExecutor) sandboxRunner() (sandbox.Runner, error) {
	if c.Sandbox.Runner != nil {
		return c.Sandbox.Runner, nil
	}
	runtime := strings.TrimSpace(c.Sandbox.Type)
	switch strings.ToLower(runtime) {
	case "docker":
		if strings.TrimSpace(c.Sandbox.Image) == "" {
			return nil, fmt.Errorf("sandbox %q requested but no image is configured; refusing host execution", runtime)
		}
		return &docker.Runner{Image: c.Sandbox.Image, MemoryMB: c.Sandbox.MemoryMB}, nil
	case "firecracker":
		if strings.TrimSpace(c.Sandbox.Kernel) == "" || strings.TrimSpace(c.Sandbox.Image) == "" {
			return nil, fmt.Errorf("sandbox %q requested but kernel/rootdir are not configured; refusing host execution", runtime)
		}
		return &firecracker.FirecrackerRunner{Kernel: c.Sandbox.Kernel, RootDir: c.Sandbox.Image}, nil
	default:
		return nil, fmt.Errorf("sandbox %q requested but no sandbox runner is installed; refusing host execution", runtime)
	}
}

// executeSandboxedDispatch builds the sandboxed run: a dedicated context
// with the configured timeout, the stripped prompt delivered as guest
// stdin (F1: a claude-family agent with no prompt is a dead phase).
func (c CommandExecutor) executeSandboxedDispatch(
	ctx context.Context,
	phase string,
	argv []string,
	input string,
	useStdin bool,
) error {
	runCtx, runCancel := c.commandContext(ctx)
	defer runCancel()
	timeout := time.Duration(c.Sandbox.TimeoutSec) * time.Second
	prompt := ""
	if useStdin {
		prompt = input
	}
	return c.executeSandboxed(runCtx, phase, argv, prompt, timeout)
}

// executeSandboxed runs the phase through the wired sandbox runner instead of
// the host shell, then funnels the outcome through the same finish pipeline so
// observation, output contracts, and error classification stay uniform. A
// clean non-zero guest exit becomes a KindFailed run; infrastructure faults
// (config, timeout) map onto their own kinds without fabricating an agent
// verdict.
func (c CommandExecutor) executeSandboxed(
	runCtx context.Context,
	phase string,
	argv []string,
	stdin string,
	timeout time.Duration,
) error {
	start := c.now()
	output, code, err := c.Sandbox.Runner.Run(runCtx, argv, stdin, timeout)
	latency := c.now().Sub(start)
	out := &cappedBuffer{cap: c.maxOutputBytes()}
	_, _ = out.Write([]byte(output))
	if err != nil {
		// Typed classification parity with the host path: only a deadline
		// is retryable; a cancellation is the caller's verdict, not a
		// timeout (review F5). No message-string matching.
		if errors.Is(runCtx.Err(), context.DeadlineExceeded) {
			return &ExecError{Phase: phase, Kind: KindTimeout, Err: err}
		}
		if runCtx.Err() != nil {
			return &ExecError{Phase: phase, Kind: KindFailed, Err: err}
		}
		return &ExecError{Phase: phase, Kind: KindConfig, Err: err}
	}
	if code != 0 {
		err = &exec.ExitError{}
	}
	return c.finish(phase, argv, out, err, runCtx.Err(), latency)
}
