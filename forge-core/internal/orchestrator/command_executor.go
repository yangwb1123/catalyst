package orchestrator

import (
	"fmt"
	"os/exec"
	"strings"

	"forgeos/forge-core/internal/asset"
)

// CommandExecutor runs an external command per agent phase — the real bridge
// beyond DryRunExecutor. Build produces the argv for a phase under a mode
// (e.g. ["claude", "-p", prompt]); pointing it at a real agent CLI is how
// forge-core graduates from narrating to actually driving agents. It is
// verified with stub commands here; driving a real LLM agent additionally
// needs that CLI plus credentials in the environment.
type CommandExecutor struct {
	// Build returns the argv to run for a phase. An empty result is an error.
	Build func(p asset.Phase, mode string) []string
	Log   func(string)
}

// Execute builds and runs the phase's command, failing closed on a missing
// Build, an empty argv, or a non-zero exit so a broken executor never
// masquerades as success — and never panics on a nil Build.
func (c CommandExecutor) Execute(p asset.Phase, mode string) error {
	if c.Build == nil {
		return fmt.Errorf("phase %s: command executor has no Build configured", p.Name)
	}
	argv := c.Build(p, mode)
	if len(argv) == 0 {
		return fmt.Errorf("phase %s: command executor produced empty argv", p.Name)
	}
	out, err := exec.Command(argv[0], argv[1:]...).CombinedOutput()
	c.logf("phase %s: ran %q -> %s", p.Name, strings.Join(argv, " "), strings.TrimSpace(string(out)))
	if err != nil {
		return fmt.Errorf("phase %s: command %q failed: %w", p.Name, argv[0], err)
	}
	return nil
}

func (c CommandExecutor) logf(format string, args ...any) {
	if c.Log != nil {
		c.Log(fmt.Sprintf(format, args...))
	}
}
