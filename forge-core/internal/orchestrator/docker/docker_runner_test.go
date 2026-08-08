package docker

import (
	"context"
	"errors"
	"os"
	"os/exec"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/orchestrator/sandbox"
)

func runProbe() error {
	return exec.Command("docker", "info").Run()
}

func TestShellJoinQuotesArgv(t *testing.T) {
	got := shellJoin([]string{"/bin/echo", "hello 'world'"})
	want := "'/bin/echo' 'hello '\"'\"'world'\"'\"''"
	if got != want {
		t.Fatalf("shellJoin = %q, want %q", got, want)
	}
}

func TestInvocationUsesSameContainerNameForRunAndCleanup(t *testing.T) {
	runner := &Runner{Image: "sandbox-image", MemoryMB: 256}
	invocation := invocationArgs(
		runner,
		[]string{"/bin/echo", "hello world"},
		"prompt",
		"forge-sandbox-fixed",
	)
	wantRun := "run --rm --name forge-sandbox-fixed --network none -i " +
		"--memory 256m sandbox-image /bin/sh -c '/bin/echo' 'hello world'"
	if got := strings.Join(invocation.run, " "); got != wantRun {
		t.Fatalf("run argv = %q, want %q", got, wantRun)
	}
	if got := strings.Join(invocation.cleanup, " "); got != "rm -f forge-sandbox-fixed" {
		t.Fatalf("cleanup argv = %q, want the run container name", got)
	}
}

func TestInvocationAppliesSafeDefaultMemoryLimit(t *testing.T) {
	runner := &Runner{Image: "sandbox-image"}
	invocation := invocationArgs(runner, []string{"true"}, "", "forge-sandbox-fixed")
	if got := strings.Join(invocation.run, " "); !strings.Contains(got, "--memory 512m") {
		t.Fatalf("default-memory run argv = %q", got)
	}
}

func TestCappedWriterReportsObservedOverflow(t *testing.T) {
	out := &cappedWriter{cap: 4}
	if _, err := out.Write([]byte("abcdef")); err != nil {
		t.Fatal(err)
	}
	if string(out.buf) != "abcd" || out.total != 6 {
		t.Fatalf("capture = %q total=%d", out.buf, out.total)
	}
	var limitErr *sandbox.OutputLimitError
	if err := out.limitError(); !errors.As(err, &limitErr) || !strings.Contains(err.Error(), "observed 6 bytes") {
		t.Fatalf("overflow error = %v", err)
	}
}

func TestCleanupUsesBoundedIndependentContextAndExactArgv(t *testing.T) {
	var gotBinary string
	var gotArgs []string
	err := cleanupContainer("docker-test", []string{"rm", "-f", "fixed"}, func(ctx context.Context, binary string, args ...string) error {
		gotBinary, gotArgs = binary, append([]string(nil), args...)
		if ctx.Err() != nil {
			t.Fatalf("cleanup context starts cancelled: %v", ctx.Err())
		}
		deadline, ok := ctx.Deadline()
		if !ok || time.Until(deadline) <= 0 || time.Until(deadline) > cleanupTimeout {
			t.Fatalf("cleanup deadline = %v, now=%v", deadline, time.Now())
		}
		return context.DeadlineExceeded
	})
	if !errors.Is(err, context.DeadlineExceeded) {
		t.Fatalf("cleanup error = %v", err)
	}
	if gotBinary != "docker-test" || strings.Join(gotArgs, " ") != "rm -f fixed" {
		t.Fatalf("cleanup invocation = %q %q", gotBinary, gotArgs)
	}
}

func TestRunnerRejectsInvalidMemoryBeforeDockerProbe(t *testing.T) {
	runner := &Runner{Image: "sandbox-image", MemoryMB: 63}
	_, _, err := runner.Run(context.Background(), []string{"true"}, "", 0)
	if err == nil || !strings.Contains(err.Error(), "memory must be between") {
		t.Fatalf("invalid memory error = %v", err)
	}
}

func TestRunnerWithoutImageFailsClosed(t *testing.T) {
	runner := &Runner{Binary: "definitely-missing-docker-binary"}
	_, _, err := runner.Run(context.Background(), []string{"echo", "hi"}, "", 0)
	if err == nil {
		t.Fatal("runner without image must fail closed")
	}
	if !strings.Contains(err.Error(), "image must be configured") {
		t.Fatalf("unexpected error: %v", err)
	}
}

// TestRunnerLiveContainer runs a real container when the Docker daemon is
// reachable; it is skipped otherwise (CI may lack Docker). This is the
// host-verified counterpart to the Firecracker live microVM test.
func TestRunnerLiveContainer(t *testing.T) {
	if os.Getenv("FORGE_DOCKER_IMAGE") == "" {
		// Probe the daemon: if Docker is absent, skip honestly instead of
		// failing the suite in environments without it.
		probe := runProbe()
		if probe != nil {
			t.Skipf("docker daemon unavailable: %v", probe)
		}
	}
	image := os.Getenv("FORGE_DOCKER_IMAGE")
	if image == "" {
		image = "alpine:latest"
	}
	runner := &Runner{Image: image, Logf: t.Logf}
	output, code, err := runner.Run(
		context.Background(),
		[]string{"/bin/echo", "FORGELIVE-DOCKER-OK"},
		"",
		60*time.Second,
	)
	if err != nil {
		t.Fatalf("live container run: %v", err)
	}
	if code != 0 {
		t.Fatalf("container exit = %d, want 0", code)
	}
	if !strings.Contains(output, "FORGELIVE-DOCKER-OK") {
		t.Fatalf("container output missing marker: %q", output)
	}
	t.Logf("live container verified: %q", output)
}

func TestRunnerLiveContainerNonZeroExit(t *testing.T) {
	image := os.Getenv("FORGE_DOCKER_IMAGE")
	if image == "" {
		probe := runProbe()
		if probe != nil {
			t.Skipf("docker daemon unavailable: %v", probe)
		}
		image = "alpine:latest"
	}
	runner := &Runner{Image: image}
	output, code, err := runner.Run(
		context.Background(),
		[]string{"/bin/sh", "-c", "exit 7"},
		"",
		60*time.Second,
	)
	if err != nil {
		t.Fatalf("live container run: %v", err)
	}
	if code != 7 {
		t.Fatalf("container exit = %d, want 7", code)
	}
	_ = output
}

// TestRunnerLiveContainerStdin proves the prompt delivery path: guest
// reads its stdin and echoes it back (review F1 verification).
func TestRunnerLiveContainerStdin(t *testing.T) {
	image := os.Getenv("FORGE_DOCKER_IMAGE")
	if image == "" {
		probe := runProbe()
		if probe != nil {
			t.Skipf("docker daemon unavailable: %v", probe)
		}
		image = "alpine:latest"
	}
	runner := &Runner{Image: image}
	output, code, err := runner.Run(
		context.Background(),
		[]string{"/bin/cat"},
		"FORGELIVE-STDIN-PROMPT",
		60*time.Second,
	)
	if err != nil {
		t.Fatalf("live container stdin run: %v", err)
	}
	if code != 0 {
		t.Fatalf("container exit = %d, want 0", code)
	}
	if !strings.Contains(output, "FORGELIVE-STDIN-PROMPT") {
		t.Fatalf("container output missing prompt echo: %q", output)
	}
	t.Logf("live container stdin verified: %q", output)
}
