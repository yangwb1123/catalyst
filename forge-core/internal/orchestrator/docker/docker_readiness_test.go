package docker

import (
	"context"
	"errors"
	"strings"
	"testing"
	"time"
)

func TestCheckReadyUsesRunTimeoutContext(t *testing.T) {
	runner := &Runner{Image: "sandbox-image"}
	runCtx, cancel := boundedRunContext(context.Background(), 20*time.Millisecond)
	defer cancel()
	started := time.Now()
	err := runner.checkReady(runCtx, "docker-test", func(ctx context.Context, _ string, args ...string) error {
		if strings.Join(args, " ") != "info" {
			t.Fatalf("readiness argv = %q", args)
		}
		if _, ok := ctx.Deadline(); !ok {
			t.Fatal("readiness probe has no run deadline")
		}
		<-ctx.Done()
		return ctx.Err()
	})
	if !errors.Is(err, context.DeadlineExceeded) || time.Since(started) > time.Second {
		t.Fatalf("bounded readiness error = %v after %s", err, time.Since(started))
	}
	message := readinessError(err, runCtx.Err(), 20*time.Millisecond).Error()
	if !strings.Contains(message, "readiness probe timed out after 20ms") {
		t.Fatalf("readiness timeout diagnostic = %q", message)
	}
}

func TestCheckReadyPreservesCallerCancellation(t *testing.T) {
	runner := &Runner{Image: "sandbox-image"}
	runCtx, cancel := boundedRunContext(context.Background(), 0)
	cancel()
	err := runner.checkReady(runCtx, "docker-test", func(ctx context.Context, _ string, _ ...string) error {
		return ctx.Err()
	})
	result := readinessError(err, runCtx.Err(), 0)
	if !errors.Is(result, context.Canceled) ||
		!strings.Contains(result.Error(), "readiness probe cancelled") ||
		strings.Contains(result.Error(), "timed out") {
		t.Fatalf("readiness cancellation diagnostic = %v", result)
	}
}

func TestParentDeadlineWithoutRunnerTimeoutDoesNotReportZeroSeconds(t *testing.T) {
	err := interruptionError("docker container", context.DeadlineExceeded, 0)
	if !errors.Is(err, context.DeadlineExceeded) ||
		!strings.Contains(err.Error(), "deadline exceeded") || strings.Contains(err.Error(), "0s") {
		t.Fatalf("parent deadline diagnostic = %v", err)
	}
}
