// Package sandbox defines the isolation contract between the orchestrator
// and its sandbox runners (Firecracker microVM, Docker container). Each
// runner executes a command in an isolated environment and reports the
// captured output, the exit code, and infrastructure failures for the
// executor to classify.
package sandbox

import (
	"context"
	"time"
)

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
