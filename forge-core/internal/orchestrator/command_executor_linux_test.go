//go:build linux

package orchestrator

import (
	"context"
	"path/filepath"
	"testing"
	"time"

	"forgeos/forge-core/internal/asset"
)

// The Linux group-kill proof is platform-specific because non-Linux targets
// deliberately use race-safe direct-child termination plus the drain backstop.
func TestCommandExecutor_ProcessGroup_GrandchildReaped(t *testing.T) {
	if testing.Short() {
		t.Skip("spawns real grandchild processes; skipped under -short")
	}
	pidFile := filepath.Join(t.TempDir(), "grandchild.pid")
	ex := CommandExecutor{
		Build:   func(asset.Phase, string) []string { return grandchildSpawner(pidFile, 30) },
		Timeout: 300 * time.Millisecond,
	}

	err := ex.Execute(context.Background(), asset.Phase{Name: "slow"}, "m")
	pid := readPID(waitForFile(pidFile, time.Second))
	if pid == 0 {
		t.Fatal("grandchild never recorded its pid; test construction broken")
	}
	t.Cleanup(func() { killPID(pid) })

	execErr := requireExecError(t, err)
	if execErr.Kind != KindTimeout {
		t.Errorf("want KindTimeout, got %v", execErr.Kind)
	}
	if !waitGone(pid, 3*time.Second) {
		t.Errorf("grandchild pid %d still alive after a group-killed timeout; process-group teardown failed", pid)
	}
}
