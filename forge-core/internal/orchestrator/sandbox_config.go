package orchestrator

import (
	"context"
	"errors"
	"fmt"
	"os/exec"
	"strings"
	"time"

	"forgeos/forge-core/internal/execbound"
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

// withSandboxRunner enforces the isolation boundary and installs an auto-wired
// runner on a receiver-local SandboxConfig copy. RunParallel shares one
// CommandExecutor across goroutines, so mutating the caller's config here would
// race concurrent executions and could tear the Runner interface value.
func (c CommandExecutor) withSandboxRunner(phase string) (CommandExecutor, error) {
	if c.Sandbox == nil {
		return c, nil
	}
	runtime := strings.TrimSpace(c.Sandbox.Type)
	if runtime == "" || strings.EqualFold(runtime, "none") {
		return c, nil
	}
	runner, err := c.sandboxRunner()
	if err != nil {
		return c, configErr(phase, err)
	}
	local := *c.Sandbox
	local.Runner = runner
	c.Sandbox = &local
	return c, nil
}

// sandboxRunner returns the wired runner for the declared Type, auto-wiring
// from configuration when no runner was injected. Unknown Types and
// incomplete firecracker configuration fail closed with a permanent config
// fault.
func (c CommandExecutor) sandboxRunner() (sandbox.Runner, error) {
	memoryMB, err := sandbox.EffectiveMemoryMB(c.Sandbox.MemoryMB)
	if err != nil {
		return nil, err
	}
	if c.Sandbox.Runner != nil {
		return c.Sandbox.Runner, nil
	}
	runtime := strings.TrimSpace(c.Sandbox.Type)
	switch strings.ToLower(runtime) {
	case "docker":
		if strings.TrimSpace(c.Sandbox.Image) == "" {
			return nil, fmt.Errorf("sandbox %q requested but no image is configured; refusing host execution", runtime)
		}
		return &docker.Runner{
			Image: c.Sandbox.Image, MemoryMB: memoryMB, MaxOutputBytes: c.maxOutputBytes(),
		}, nil
	case "firecracker":
		if strings.TrimSpace(c.Sandbox.Kernel) == "" || strings.TrimSpace(c.Sandbox.Image) == "" {
			return nil, fmt.Errorf("sandbox %q requested but kernel/rootdir are not configured; refusing host execution", runtime)
		}
		return &firecracker.FirecrackerRunner{
			Kernel: c.Sandbox.Kernel, RootDir: c.Sandbox.Image,
			MemoryMB: memoryMB, MaxOutputBytes: c.maxOutputBytes(),
		}, nil
	default:
		return nil, fmt.Errorf("sandbox %q requested but no sandbox runner is installed; refusing host execution", runtime)
	}
}

// commandDeadlineCtx derives a run context with the executor's documented
// Timeout semantics (zero = no deadline, the back-compat default) — the
// sandbox path's analogue of execbound's internal deadline derivation (the
// sandbox runner needs a ctx + timeout PAIR, so it cannot ride execbound.Run
// directly). Positive Timeout → a deadline; zero/negative → merely cancelable.
func (c CommandExecutor) commandDeadlineCtx(parent context.Context) (context.Context, context.CancelFunc) {
	if c.Timeout > 0 {
		return context.WithTimeout(parent, c.Timeout)
	}
	return context.WithCancel(parent)
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
	runCtx, runCancel := c.commandDeadlineCtx(ctx)
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
// verdict. The guest output rides execbound.FromBytes (the shared capped-
// retention + honest truncation marker), with the run ctx error preserved so
// finish's classification is byte-identical to the host path.
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
	res := execbound.FromBytes([]byte(output), c.MaxOutputBytes)
	res.CtxErr = runCtx.Err()
	if err != nil {
		var outputLimit *sandbox.OutputLimitError
		if errors.As(err, &outputLimit) {
			return &ExecError{Phase: phase, Kind: KindFailed, Err: err}
		}
		// Typed classification parity with the host path: only a deadline
		// is retryable; a cancellation is the caller's verdict, not a
		// timeout (review F5). No message-string matching.
		if errors.Is(err, context.DeadlineExceeded) ||
			errors.Is(runCtx.Err(), context.DeadlineExceeded) {
			return &ExecError{Phase: phase, Kind: KindTimeout, Err: err}
		}
		if errors.Is(err, context.Canceled) || runCtx.Err() != nil {
			return &ExecError{Phase: phase, Kind: KindFailed, Err: err}
		}
		return &ExecError{Phase: phase, Kind: KindConfig, Err: err}
	}
	if code != 0 {
		res.Err = &exec.ExitError{}
	}
	return c.finish(phase, argv, res, latency)
}
