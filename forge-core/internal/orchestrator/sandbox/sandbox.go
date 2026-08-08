// Package sandbox defines the isolation contract between the orchestrator
// and its sandbox runners (Firecracker microVM, Docker container). Each
// runner executes a command in an isolated environment and reports the
// captured output, the exit code, and infrastructure failures for the
// executor to classify.
package sandbox

import (
	"context"
	"fmt"
	"time"
)

const (
	// DefaultMemoryMB is the safe RAM limit used when a runner is created
	// without an explicit limit.
	DefaultMemoryMB = 512
	// MinMemoryMB and MaxMemoryMB reject nonsensical or dangerously broad
	// sandbox configurations before a process or VM is started.
	MinMemoryMB = 64
	MaxMemoryMB = 32 * 1024
	// DefaultMaxOutputBytes matches CommandExecutor's retained-output default.
	DefaultMaxOutputBytes = 10 << 20
)

// EffectiveMemoryMB resolves zero to the safe default and validates explicit
// limits. It is shared by every runner so Docker and Firecracker cannot drift.
func EffectiveMemoryMB(configured int) (int, error) {
	if configured == 0 {
		return DefaultMemoryMB, nil
	}
	if configured < MinMemoryMB || configured > MaxMemoryMB {
		return 0, fmt.Errorf("sandbox memory must be between %d and %d MiB", MinMemoryMB, MaxMemoryMB)
	}
	return configured, nil
}

// OutputLimitError reports that a sandbox produced more output than its
// bounded host capture could retain. Total is the observed lower bound when
// the runner stops immediately after overflow.
type OutputLimitError struct {
	Limit int
	Total int
}

func (e *OutputLimitError) Error() string {
	return fmt.Sprintf("sandbox output exceeded %d-byte limit (observed %d bytes)", e.Limit, e.Total)
}

// Runner executes a command inside an isolated environment. Run returns the
// captured output, the guest exit code (0 on success), and an infrastructure
// error (config fault, timeout, or transport failure). A non-zero code with
// a nil error is a clean run that failed. stdin is delivered to the guest's
// standard input (empty when the command needs none) — for claude-family
// agents under PromptViaStdin the prompt MUST reach the guest or the phase
// runs with no task.
type Runner interface {
	Run(ctx context.Context, argv []string, stdin string, timeout time.Duration) (output string, exitCode int, err error)
}
