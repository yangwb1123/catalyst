package firecracker

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestGuestInitScriptQuotesArgvAndRecordsExit(t *testing.T) {
	script := guestInitScript([]string{"/bin/echo", "hello 'world'"})
	if !strings.Contains(script, "mount -t proc none /proc") {
		t.Fatal("init script must mount proc")
	}
	if !strings.Contains(script, `'/bin/echo' 'hello '"'"'world'"'"''`) {
		t.Fatalf("argv not shell-quoted: %s", script)
	}
	if !strings.Contains(script, "echo $? > /forge-exit") {
		t.Fatal("init script must record the exit code marker")
	}
	if !strings.Contains(script, "poweroff -f") {
		t.Fatal("init script must power off the guest")
	}
}

func TestGuestOutputExtractsBetweenMarkers(t *testing.T) {
	dir := t.TempDir()
	logPath := filepath.Join(dir, "firecracker.log")
	content := strings.Join([]string{
		"2026-08-05T18:00:00 [anonymous-instance:main] booting",
		"2026-08-05T18:00:01 [anonymous-instance:serial] FORGE-GUEST-START",
		"2026-08-05T18:00:02 [anonymous-instance:serial] guest line one",
		"2026-08-05T18:00:03 [anonymous-instance:serial] FORGE-GUEST-DONE",
	}, "\n")
	if err := os.WriteFile(logPath, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
	got := guestOutput(logPath)
	want := "guest line one"
	if got != want {
		t.Fatalf("guestOutput = %q, want %q", got, want)
	}
}

func TestReadMarkerParsesExitCode(t *testing.T) {
	code, found, err := parseMarkerText("0\n")
	if err != nil || !found || code != 0 {
		t.Fatalf("parse 0: code=%d found=%v err=%v", code, found, err)
	}
	code, found, err = parseMarkerText("127\n")
	if err != nil || !found || code != 127 {
		t.Fatalf("parse 127: code=%d found=%v err=%v", code, found, err)
	}
	_, found, err = parseMarkerText("not-a-number")
	if err == nil || found {
		t.Fatalf("garbage marker must fail: found=%v err=%v", found, err)
	}
}

// TestFirecrackerRunnerLiveMicroVM boots a real KVM microVM when
// FORGE_FIRECRACKER_KERNEL and FORGE_FIRECRACKER_ROOTFS point at a vmlinux
// and ext4 rootfs template (see docs/external-resource-verification.md). It
// is skipped otherwise — CI exercises the fake-runner wiring above; this is
// the host-verified counterpart proving the debugfs injection, serial log
// capture, and marker read-back work against a real Firecracker.
func TestFirecrackerRunnerLiveMicroVM(t *testing.T) {
	kernel := os.Getenv("FORGE_FIRECRACKER_KERNEL")
	if kernel == "" {
		t.Skip("FORGE_FIRECRACKER_KERNEL unset; skipping live microVM test")
	}
	rootdir := os.Getenv("FORGE_FIRECRACKER_ROOTDIR")
	if rootdir == "" {
		t.Skip("FORGE_FIRECRACKER_ROOTDIR unset; skipping live microVM test")
	}
	runner := &FirecrackerRunner{
		Binary:  os.Getenv("FORGE_FIRECRACKER_BINARY"),
		DebugFS: os.Getenv("FORGE_FIRECRACKER_DEBUGFS"),
		Mke2fs:  os.Getenv("FORGE_FIRECRACKER_MKE2FS"),
		Kernel:  kernel,
		RootDir: rootdir,
		Logf:    t.Logf,
	}
	output, code, err := runner.Run(
		context.Background(),
		[]string{"/bin/echo", "FORGELIVE-VM-OK"},
		120*time.Second,
	)
	if err != nil {
		t.Fatalf("live microVM run: %v", err)
	}
	if code != 0 {
		t.Fatalf("guest exit = %d, want 0", code)
	}
	if !strings.Contains(output, "FORGELIVE-VM-OK") {
		t.Fatalf("guest output missing marker: %q", output)
	}
	t.Logf("live microVM verified: %q", output)
}
