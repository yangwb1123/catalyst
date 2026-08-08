package firecracker

import (
	"context"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"time"

	"forgeos/forge-core/internal/orchestrator/sandbox"
)

func firecrackerRunContext(
	ctx context.Context,
	timeout time.Duration,
) (context.Context, context.CancelFunc) {
	if timeout > 0 {
		return context.WithTimeout(ctx, timeout)
	}
	return context.WithCancel(ctx)
}

func firecrackerContextError(cause error, timeout time.Duration) error {
	if errors.Is(cause, context.DeadlineExceeded) && timeout > 0 {
		return fmt.Errorf("firecracker run timed out after %s: %w", timeout, cause)
	}
	return fmt.Errorf("firecracker run interrupted: %w", cause)
}

func (r *FirecrackerRunner) toolBinaries() (string, string, string) {
	firecracker, debugfs, mke2fs := r.Binary, r.DebugFS, r.Mke2fs
	if firecracker == "" {
		firecracker = "firecracker"
	}
	if debugfs == "" {
		debugfs = "debugfs"
	}
	if mke2fs == "" {
		mke2fs = "mke2fs"
	}
	return firecracker, debugfs, mke2fs
}

// checkReady validates tools through PATH and proves KVM can be opened
// read/write; mere device existence is not claimed as host readiness.
func (r *FirecrackerRunner) checkReady(firecracker, debugfs, mke2fs string) error {
	if _, err := sandbox.EffectiveMemoryMB(r.MemoryMB); err != nil {
		return configFault(err)
	}
	if r.Kernel == "" || r.RootDir == "" {
		return configFault(errors.New("firecracker runner: kernel and rootdir must be configured"))
	}
	for _, path := range []string{r.Kernel, r.RootDir} {
		if _, err := os.Stat(path); err != nil {
			return configFault(fmt.Errorf("firecracker runner: %s unavailable: %w", path, err))
		}
	}
	for _, tool := range []string{firecracker, debugfs, mke2fs} {
		if err := checkHostTool(tool); err != nil {
			return configFault(err)
		}
	}
	kvm, err := os.OpenFile("/dev/kvm", os.O_RDWR, 0)
	if err != nil {
		return configFault(fmt.Errorf("firecracker runner: /dev/kvm is not usable read/write: %w", err))
	}
	_ = kvm.Close()
	return nil
}

func checkHostTool(tool string) error {
	if _, err := exec.LookPath(tool); err != nil {
		return fmt.Errorf("firecracker runner: tool %s unavailable on PATH: %w", tool, err)
	}
	return nil
}
