package docker

import (
	"context"
	"os"
	"os/exec"
	"strings"
	"testing"
	"time"
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
