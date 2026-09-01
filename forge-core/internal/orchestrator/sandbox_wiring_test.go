package orchestrator

import (
	"context"
	"errors"
	"fmt"
	"math"
	"os/exec"
	"strconv"
	"strings"
	"sync"
	"testing"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/orchestrator/docker"
	"forgeos/forge-core/internal/orchestrator/firecracker"
	"forgeos/forge-core/internal/orchestrator/sandbox"
)

// fakeRunner records the argv/timeout it received and returns scripted
// output, exit code, and error.
type fakeRunner struct {
	argv    []string
	stdin   string
	timeout time.Duration
	output  string
	code    int
	err     error
	calls   int
}

type cancelThenSucceedRunner struct {
	cancel context.CancelFunc
}

func (r cancelThenSucceedRunner) Run(
	_ context.Context, _ []string, _ string, _ time.Duration,
) (string, int, error) {
	r.cancel()
	return "apparently valid", 0, nil
}

func (f *fakeRunner) Run(
	_ context.Context,
	argv []string,
	stdin string,
	timeout time.Duration,
) (string, int, error) {
	f.calls++
	f.argv = argv
	f.stdin = stdin
	f.timeout = timeout
	return f.output, f.code, f.err
}

func sandboxedExecutor(runner sandbox.Runner) CommandExecutor {
	return CommandExecutor{
		Build: func(_ asset.Phase, _ string) []string {
			return []string{"agent", "-p", "prompt"}
		},
		Sandbox: &SandboxConfig{
			Type:       "firecracker",
			TimeoutSec: 7,
			Runner:     runner,
		},
	}
}

func TestSandboxRunnerReceivesArgvAndTimeout(t *testing.T) {
	runner := &fakeRunner{output: "guest output", code: 0}
	executor := sandboxedExecutor(runner)
	err := executor.Execute(context.Background(), asset.Phase{Name: "sandbox-phase"}, "run")
	if err != nil {
		t.Fatalf("Execute: %v", err)
	}
	if runner.calls != 1 {
		t.Fatalf("runner calls = %d, want 1", runner.calls)
	}
	if got := strings.Join(runner.argv, " "); got != "agent -p prompt" {
		t.Fatalf("runner argv = %q", got)
	}
	if runner.timeout != 7*time.Second {
		t.Fatalf("runner timeout = %v, want 7s", runner.timeout)
	}
}

func TestSandboxCleanNonZeroExitIsKindFailed(t *testing.T) {
	runner := &fakeRunner{output: "agent said no", code: 3}
	executor := sandboxedExecutor(runner)
	err := executor.Execute(context.Background(), asset.Phase{Name: "sandbox-phase"}, "run")
	var execErr *ExecError
	if !errors.As(err, &execErr) {
		t.Fatalf("expected ExecError, got %v", err)
	}
	if execErr.Kind != KindFailed {
		t.Fatalf("kind = %v, want KindFailed", execErr.Kind)
	}
	var exitError *sandbox.ExitError
	if !errors.As(err, &exitError) || exitError.ExitCode() != 3 ||
		!strings.Contains(err.Error(), "status 3") {
		t.Fatalf("sandbox exit status was not preserved: %v", err)
	}
}

func TestSandboxRejectsOversizedInputBeforeDispatch(t *testing.T) {
	runner := &fakeRunner{output: "must not run"}
	executor := sandboxedExecutor(runner)
	executor.PromptViaStdin = true
	executor.Build = func(asset.Phase, string) []string {
		return []string{"agent", "-p", strings.Repeat("x", 16<<20+1)}
	}
	dispatches := 0
	executor.OnDispatch = func(string) { dispatches++ }
	err := executor.Execute(context.Background(), asset.Phase{Name: "sandbox-phase"}, "run")
	var execErr *ExecError
	if !errors.As(err, &execErr) || execErr.Kind != KindConfig ||
		dispatches != 0 || runner.calls != 0 {
		t.Fatalf("oversized sandbox input result = %v, dispatches=%d calls=%d",
			err, dispatches, runner.calls)
	}
}

func TestSandboxTimeoutSecondsRejectNegativeAndOverflow(t *testing.T) {
	if _, err := sandboxTimeoutDuration(-1); err == nil {
		t.Fatal("negative sandbox timeout was accepted")
	}
	overflow := int64(math.MaxInt64/int64(time.Second)) + 1
	if strconv.IntSize == 64 {
		if _, err := sandboxTimeoutDuration(int(overflow)); err == nil {
			t.Fatal("overflowing sandbox timeout was accepted")
		}
	}
}

func TestSandboxInfraFaultIsKindConfig(t *testing.T) {
	runner := &fakeRunner{err: errors.New("sandbox firecracker: /dev/kvm unavailable")}
	executor := sandboxedExecutor(runner)
	err := executor.Execute(context.Background(), asset.Phase{Name: "sandbox-phase"}, "run")
	var execErr *ExecError
	if !errors.As(err, &execErr) {
		t.Fatalf("expected ExecError, got %v", err)
	}
	if execErr.Kind != KindConfig {
		t.Fatalf("kind = %v, want KindConfig", execErr.Kind)
	}
}

func TestSandboxOutputOverflowIsObservableKindFailed(t *testing.T) {
	runner := &fakeRunner{
		output: "retained", err: &sandbox.OutputLimitError{Limit: 8, Total: 12},
	}
	executor := sandboxedExecutor(runner)
	err := executor.Execute(context.Background(), asset.Phase{Name: "sandbox-phase"}, "run")
	var execErr *ExecError
	if !errors.As(err, &execErr) || execErr.Kind != KindFailed {
		t.Fatalf("output overflow must be KindFailed, got %v", err)
	}
	if !strings.Contains(err.Error(), "observed 12 bytes") {
		t.Fatalf("output overflow detail missing: %v", err)
	}
}

func TestSandboxTimeoutIsKindTimeout(t *testing.T) {
	// A deadline-exceeded context must classify as a retryable timeout.
	ctx, cancel := context.WithTimeout(context.Background(), time.Nanosecond)
	defer cancel()
	<-ctx.Done()
	runner := &fakeRunner{err: context.DeadlineExceeded}
	executor := sandboxedExecutor(runner)
	err := executor.Execute(ctx, asset.Phase{Name: "sandbox-phase"}, "run")
	var execErr *ExecError
	if !errors.As(err, &execErr) {
		t.Fatalf("expected ExecError, got %v", err)
	}
	if execErr.Kind != KindTimeout {
		t.Fatalf("kind = %v, want KindTimeout", execErr.Kind)
	}
}

func TestSandboxRunnerDeadlineIsKindTimeoutWhileParentIsLive(t *testing.T) {
	runner := &fakeRunner{err: fmt.Errorf("runner timed out: %w", context.DeadlineExceeded)}
	executor := sandboxedExecutor(runner)
	err := executor.Execute(
		context.Background(), asset.Phase{Name: "sandbox-phase"}, "run",
	)
	var execErr *ExecError
	if !errors.As(err, &execErr) || execErr.Kind != KindTimeout {
		t.Fatalf("wrapped runner deadline must be KindTimeout, got %v", err)
	}
}

func TestDeclaredSandboxWithoutRunnerFailsClosed(t *testing.T) {
	executor := CommandExecutor{
		Build: func(_ asset.Phase, _ string) []string {
			return []string{"agent"}
		},
		Sandbox: &SandboxConfig{Type: "firecracker"},
	}
	err := executor.Execute(context.Background(), asset.Phase{Name: "sandbox-phase"}, "run")
	var execErr *ExecError
	if !errors.As(err, &execErr) {
		t.Fatalf("expected ExecError, got %v", err)
	}
	if execErr.Kind != KindConfig {
		t.Fatalf("kind = %v, want KindConfig", execErr.Kind)
	}
	if !strings.Contains(err.Error(), "refusing host execution") {
		t.Fatalf("error must refuse host execution: %v", err)
	}
}

// TestSandboxAutoWireDockerRunsLiveContainer proves the configuration-only
// path: SandboxConfig{Type:"docker", Image} auto-wires a real Docker runner
// with no injected Runner. Skipped when the daemon is unreachable.
func TestSandboxAutoWireDockerRunsLiveContainer(t *testing.T) {
	probe := exec.Command("docker", "info")
	if err := probe.Run(); err != nil {
		t.Skipf("docker daemon unavailable: %v", err)
	}
	executor := CommandExecutor{
		Build: func(_ asset.Phase, _ string) []string {
			return []string{"/bin/echo", "AUTOWIRE-DOCKER-OK"}
		},
		Sandbox: &SandboxConfig{
			Type:  "docker",
			Image: "alpine:latest",
		},
	}
	err := executor.Execute(context.Background(), asset.Phase{Name: "sandbox-phase"}, "run")
	if err != nil {
		t.Fatalf("auto-wired docker Execute: %v", err)
	}
}

// TestSandboxAutoWireUnknownTypeFailsClosed proves an unknown Type still
// fails closed with no runner injected.
func TestSandboxAutoWireUnknownTypeFailsClosed(t *testing.T) {
	executor := CommandExecutor{
		Build: func(_ asset.Phase, _ string) []string {
			return []string{"agent"}
		},
		Sandbox: &SandboxConfig{Type: "podman"},
	}
	err := executor.Execute(context.Background(), asset.Phase{Name: "sandbox-phase"}, "run")
	var execErr *ExecError
	if !errors.As(err, &execErr) || execErr.Kind != KindConfig {
		t.Fatalf("unknown sandbox type must fail closed as KindConfig, got %v", err)
	}
}

// TestSandboxAutoWireFirecrackerRequiresKernelAndRootdir proves incomplete
// firecracker configuration fails closed instead of fabricating a runner.
func TestSandboxAutoWireFirecrackerRequiresKernelAndRootdir(t *testing.T) {
	executor := CommandExecutor{
		Build: func(_ asset.Phase, _ string) []string {
			return []string{"agent"}
		},
		Sandbox: &SandboxConfig{Type: "firecracker", Kernel: "/vmlinux.bin"},
	}
	err := executor.Execute(context.Background(), asset.Phase{Name: "sandbox-phase"}, "run")
	var execErr *ExecError
	if !errors.As(err, &execErr) || execErr.Kind != KindConfig {
		t.Fatalf("incomplete firecracker config must fail closed as KindConfig, got %v", err)
	}
	if !strings.Contains(err.Error(), "refusing host execution") {
		t.Fatalf("error must refuse host execution: %v", err)
	}
}

func TestSandboxAutoWireFirecrackerCarriesMemoryLimit(t *testing.T) {
	executor := CommandExecutor{MaxOutputBytes: 4096, Sandbox: &SandboxConfig{
		Type:     "firecracker",
		Kernel:   "/vmlinux.bin",
		Image:    "/rootdir",
		MemoryMB: 768,
	}}
	runner, err := executor.sandboxRunner()
	if err != nil {
		t.Fatalf("sandboxRunner: %v", err)
	}
	firecrackerRunner, ok := runner.(*firecracker.FirecrackerRunner)
	if !ok {
		t.Fatalf("runner type = %T, want *firecracker.FirecrackerRunner", runner)
	}
	if firecrackerRunner.MemoryMB != 768 {
		t.Fatalf("runner MemoryMB = %d, want 768", firecrackerRunner.MemoryMB)
	}
	if firecrackerRunner.MaxOutputBytes != 4096 {
		t.Fatalf("runner MaxOutputBytes = %d, want 4096", firecrackerRunner.MaxOutputBytes)
	}
}

func TestSandboxAutoWireDockerCarriesDefaultMemoryAndExecutorOutputLimit(t *testing.T) {
	executor := CommandExecutor{MaxOutputBytes: 8192, Sandbox: &SandboxConfig{
		Type: "docker", Image: "sandbox-image",
	}}
	runner, err := executor.sandboxRunner()
	if err != nil {
		t.Fatalf("sandboxRunner: %v", err)
	}
	dockerRunner, ok := runner.(*docker.Runner)
	if !ok {
		t.Fatalf("runner type = %T, want *docker.Runner", runner)
	}
	if dockerRunner.MemoryMB != sandbox.DefaultMemoryMB {
		t.Fatalf("runner MemoryMB = %d, want %d", dockerRunner.MemoryMB, sandbox.DefaultMemoryMB)
	}
	if dockerRunner.MaxOutputBytes != 8192 {
		t.Fatalf("runner MaxOutputBytes = %d, want 8192", dockerRunner.MaxOutputBytes)
	}
}

func TestSandboxAutoWireRejectsInvalidMemoryBeforeRunnerCreation(t *testing.T) {
	executor := CommandExecutor{Sandbox: &SandboxConfig{
		Type: "docker", Image: "sandbox-image", MemoryMB: sandbox.MinMemoryMB - 1,
	}}
	if _, err := executor.sandboxRunner(); err == nil || !strings.Contains(err.Error(), "memory must be between") {
		t.Fatalf("invalid memory error = %v", err)
	}
}

func TestSandboxAutoWireIsReceiverLocalUnderConcurrency(t *testing.T) {
	config := &SandboxConfig{Type: "docker", Image: "sandbox-image"}
	executor := CommandExecutor{Sandbox: config}
	const workers = 32
	var wait sync.WaitGroup
	wait.Add(workers)
	errors := make(chan error, workers)
	for range workers {
		go func() {
			defer wait.Done()
			local, err := executor.withSandboxRunner("parallel-phase")
			if err != nil {
				errors <- err
				return
			}
			if local.Sandbox == config || local.Sandbox.Runner == nil {
				errors <- fmt.Errorf("auto-wired sandbox was not receiver-local")
			}
		}()
	}
	wait.Wait()
	close(errors)
	for err := range errors {
		t.Fatal(err)
	}
	if config.Runner != nil {
		t.Fatal("shared SandboxConfig was mutated")
	}
}

// TestSandboxCancellationIsKindFailed proves a cancelled context is the
// caller's verdict (KindFailed), not a retryable timeout (review F5).
func TestSandboxCancellationIsKindFailed(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	runner := &fakeRunner{err: context.Canceled}
	executor := sandboxedExecutor(runner)
	err := executor.Execute(ctx, asset.Phase{Name: "sandbox-phase"}, "run")
	var execErr *ExecError
	if !errors.As(err, &execErr) {
		t.Fatalf("expected ExecError, got %v", err)
	}
	if execErr.Kind != KindFailed {
		t.Fatalf("kind = %v, want KindFailed on cancellation", execErr.Kind)
	}
}

func TestSandboxRunnerCancellationIsKindFailedWhileParentIsLive(t *testing.T) {
	runner := &fakeRunner{err: fmt.Errorf("runner cancelled: %w", context.Canceled)}
	executor := sandboxedExecutor(runner)
	err := executor.Execute(
		context.Background(), asset.Phase{Name: "sandbox-phase"}, "run",
	)
	var execErr *ExecError
	if !errors.As(err, &execErr) || execErr.Kind != KindFailed {
		t.Fatalf("wrapped runner cancellation must be KindFailed, got %v", err)
	}
}

func TestSandboxCancellationVisibleAtRunnerReturnSuppressesAcceptance(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	executor := sandboxedExecutor(cancelThenSucceedRunner{cancel: cancel})
	var validated, committed, observed bool
	executor.ValidateOutput = func(string, string) error { validated = true; return nil }
	executor.CommitValidatedOutput = func(string, string, string, time.Duration) error {
		committed = true
		return nil
	}
	executor.Observe = func(string, string, time.Duration) { observed = true }
	err := executor.Execute(ctx, asset.Phase{Name: "sandbox-phase"}, "run")
	var execErr *ExecError
	if !errors.As(err, &execErr) || execErr.Kind != KindFailed ||
		!errors.Is(err, context.Canceled) {
		t.Fatalf("cancelled success result = %v", err)
	}
	if validated || committed || observed {
		t.Fatalf("cancelled result was accepted: validate=%v commit=%v observe=%v",
			validated, committed, observed)
	}
}

func TestSandboxCancellationAfterRunnerReturnDoesNotRetroactivelyFail(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	executor := sandboxedExecutor(&fakeRunner{output: "accepted"})
	clockEntered := make(chan struct{})
	releaseClock := make(chan struct{})
	clockCalls := 0
	base := time.Unix(1_700_000_000, 0)
	executor.Now = func() time.Time {
		clockCalls++
		if clockCalls == 2 {
			close(clockEntered)
			<-releaseClock
			return base.Add(time.Second)
		}
		return base
	}
	var validated, committed, observed bool
	executor.ValidateOutput = func(string, string) error { validated = true; return nil }
	executor.CommitValidatedOutput = func(string, string, string, time.Duration) error {
		committed = true
		return nil
	}
	executor.Observe = func(string, string, time.Duration) { observed = true }
	done := make(chan error, 1)
	go func() {
		done <- executor.Execute(ctx, asset.Phase{Name: "sandbox-phase"}, "run")
	}()
	<-clockEntered
	cancel()
	close(releaseClock)
	if err := <-done; err != nil {
		t.Fatalf("post-linearization cancellation changed completed run: %v", err)
	}
	if !validated || !committed || !observed {
		t.Fatalf("completed result was not accepted: validate=%v commit=%v observe=%v",
			validated, committed, observed)
	}
}

// TestSandboxDeliversPromptViaStdin proves the stripped claude prompt
// reaches the runner (review F1): with PromptViaStdin the executor must
// hand the prompt to the sandbox, not drop it.
func TestSandboxDeliversPromptViaStdin(t *testing.T) {
	runner := &fakeRunner{output: "guest output", code: 0}
	executor := CommandExecutor{
		Build: func(_ asset.Phase, _ string) []string {
			return []string{"claude", "--model", "x", "-p", "THE-PROMPT"}
		},
		PromptViaStdin: true,
		Sandbox: &SandboxConfig{
			Type:       "firecracker",
			TimeoutSec: 7,
			Runner:     runner,
		},
	}
	err := executor.Execute(context.Background(), asset.Phase{Name: "sandbox-phase"}, "run")
	if err != nil {
		t.Fatalf("Execute: %v", err)
	}
	if runner.stdin != "THE-PROMPT" {
		t.Fatalf("runner stdin = %q, want the stripped prompt", runner.stdin)
	}
	if got := strings.Join(runner.argv, " "); got != "claude --model x -p" {
		t.Fatalf("runner argv = %q, want prompt-free argv", got)
	}
}
