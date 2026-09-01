package firecracker

import (
	"context"
	"errors"
	"math"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/orchestrator/sandbox"
)

func TestRunnerRejectsOversizedStdinBeforeHostPrerequisites(t *testing.T) {
	runner := &FirecrackerRunner{}
	_, _, err := runner.Run(
		context.Background(), []string{"true"}, strings.Repeat("x", 16<<20+1), 0,
	)
	if err == nil || !strings.Contains(err.Error(), "stdin exceeds") {
		t.Fatalf("oversized stdin error = %v", err)
	}
}

func TestRunnerRejectsEmptyArgvBeforeHostPrerequisites(t *testing.T) {
	runner := &FirecrackerRunner{}
	_, _, err := runner.Run(context.Background(), nil, "", 0)
	if err == nil || !strings.Contains(err.Error(), "empty guest argv") {
		t.Fatalf("empty argv error = %v", err)
	}
}

func TestRunnerRejectsNegativeOutputLimitBeforeHostPrerequisites(t *testing.T) {
	runner := &FirecrackerRunner{MaxOutputBytes: -1}
	_, _, err := runner.Run(context.Background(), []string{"true"}, "", 0)
	if err == nil || !strings.Contains(err.Error(), "max output bytes must be >= 0") {
		t.Fatalf("negative output limit error = %v", err)
	}
}

func TestGuestInitScriptDropsPrivilegesAndRecordsOutOfBandStatus(t *testing.T) {
	script := guestInitScript(
		[]string{"/bin/echo", "hello 'world'"}, "",
		"/forge-result-00000000000000000000000000000000",
	)
	if !strings.Contains(script, "mount -t proc none /proc") {
		t.Fatal("init script must mount proc")
	}
	if !strings.Contains(script, `'/bin/echo' 'hello '"'"'world'"'"''`) {
		t.Fatalf("argv not shell-quoted: %s", script)
	}
	for _, required := range []string{
		`mkdir -m 700 "$FORGE_RESULT"`,
		"--reuid=65534 --regid=65534 --clear-groups",
		"--bounding-set=-all --no-new-privs",
		"/bin/env -i PATH=/bin:/sbin:/usr/bin:/usr/sbin",
		"CapBnd:",
		"NoNewPrivs:",
		`> "$FORGE_RESULT/output" 2>&1`,
		`write_status "exit:$code"`,
		"< /dev/null",
	} {
		if !strings.Contains(script, required) {
			t.Fatalf("trusted init missing %q: %s", required, script)
		}
	}
	if strings.Contains(script, "FORGE-GUEST-") || strings.Contains(script, "/forge-exit") {
		t.Fatal("workload-forgeable completion protocol remains in init script")
	}
}

func TestGuestInitScriptHasValidShellSyntax(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("POSIX guest init syntax")
	}
	script := guestInitScript(
		[]string{"/bin/echo", "hello 'world'"}, "prompt",
		"/forge-result-00000000000000000000000000000000",
	)
	command := exec.Command("sh", "-n")
	command.Stdin = strings.NewReader(script)
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("guest init syntax: %v: %q", err, output)
	}
}

func TestSerialCaptureBoundsMemoryAndSignalsExplicitFailure(t *testing.T) {
	serial := newSerialCapture(8)
	if _, err := serial.Write([]byte("diagnostics-runaway")); err != nil {
		t.Fatal(err)
	}
	retained, total, overflow := serial.snapshot()
	if len(retained) != 8 || total != int64(len("diagnostics-runaway")) || overflow {
		t.Fatalf("retained=%d total=%d", len(retained), total)
	}
	var limitErr *sandbox.OutputLimitError
	if err := serial.limitError(); !errors.As(err, &limitErr) {
		t.Fatalf("overflow error = %v", err)
	}
	if limitErr.Limit != 8 || limitErr.Total != total || limitErr.CountOverflow {
		t.Fatalf("overflow detail = %+v", limitErr)
	}
}

func TestSerialCaptureSaturatesByteCountOnOverflow(t *testing.T) {
	serial := newSerialCapture(8)
	serial.total = math.MaxInt64 - 1
	if _, err := serial.Write([]byte("xx")); err != nil {
		t.Fatal(err)
	}
	_, total, overflow := serial.snapshot()
	if total != math.MaxInt64 || !overflow {
		t.Fatalf("serial count = %d overflow=%v", total, overflow)
	}
	var limitErr *sandbox.OutputLimitError
	if err := serial.limitError(); !errors.As(err, &limitErr) || !limitErr.CountOverflow {
		t.Fatalf("serial overflow error = %v", err)
	}
}

func TestRunnerRejectsInvalidMemoryBeforeHostPrerequisites(t *testing.T) {
	runner := &FirecrackerRunner{MemoryMB: 63}
	_, _, err := runner.Run(context.Background(), []string{"true"}, "", 0)
	if err == nil || !strings.Contains(err.Error(), "memory must be between") {
		t.Fatalf("invalid memory error = %v", err)
	}
}

func TestGuestStatusParserRequiresCanonicalTrustedRecord(t *testing.T) {
	for raw, want := range map[string]int{"exit:0\n": 0, "exit:127\n": 127, "exit:255\n": 255} {
		code, err := parseGuestStatus([]byte(raw))
		if err != nil || code != want {
			t.Fatalf("parse %q: code=%d err=%v", raw, code, err)
		}
	}
	for _, raw := range []string{"0\n", "exit:00\n", "exit:256\n", "exit:0\nexit:1\n", "exit:0"} {
		if _, err := parseGuestStatus([]byte(raw)); err == nil {
			t.Fatalf("non-canonical status %q accepted", raw)
		}
	}
	if _, err := parseGuestStatus([]byte("infra:privilege-drop\n")); err == nil ||
		!strings.Contains(err.Error(), "privilege drop failed") {
		t.Fatalf("trusted infrastructure status = %v", err)
	}
}

func TestGuestResultDirectoryIsFreshAndPathGrammarIsClosed(t *testing.T) {
	first, err := newGuestResultDir()
	if err != nil {
		t.Fatal(err)
	}
	second, err := newGuestResultDir()
	if err != nil {
		t.Fatal(err)
	}
	if first == second || !validGuestResultPath(first+"/status") ||
		!validGuestResultPath(first+"/output") {
		t.Fatalf("result directories = %q and %q", first, second)
	}
	for _, path := range []string{
		"/forge-result/status",
		first + "/status extra",
		first + "/unknown",
	} {
		if validGuestResultPath(path) {
			t.Fatalf("unsafe result path %q accepted", path)
		}
	}
}

func TestResultFilePreservesFormerSentinelsAsOrdinaryBytes(t *testing.T) {
	path := filepath.Join(t.TempDir(), "result")
	want := "FORGE-GUEST-DONE\n0\nLEFT] RIGHT\nFORGE-GUEST-START\x00"
	if err := os.WriteFile(path, []byte(want), 0o600); err != nil {
		t.Fatal(err)
	}
	got, err := readBoundedRegularFile(path, len(want))
	if err != nil || string(got) != want {
		t.Fatalf("result read = %q, err=%v", got, err)
	}
}

func TestResultFileLimitFailsBeforeUnboundedRead(t *testing.T) {
	path := filepath.Join(t.TempDir(), "result")
	if err := os.WriteFile(path, []byte("12345"), 0o600); err != nil {
		t.Fatal(err)
	}
	_, err := readBoundedRegularFile(path, 4)
	var limitErr *sandbox.OutputLimitError
	if !errors.As(err, &limitErr) || limitErr.Limit != 4 || limitErr.Total != 5 {
		t.Fatalf("bounded result error = %v", err)
	}
}

// TestFirecrackerRunnerLiveMicroVM boots a real KVM microVM when
// FORGE_FIRECRACKER_KERNEL and FORGE_FIRECRACKER_ROOTFS point at a vmlinux
// and ext4 rootfs template (see docs/external-resource-verification.md). It
// is skipped otherwise — CI exercises the deterministic boundary tests above;
// this proves privilege drop plus post-shutdown result read-back on a host.
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
		"",
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

// TestFirecrackerRunnerLiveMicroVMStdin proves the prompt delivery path in
// a real microVM: /forge-stdin is injected into the rootfs and the guest
// redirects it to the command's stdin (review F1 verification).
func TestFirecrackerRunnerLiveMicroVMStdin(t *testing.T) {
	kernel := os.Getenv("FORGE_FIRECRACKER_KERNEL")
	rootdir := os.Getenv("FORGE_FIRECRACKER_ROOTDIR")
	if kernel == "" || rootdir == "" {
		t.Skip("FORGE_FIRECRACKER_KERNEL/FORGE_FIRECRACKER_ROOTDIR unset; skipping live microVM test")
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
		[]string{"/bin/cat"},
		"FORGELIVE-VM-STDIN-PROMPT",
		120*time.Second,
	)
	if err != nil {
		t.Fatalf("live microVM stdin run: %v", err)
	}
	if code != 0 {
		t.Fatalf("guest exit = %d, want 0", code)
	}
	if !strings.Contains(output, "FORGELIVE-VM-STDIN-PROMPT") {
		t.Fatalf("guest output missing prompt echo: %q", output)
	}
	t.Logf("live microVM stdin verified: %q", output)
}
