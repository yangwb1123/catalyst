package main

import (
	"fmt"
	"time"

	"forgeos/forge-core/internal/orchestrator/sandbox"
)

func validateSandboxMemory(memoryMB int) error {
	if memoryMB < sandbox.MinMemoryMB || memoryMB > sandbox.MaxMemoryMB {
		return fmt.Errorf("--sandbox-memory-mb must be between %d and %d", sandbox.MinMemoryMB, sandbox.MaxMemoryMB)
	}
	return nil
}

func validateAgentTimeout(timeout time.Duration) error {
	if timeout < 0 {
		return fmt.Errorf("--timeout must be >= 0 (got %s)", timeout)
	}
	return nil
}
