package main

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestForgeServerCrossBuildsSupportedAndFailClosedTargets(t *testing.T) {
	targets := []struct {
		goos      string
		goarch    string
		supported bool
	}{
		{"linux", "amd64", true},
		{"android", "arm64", false},
		{"aix", "ppc64", false},
		{"darwin", "amd64", false},
		{"illumos", "amd64", false},
		{"solaris", "amd64", false},
		{"windows", "amd64", false},
	}
	for _, target := range targets {
		t.Run(target.goos+"-"+target.goarch, func(t *testing.T) {
			environment := crossBuildEnvironment(target.goos, target.goarch)
			output := filepath.Join(t.TempDir(), "forge-server")
			command := exec.Command("go", "build", "-o", output, ".")
			command.Env = environment
			if combined, err := command.CombinedOutput(); err != nil {
				t.Fatalf("cross-build: %v, output=%q", err, combined)
			}
			assertLockImplementation(t, environment, target.supported)
		})
	}
}

func assertLockImplementation(t *testing.T, environment []string, supported bool) {
	t.Helper()
	command := exec.Command("go", "list", "-f", "{{join .GoFiles \",\"}}",
		"forgeos/forge-core/internal/appserver")
	command.Env = environment
	output, err := command.CombinedOutput()
	if err != nil {
		t.Fatalf("list lock implementation: %v, output=%q", err, output)
	}
	want := "instance_lock_other.go"
	if supported {
		want = "instance_lock_unix.go"
	}
	if !strings.Contains(string(output), want) {
		t.Fatalf("selected lock implementation = %q, want %s", output, want)
	}
}

func crossBuildEnvironment(goos, goarch string) []string {
	environment := make([]string, 0, len(os.Environ())+4)
	for _, value := range os.Environ() {
		name := strings.SplitN(value, "=", 2)[0]
		if name != "GOOS" && name != "GOARCH" && name != "CGO_ENABLED" && name != "GOFLAGS" {
			environment = append(environment, value)
		}
	}
	return append(environment,
		"GOOS="+goos, "GOARCH="+goarch, "CGO_ENABLED=0", "GOFLAGS=-buildvcs=false")
}
