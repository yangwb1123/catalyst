package main

import (
	"flag"
	"path/filepath"
	"testing"

	"forgeos/forge-core/internal/mode"
)

// Unknown lifecycle values are deliberately not rejected at the configuration
// boundary: both CLI and project.yml preserve the raw value so mode.Effective can
// apply its strict fail-safe. Pin both input paths to the same cto build halt.
func TestUnknownLifecycleInputsPreserveCTOBuildHalt(t *testing.T) {
	const unknown = "typo-lifecycle"

	root := t.TempDir()
	mkdir(t, filepath.Join(root, ".agent"))
	writeFile(t, filepath.Join(root, ".agent", "project.yml"), "lifecycle: "+unknown+"\n")

	var cli runOpts
	fs := flag.NewFlagSet("lifecycle-failsafe", flag.ContinueOnError)
	bindRunOpts(fs, &cli)
	if err := fs.Parse([]string{"--mode", "cto", "--lifecycle", unknown, "--root", root}); err != nil {
		t.Fatal(err)
	}

	for _, tc := range []struct {
		name string
		opts runOpts
	}{
		{name: "cli flag", opts: cli},
		{name: "project config", opts: runOpts{mode: "cto", root: root}},
	} {
		t.Run(tc.name, func(t *testing.T) {
			lifecycle := resolveLifecycle(tc.opts)
			if lifecycle != unknown {
				t.Fatalf("resolved lifecycle = %q, want raw unknown value %q", lifecycle, unknown)
			}
			policy := mode.Effective("cto", lifecycle)
			if !policy.BuildHalted() {
				t.Fatal("unknown lifecycle relaxed cto workflow_depth.build=halt")
			}
			for _, gateName := range []string{
				mode.GateLint, mode.GateTest, mode.GateBuild,
				mode.GateComplexity, mode.GateArch, mode.GateSecurity,
			} {
				if !policy.Allows(gateName) {
					t.Errorf("unknown lifecycle must retain strict floor gate %q", gateName)
				}
			}
		})
	}
}
