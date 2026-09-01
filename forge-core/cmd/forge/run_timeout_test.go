package main

import (
	"strings"
	"testing"
)

func TestRunAndEvolveRejectNegativeAgentTimeoutAtCLI(t *testing.T) {
	for _, command := range []string{"run", "evolve"} {
		t.Run(command, func(t *testing.T) {
			code, output := captureChainOutput(t, func() int {
				return run([]string{command, "build", "--timeout=-1s"})
			})
			if code != 2 || !strings.Contains(output, "--timeout must be >= 0") {
				t.Fatalf("negative timeout result = %d, %q", code, output)
			}
		})
	}
}
