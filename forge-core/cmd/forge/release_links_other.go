//go:build !linux

package main

import (
	"fmt"
	"os"
	"path/filepath"
	"runtime"
)

// Release stages fail closed on platforms where this build cannot prove that a
// declared output is not a hard-link alias to a file outside docs/release.
func releaseRegularSingleLink(os.FileInfo) bool {
	return false
}

const releasePinnedExecCommand = "__forge-release-exec-pinned"

func releasePinnedExecutionSupport() error {
	return fmt.Errorf(
		"release executable pinning is unavailable on %s; refusing pathname-based execution",
		runtime.GOOS,
	)
}

func trustedReleaseGitExecutable() (string, error) {
	return "", fmt.Errorf(
		"release source inventory requires a trusted Linux host Git; unavailable on %s",
		runtime.GOOS,
	)
}

func verifyForgeGitRoot(root string) (bool, error) {
	_, err := os.Lstat(filepath.Join(root, ".git"))
	if os.IsNotExist(err) {
		return false, nil
	}
	if err != nil {
		return false, fmt.Errorf("inspect Git control path: %w", err)
	}
	return true, nil
}

// Non-Linux release execution already fails closed. Generic chain operation
// remains available where this build has no verified host-Git TCB.
func rejectTrackedForgeControlState(string) error {
	return nil
}

func releasePinnedLauncherArgv(_, _ string, _ []string) []string {
	return nil
}

func cmdReleaseExecPinned(_ []string) int {
	fmt.Fprintf(
		os.Stderr,
		"forge: release executable pinning is unavailable on %s\n",
		runtime.GOOS,
	)
	return 126
}
