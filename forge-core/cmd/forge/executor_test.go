package main

import "testing"

func TestSandboxConfigMapsCLIFlags(t *testing.T) {
	if got := sandboxConfig(runOpts{}); got != nil {
		t.Fatalf("empty sandbox must select host execution, got %+v", got)
	}
	cfg := sandboxConfig(runOpts{
		sandbox:       "docker",
		sandboxImage:  "alpine:latest",
		sandboxKernel: "/vmlinux.bin",
	})
	if cfg == nil || cfg.Type != "docker" || cfg.Image != "alpine:latest" {
		t.Fatalf("docker config mismatch: %+v", cfg)
	}
	if cfg.Kernel != "/vmlinux.bin" {
		t.Fatalf("unused kernel must still round-trip: %+v", cfg)
	}
	fc := sandboxConfig(runOpts{sandbox: "firecracker", sandboxImage: "/rootdir", sandboxKernel: "/vmlinux.bin"})
	if fc == nil || fc.Type != "firecracker" || fc.Image != "/rootdir" || fc.Kernel != "/vmlinux.bin" {
		t.Fatalf("firecracker config mismatch: %+v", fc)
	}
}
