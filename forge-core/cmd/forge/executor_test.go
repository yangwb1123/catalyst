package main

import (
	"flag"
	"testing"

	"forgeos/forge-core/internal/orchestrator/sandbox"
)

func TestSandboxConfigMapsCLIFlags(t *testing.T) {
	if got := sandboxConfig(runOpts{}); got != nil {
		t.Fatalf("empty sandbox must select host execution, got %+v", got)
	}
	cfg := sandboxConfig(runOpts{
		sandbox:         "docker",
		sandboxImage:    "alpine:latest",
		sandboxKernel:   "/vmlinux.bin",
		sandboxMemoryMB: 768,
	})
	if cfg == nil || cfg.Type != "docker" || cfg.Image != "alpine:latest" {
		t.Fatalf("docker config mismatch: %+v", cfg)
	}
	if cfg.Kernel != "/vmlinux.bin" {
		t.Fatalf("unused kernel must still round-trip: %+v", cfg)
	}
	if cfg.MemoryMB != 768 {
		t.Fatalf("docker memory limit mismatch: %+v", cfg)
	}
	fc := sandboxConfig(runOpts{
		sandbox: "firecracker", sandboxImage: "/rootdir", sandboxKernel: "/vmlinux.bin",
		sandboxMemoryMB: 1024,
	})
	if fc == nil || fc.Type != "firecracker" || fc.Image != "/rootdir" || fc.Kernel != "/vmlinux.bin" {
		t.Fatalf("firecracker config mismatch: %+v", fc)
	}
	if fc.MemoryMB != 1024 {
		t.Fatalf("firecracker memory limit mismatch: %+v", fc)
	}
}

func TestSandboxMemoryFlagHasSafeDefaultAndValidatesExplicitBounds(t *testing.T) {
	var opts runOpts
	flags := flag.NewFlagSet("sandbox-memory", flag.ContinueOnError)
	bindRunOpts(flags, &opts)
	if err := flags.Parse(nil); err != nil {
		t.Fatal(err)
	}
	if opts.sandboxMemoryMB != sandbox.DefaultMemoryMB {
		t.Fatalf("default memory = %d, want %d", opts.sandboxMemoryMB, sandbox.DefaultMemoryMB)
	}
	if err := flags.Parse([]string{"--sandbox-memory-mb", "2048"}); err != nil {
		t.Fatal(err)
	}
	if opts.sandboxMemoryMB != 2048 {
		t.Fatalf("explicit memory = %d, want 2048", opts.sandboxMemoryMB)
	}
	for _, invalid := range []int{0, 63, 32*1024 + 1} {
		if err := validateSandboxMemory(invalid); err == nil {
			t.Errorf("memory %d unexpectedly valid", invalid)
		}
	}
}

func TestRunAndEvolveRejectInvalidSandboxMemoryAtCLIEntry(t *testing.T) {
	args := []string{"missing-workflow", "--sandbox-memory-mb", "63"}
	if code := cmdRun(args); code != 2 {
		t.Fatalf("run invalid-memory exit = %d, want 2", code)
	}
	if code := cmdEvolve(args); code != 2 {
		t.Fatalf("evolve invalid-memory exit = %d, want 2", code)
	}
}
