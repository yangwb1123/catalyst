package appserver

import (
	"path/filepath"
	"strings"
	"testing"
)

func testConfig(t *testing.T) Config {
	t.Helper()
	return Config{
		ListenAddress: "127.0.0.1:0",
		StateDir:      filepath.Join(t.TempDir(), "state"),
		Build:         BuildInfo{Version: "dev", Commit: "abc123"},
	}
}

func TestConfigAcceptsLiteralLoopbackAddresses(t *testing.T) {
	for _, address := range []string{"127.0.0.1:0", "127.0.0.1:7467", "[::1]:7467"} {
		config := testConfig(t)
		config.ListenAddress = address
		if err := config.Validate(); err != nil {
			t.Errorf("Validate(%q): %v", address, err)
		}
	}
}

func TestConfigRejectsUnsafeListenAddresses(t *testing.T) {
	addresses := []string{"", "localhost:7467", "0.0.0.0:7467", "[::]:7467", "127.0.0.1:-1", "127.0.0.1:65536"}
	for _, address := range addresses {
		config := testConfig(t)
		config.ListenAddress = address
		if err := config.Validate(); err == nil {
			t.Errorf("Validate(%q) succeeded", address)
		}
	}
}

func TestConfigRejectsUnsafeStateDirectories(t *testing.T) {
	base := t.TempDir()
	paths := []string{
		"", "relative/state", string(filepath.Separator),
		base + string(filepath.Separator),
		base + "//state",
		base + "/missing/../state",
		base + "/./state",
	}
	for _, path := range paths {
		config := testConfig(t)
		config.StateDir = path
		if err := config.Validate(); err == nil {
			t.Errorf("Validate state-dir %q succeeded", path)
		}
	}
}

func TestConfigBoundsStateDirectoryBeforeCleaning(t *testing.T) {
	config := testConfig(t)
	config.StateDir = string(filepath.Separator) + strings.Repeat("a", maxStateDirBytes-1)
	if err := config.Validate(); err != nil {
		t.Fatalf("state-dir byte boundary: %v", err)
	}
	config.StateDir += "a"
	if err := config.Validate(); err == nil {
		t.Fatal("oversized state-dir succeeded")
	}
	components := make([]string, maxStateDirComponents)
	for index := range components {
		components[index] = "a"
	}
	config.StateDir = string(filepath.Separator) + filepath.Join(components...)
	if err := config.Validate(); err != nil {
		t.Fatalf("state-dir component boundary: %v", err)
	}
	config.StateDir += string(filepath.Separator) + "a"
	if err := config.Validate(); err == nil {
		t.Fatal("state-dir with too many components succeeded")
	}
}

func TestConfigRejectsUnboundedBuildIdentity(t *testing.T) {
	config := testConfig(t)
	config.Build.Version = ""
	if err := config.Validate(); err == nil {
		t.Fatal("empty version succeeded")
	}
	config.Build.Version = "dev"
	config.Build.Commit = strings.Repeat("a", 129)
	if err := config.Validate(); err == nil {
		t.Fatal("oversized commit succeeded")
	}
	config.Build.Commit = "bad commit"
	if err := config.Validate(); err == nil {
		t.Fatal("commit with whitespace succeeded")
	}
}
