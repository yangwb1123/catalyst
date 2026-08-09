//go:build linux

package main

import (
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"syscall"
	"testing"
)

func TestReleasePinnedLauncherExecutesVerifiedNativeFromOpenFD(t *testing.T) {
	path := filepath.Join(t.TempDir(), "claude")
	writeNativeCommandAt(t, path, "echo", 0o700)
	argv := releasePinnedLauncherArgv(
		path, mustFileSHA256(t, path), []string{path, "sentinel"},
	)
	out, err := exec.Command(argv[0], argv[1:]...).CombinedOutput()
	if err != nil {
		t.Fatalf("pinned launcher: %v\n%s", err, out)
	}
	if got := string(out); got != "sentinel\n" {
		t.Fatalf("pinned launcher output = %q", got)
	}
}

func TestBindReleaseAgentRejectsEveryNonELFExecutable(t *testing.T) {
	for _, payload := range []string{"#!/usr/bin/env sh\nexit 0\n", "#!/bin/sh\nexit 0\n", "binfmt payload\n"} {
		path := filepath.Join(t.TempDir(), "claude")
		if err := os.WriteFile(path, []byte(payload), 0o700); err != nil {
			t.Fatal(err)
		}
		binding := bindReleaseAgent(t.TempDir(), "claude", path, mustFileSHA256(t, path))
		if binding.err == nil || !strings.Contains(binding.err.Error(), "native Linux ELF") {
			t.Fatalf("payload %q binding error = %v", payload, binding.err)
		}
	}
}

func TestTrustedReleaseGitRejectsInvokerOwnedReadOnlyComponent(t *testing.T) {
	const owner = uint32(4242)
	if !releaseGitComponentMutable(owner, 0o555, int(owner)) {
		t.Fatal("invoker-owned 0555 component can be chmodded and must be untrusted")
	}
	if releaseGitComponentMutable(owner, 0o555, 0) {
		t.Fatal("root is the explicit host TCB and must not be rejected as an ordinary invoker")
	}
	if releaseGitComponentMutable(owner, 0o555, 4343) {
		t.Fatal("immutable component owned by another host principal was rejected")
	}
	if !releaseGitComponentMutable(owner, 0o575, 4343) {
		t.Fatal("group-writable component must be untrusted regardless of ownership")
	}
}

func TestTrustedReleaseGitRejectsPrivilegedMetadata(t *testing.T) {
	noCapabilities := func(_, _ string, _ []byte) (int, error) {
		return 0, syscall.ENODATA
	}
	for _, mode := range []os.FileMode{os.ModeSetuid | 0o755, os.ModeSetgid | 0o755} {
		if err := validateReleaseGitPrivilegedMetadata("/usr/bin/git", mode, noCapabilities); err == nil {
			t.Fatalf("privileged mode %v was trusted", mode)
		}
	}
	hasCapabilities := func(_, _ string, _ []byte) (int, error) { return 20, nil }
	if err := validateReleaseGitPrivilegedMetadata("/usr/bin/git", 0o755, hasCapabilities); err == nil {
		t.Fatal("security.capability xattr was trusted")
	}
	unverifiable := func(_, _ string, _ []byte) (int, error) { return 0, syscall.EPERM }
	if err := validateReleaseGitPrivilegedMetadata("/usr/bin/git", 0o755, unverifiable); err == nil {
		t.Fatal("unverifiable capability metadata did not fail closed")
	}
}

func TestTrustedReleaseGitRejectsSetuidFile(t *testing.T) {
	path := filepath.Join(t.TempDir(), "git")
	if err := os.WriteFile(path, []byte("fixture"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(path, 0o4755); err != nil {
		t.Skipf("setuid fixture unavailable: %v", err)
	}
	info, err := os.Lstat(path)
	if err != nil {
		t.Fatal(err)
	}
	if info.Mode()&os.ModeSetuid == 0 {
		t.Skip("filesystem cleared the setuid fixture bit")
	}
	if err := validateReleaseGitPrivilegedMetadata(path, info.Mode(), syscall.Getxattr); err == nil {
		t.Fatal("setuid Git fixture was trusted")
	}
}

func TestReleaseMemfdSealsBlockExistingWritableAlias(t *testing.T) {
	memfd, err := createReleaseMemfd()
	if err != nil {
		t.Fatal(err)
	}
	defer func() {
		if err := memfd.Close(); err != nil {
			t.Errorf("close memfd: %v", err)
		}
	}()
	if _, err := memfd.Write([]byte{0x7f, 'E', 'L', 'F'}); err != nil {
		t.Fatal(err)
	}
	alias, err := os.OpenFile("/proc/self/fd/"+fileDescriptorString(memfd), os.O_RDWR, 0)
	if err != nil {
		t.Fatal(err)
	}
	defer func() {
		if err := alias.Close(); err != nil {
			t.Errorf("close alias: %v", err)
		}
	}()
	if err := sealReleaseMemfd(memfd); err != nil {
		t.Fatal(err)
	}
	if _, err := alias.WriteAt([]byte{0}, 0); err == nil {
		t.Fatal("pre-existing writable alias bypassed F_SEAL_WRITE")
	}
	if err := alias.Truncate(0); err == nil {
		t.Fatal("pre-existing writable alias bypassed grow/shrink seals")
	}
}

func TestReleasePinnedLauncherRejectsVerifiedShebangAtFinalFD(t *testing.T) {
	path := filepath.Join(t.TempDir(), "claude")
	if err := os.WriteFile(path, []byte("#!/bin/sh\nexit 0\n"), 0o700); err != nil {
		t.Fatal(err)
	}
	argv := releasePinnedLauncherArgv(path, mustFileSHA256(t, path), []string{path})
	out, err := exec.Command(argv[0], argv[1:]...).CombinedOutput()
	if err == nil || !strings.Contains(string(out), "native Linux ELF") {
		t.Fatalf("verified shebang final-FD error = %v, output=%s", err, out)
	}
}

func TestReleasePinnedLauncherRejectsVerifiedBinfmtPayloadAtFinalFD(t *testing.T) {
	path := filepath.Join(t.TempDir(), "claude")
	if err := os.WriteFile(path, []byte("registered binfmt payload\n"), 0o700); err != nil {
		t.Fatal(err)
	}
	argv := releasePinnedLauncherArgv(path, mustFileSHA256(t, path), []string{path})
	out, err := exec.Command(argv[0], argv[1:]...).CombinedOutput()
	if err == nil || !strings.Contains(string(out), "native Linux ELF") {
		t.Fatalf("verified binfmt final-FD error = %v, output=%s", err, out)
	}
}

func TestSourceStateRevisionDisablesRepositoryFSMonitorAndAmbientGit(t *testing.T) {
	root := t.TempDir()
	seedReleaseApprovalContext(t, root, "deploy")
	hookSentinel := filepath.Join(root, "fsmonitor-ran")
	hook := filepath.Join(root, "fsmonitor.sh")
	hookBody := "#!/bin/sh\n: > " + hookSentinel + "\nprintf 'token\\n'\n"
	if err := os.WriteFile(hook, []byte(hookBody), 0o700); err != nil {
		t.Fatal(err)
	}
	mustGit(t, root, "config", "core.fsmonitor", hook)

	// Prove the malicious fixture is live under ordinary repository-configured
	// Git, then clear its sentinel before exercising the restricted inventory.
	_, _ = exec.Command("/usr/bin/git", "-C", root, "status", "--porcelain").CombinedOutput()
	if _, err := os.Stat(hookSentinel); err != nil {
		t.Fatalf("fsmonitor fixture did not execute under ordinary Git: %v", err)
	}
	if err := os.Remove(hookSentinel); err != nil {
		t.Fatal(err)
	}

	shadowDir := t.TempDir()
	shadowSentinel := filepath.Join(root, "ambient-git-ran")
	shadow := filepath.Join(shadowDir, "git")
	shadowBody := "#!/bin/sh\n: > " + shadowSentinel + "\nexit 99\n"
	if err := os.WriteFile(shadow, []byte(shadowBody), 0o700); err != nil {
		t.Fatal(err)
	}
	t.Setenv("PATH", shadowDir)
	if _, err := sourceStateRevision(root); err != nil {
		t.Fatalf("restricted source inventory: %v", err)
	}
	for _, sentinel := range []string{hookSentinel, shadowSentinel} {
		if _, err := os.Stat(sentinel); !os.IsNotExist(err) {
			t.Fatalf("restricted source inventory executed %q: %v", sentinel, err)
		}
	}
}

func TestReleasePinnedLauncherExecutesCanonicalTargetBehindClaudeSymlink(t *testing.T) {
	repoRoot := t.TempDir()
	installDir := t.TempDir()
	target := filepath.Join(installDir, "claude.exe")
	writeNativeCommandAt(t, target, "echo", 0o700)
	entry := filepath.Join(installDir, "claude")
	if err := os.Symlink(target, entry); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}

	binding := bindReleaseAgent(repoRoot, "claude", entry, mustFileSHA256(t, target))
	if binding.err != nil {
		t.Fatalf("external claude symlink binding: %v", binding.err)
	}
	if binding.path != target {
		t.Fatalf("canonical binding path = %q, want %q", binding.path, target)
	}
	argv := binding.wrapArgv([]string{"claude", "sentinel"})
	out, err := exec.Command(argv[0], argv[1:]...).CombinedOutput()
	if err != nil {
		t.Fatalf("pinned canonical launcher: %v\n%s", err, out)
	}
	if got := string(out); got != "sentinel\n" {
		t.Fatalf("pinned canonical launcher output = %q", got)
	}
}

func TestReleasePinnedLauncherRejectsSwapAfterArgvConstruction(t *testing.T) {
	root := t.TempDir()
	path := filepath.Join(root, "claude")
	writeNativeCommandAt(t, path, "true", 0o700)
	argv := releasePinnedLauncherArgv(
		path, mustFileSHA256(t, path), []string{path},
	)
	writeNativeCommandAt(t, path, "false", 0o700)
	out, err := exec.Command(argv[0], argv[1:]...).CombinedOutput()
	if err == nil || !strings.Contains(string(out), "bytes changed before pinned execution") {
		t.Fatalf("swapped executable error = %v, output = %s", err, out)
	}
}

func TestPreparedReleaseExecutableIsSealedAndIndependentOfSourcePath(t *testing.T) {
	path := filepath.Join(t.TempDir(), "claude")
	writeNativeCommandAt(t, path, "true", 0o700)
	original, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	expected := mustFileSHA256(t, path)
	pinned, err := preparePinnedReleaseExecutable(path, expected)
	if err != nil {
		t.Fatal(err)
	}
	defer func() {
		if err := pinned.Close(); err != nil {
			t.Errorf("close pinned executable: %v", err)
		}
	}()
	seals, err := releaseMemfdSeals(pinned)
	if err != nil {
		t.Fatal(err)
	}
	if seals&releaseSealMask != releaseSealMask {
		t.Fatalf("pinned memfd seals=%#x, want mask %#x", seals, releaseSealMask)
	}
	if _, err := pinned.WriteAt([]byte{0}, 0); err == nil {
		t.Fatal("sealed release-agent memfd accepted a byte overwrite")
	}
	if err := pinned.Truncate(0); err == nil {
		t.Fatal("sealed release-agent memfd accepted truncation")
	}
	if err := validatePinnedReleaseDigest(pinned, expected); err != nil {
		t.Fatalf("sealed final FD digest: %v", err)
	}
	writeNativeCommandAt(t, path, "false", 0o700)
	data, err := io.ReadAll(pinned)
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != string(original) {
		t.Fatal("open pinned bytes changed with source path")
	}
	link, err := os.Readlink("/proc/self/fd/" + fileDescriptorString(pinned))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(link, "(deleted)") {
		t.Fatalf("pinned executable still has a mutable pathname: %q", link)
	}
}

func fileDescriptorString(file *os.File) string {
	return fmt.Sprintf("%d", file.Fd())
}
