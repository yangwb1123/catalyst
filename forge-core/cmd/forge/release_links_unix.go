//go:build linux

package main

import (
	"crypto/sha256"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"syscall"
	"unsafe"
)

func releaseRegularSingleLink(info os.FileInfo) bool {
	stat, ok := info.Sys().(*syscall.Stat_t)
	return ok && stat.Nlink == 1
}

const releasePinnedExecCommand = "__forge-release-exec-pinned"
const trustedReleaseGitPath = "/usr/bin/git"

func releasePinnedExecutionSupport() error {
	if info, err := os.Stat("/proc/self/exe"); err != nil || !info.Mode().IsRegular() {
		return fmt.Errorf("release executable pinning requires a usable /proc/self/exe")
	}
	if info, err := os.Stat("/proc/self/fd"); err != nil || !info.IsDir() {
		return fmt.Errorf("release executable pinning requires a usable /proc/self/fd")
	}
	probe, err := createReleaseMemfd()
	if err != nil {
		return fmt.Errorf("release executable pinning requires memfd sealing: %w", err)
	}
	defer probe.Close()
	if err := sealReleaseMemfd(probe); err != nil {
		return fmt.Errorf("release executable pinning requires memfd sealing: %w", err)
	}
	return nil
}

func trustedReleaseGitExecutable() (string, error) {
	components := []string{"/", "/usr", "/usr/bin", trustedReleaseGitPath}
	var hostOwner uint32
	for i, path := range components {
		info, err := os.Lstat(path)
		if err != nil {
			return "", fmt.Errorf("inspect trusted release Git path %q: %w", path, err)
		}
		stat, ok := info.Sys().(*syscall.Stat_t)
		if !ok {
			return "", fmt.Errorf("trusted release Git path %q has unverifiable ownership", path)
		}
		if i == 0 {
			hostOwner = stat.Uid
		}
		if stat.Uid != hostOwner || info.Mode()&os.ModeSymlink != 0 ||
			releaseGitComponentMutable(stat.Uid, info.Mode().Perm(), os.Geteuid()) {
			return "", fmt.Errorf("trusted release Git path %q is mutable by the invoking user or has inconsistent ownership", path)
		}
		if i < len(components)-1 && !info.IsDir() {
			return "", fmt.Errorf("trusted release Git path component %q is not a directory", path)
		}
		if i == len(components)-1 &&
			(!info.Mode().IsRegular() || info.Mode().Perm()&0o111 == 0) {
			return "", fmt.Errorf("trusted release Git %q is not a regular executable", path)
		}
		if i == len(components)-1 {
			if err := validateReleaseGitPrivilegedMetadata(path, info.Mode(), syscall.Getxattr); err != nil {
				return "", err
			}
		}
	}
	return trustedReleaseGitPath, nil
}

func releaseGitComponentMutable(owner uint32, permissions os.FileMode, euid int) bool {
	return permissions&0o022 != 0 || (euid != 0 && owner == uint32(euid))
}

func verifyForgeGitRoot(root string) (bool, error) {
	absoluteRoot, err := filepath.Abs(root)
	if err != nil {
		return false, fmt.Errorf("resolve repository root: %w", err)
	}
	resolvedRoot, err := filepath.EvalSymlinks(absoluteRoot)
	if err != nil {
		return false, fmt.Errorf("resolve repository root: %w", err)
	}
	controlRoot, found, err := nearestGitControlRoot(resolvedRoot)
	if err != nil || !found {
		return false, err
	}
	if controlRoot != resolvedRoot {
		return false, fmt.Errorf(
			"repository root %q is nested under Git worktree %q; use the Git toplevel",
			resolvedRoot, controlRoot,
		)
	}
	topRaw, err := gitOutput(root, "rev-parse", "--show-toplevel")
	if err != nil {
		return false, fmt.Errorf("resolve Git worktree toplevel: %w", err)
	}
	top := strings.TrimSpace(string(topRaw))
	if top == "" {
		return false, fmt.Errorf("Git returned an empty worktree toplevel")
	}
	rootInfo, rootErr := os.Stat(resolvedRoot)
	topInfo, topErr := os.Stat(top)
	if rootErr != nil || topErr != nil || !os.SameFile(rootInfo, topInfo) {
		return false, fmt.Errorf("repository root %q does not match Git toplevel %q", resolvedRoot, top)
	}
	return true, nil
}

func nearestGitControlRoot(root string) (string, bool, error) {
	for cursor := filepath.Clean(root); ; cursor = filepath.Dir(cursor) {
		control := filepath.Join(cursor, ".git")
		info, err := os.Lstat(control)
		if err == nil {
			valid := validGitControl(control, info)
			if cursor == root && !valid {
				return "", false, fmt.Errorf("Git control path %q is not a valid real directory or gitfile", control)
			}
			if valid {
				return cursor, true, nil
			}
			continue
		}
		if !os.IsNotExist(err) {
			return "", false, fmt.Errorf("inspect Git control path %q: %w", control, err)
		}
		parent := filepath.Dir(cursor)
		if parent == cursor {
			return "", false, nil
		}
	}
}

func validGitControl(control string, info os.FileInfo) bool {
	if info.IsDir() {
		return validGitDir(control)
	}
	if !info.Mode().IsRegular() {
		return false
	}
	raw, ok := readSmallGitFile(control)
	if !ok || !strings.HasPrefix(raw, "gitdir:") {
		return false
	}
	target := strings.TrimSpace(strings.TrimPrefix(raw, "gitdir:"))
	if target == "" {
		return false
	}
	if !filepath.IsAbs(target) {
		target = filepath.Join(filepath.Dir(control), target)
	}
	target, err := filepath.EvalSymlinks(target)
	return err == nil && validGitDir(target)
}

func validGitDir(dir string) bool {
	head, ok := readSmallGitFile(filepath.Join(dir, "HEAD"))
	if !ok || head == "" {
		return false
	}
	objects, err := os.Lstat(filepath.Join(dir, "objects"))
	if err == nil && objects.IsDir() {
		return true
	}
	common, ok := readSmallGitFile(filepath.Join(dir, "commondir"))
	if !ok || common == "" {
		return false
	}
	if !filepath.IsAbs(common) {
		common = filepath.Join(dir, common)
	}
	objects, err = os.Lstat(filepath.Join(common, "objects"))
	return err == nil && objects.IsDir()
}

func readSmallGitFile(path string) (string, bool) {
	fd, err := syscall.Open(path, syscall.O_RDONLY|syscall.O_CLOEXEC|syscall.O_NOFOLLOW, 0)
	if err != nil {
		return "", false
	}
	file := os.NewFile(uintptr(fd), path)
	defer file.Close()
	info, err := file.Stat()
	if err != nil || !info.Mode().IsRegular() || !releaseRegularSingleLink(info) {
		return "", false
	}
	raw, err := io.ReadAll(io.LimitReader(file, 4097))
	if err != nil || len(raw) > 4096 {
		return "", false
	}
	return strings.TrimSpace(string(raw)), true
}

func rejectTrackedForgeControlState(root string) error {
	isGitRoot, err := verifyForgeGitRoot(root)
	if err != nil {
		return fmt.Errorf("verify Forge control-state provenance: %w", err)
	}
	if !isGitRoot {
		return nil
	}
	raw, err := gitOutput(root, "ls-files", "--cached", "-z")
	if err != nil {
		return fmt.Errorf("verify Forge control-state provenance: %w", err)
	}
	for _, path := range splitNUL(raw) {
		classified, err := classifyInventoryPath(path)
		if err != nil {
			return fmt.Errorf("verify Forge control-state provenance: %w", err)
		}
		if classified.kind == inventoryForgeControlPath {
			return fmt.Errorf("tracked Forge control state %q is forbidden", path)
		}
	}
	return nil
}

type releaseXattrGetter func(path, attr string, dest []byte) (int, error)

func validateReleaseGitPrivilegedMetadata(path string, mode os.FileMode, getxattr releaseXattrGetter) error {
	if mode&(os.ModeSetuid|os.ModeSetgid) != 0 {
		return fmt.Errorf("trusted release Git %q must not carry setuid or setgid", path)
	}
	_, err := getxattr(path, "security.capability", nil)
	if err == nil {
		return fmt.Errorf("trusted release Git %q must not carry Linux file capabilities", path)
	}
	if errors.Is(err, syscall.ENODATA) {
		return nil
	}
	return fmt.Errorf("inspect trusted release Git capabilities for %q: %w", path, err)
}

func releasePinnedLauncherArgv(path, expectedSHA256 string, argv []string) []string {
	if len(argv) == 0 {
		return nil
	}
	wrapped := []string{
		"/proc/self/exe", releasePinnedExecCommand,
		path, expectedSHA256, "--",
	}
	return append(wrapped, argv[1:]...)
}

func cmdReleaseExecPinned(args []string) int {
	if err := executePinnedReleaseAgent(args); err != nil {
		fmt.Fprintf(os.Stderr, "forge: pinned release agent: %v\n", err)
		return 126
	}
	return 0
}

func executePinnedReleaseAgent(args []string) error {
	path, expectedSHA256, agentArgs, err := parsePinnedReleaseArgs(args)
	if err != nil {
		return err
	}
	pinned, err := preparePinnedReleaseExecutable(path, expectedSHA256)
	if err != nil {
		return err
	}
	defer pinned.Close()
	if err := inheritPinnedFD(pinned); err != nil {
		return err
	}
	fdPath := fmt.Sprintf("/proc/self/fd/%d", pinned.Fd())
	argv := append([]string{path}, agentArgs...)
	err = syscall.Exec(fdPath, argv, os.Environ())
	runtime.KeepAlive(pinned)
	return fmt.Errorf("execute verified anonymous release agent: %w", err)
}

// preparePinnedReleaseExecutable copies, seals, then verifies the final FD.
func preparePinnedReleaseExecutable(path, expectedSHA256 string) (*os.File, error) {
	source, err := openPinnedReleaseSource(path)
	if err != nil {
		return nil, err
	}
	defer source.Close()
	pinned, err := createReleaseMemfd()
	if err != nil {
		return nil, err
	}
	keep := false
	defer func() {
		if !keep {
			pinned.Close()
		}
	}()
	if err := populateAndSealReleaseMemfd(pinned, source); err != nil {
		return nil, err
	}
	if err := validatePinnedReleaseDigest(pinned, expectedSHA256); err != nil {
		return nil, err
	}
	if err := validatePinnedNativeFD(pinned); err != nil {
		return nil, err
	}
	readOnly, err := reopenSealedReleaseMemfd(pinned)
	if err != nil {
		return nil, err
	}
	if err := pinned.Close(); err != nil {
		readOnly.Close()
		return nil, fmt.Errorf("close writable release-agent memfd handle: %w", err)
	}
	pinned = readOnly
	keep = true
	return pinned, nil
}

func populateAndSealReleaseMemfd(pinned, source *os.File) error {
	if _, err := io.Copy(pinned, source); err != nil {
		return fmt.Errorf("copy trusted release agent into memfd: %w", err)
	}
	if err := pinned.Sync(); err != nil {
		return fmt.Errorf("sync trusted release-agent memfd: %w", err)
	}
	if err := syscall.Fchmod(int(pinned.Fd()), 0o500); err != nil {
		return fmt.Errorf("mark trusted release-agent memfd executable: %w", err)
	}
	if err := sealReleaseMemfd(pinned); err != nil {
		return err
	}
	return nil
}

const (
	mfdCloexec      = 0x0001
	mfdAllowSealing = 0x0002
	mfdExec         = 0x0010
	fAddSeals       = 1033
	fGetSeals       = 1034
	fSealSeal       = 0x0001
	fSealShrink     = 0x0002
	fSealGrow       = 0x0004
	fSealWrite      = 0x0008
	fSealExec       = 0x0020
	releaseSealMask = fSealSeal | fSealShrink | fSealGrow | fSealWrite | fSealExec
)

func createReleaseMemfd() (*os.File, error) {
	number, err := memfdCreateSyscall()
	if err != nil {
		return nil, err
	}
	name, err := syscall.BytePtrFromString("forge-release-agent")
	if err != nil {
		return nil, fmt.Errorf("encode release-agent memfd name: %w", err)
	}
	fd, _, errno := syscall.Syscall(
		number, uintptr(unsafe.Pointer(name)), mfdCloexec|mfdAllowSealing|mfdExec, 0,
	)
	runtime.KeepAlive(name)
	if errno != 0 {
		return nil, fmt.Errorf("create release-agent memfd: %w", errno)
	}
	file := os.NewFile(fd, "forge-release-agent")
	if file == nil {
		syscall.Close(int(fd))
		return nil, fmt.Errorf("create release-agent memfd file handle")
	}
	return file, nil
}

func memfdCreateSyscall() (uintptr, error) {
	switch runtime.GOARCH {
	case "amd64":
		return 319, nil
	case "386":
		return 356, nil
	case "arm":
		return 385, nil
	case "arm64", "loong64", "riscv64":
		return 279, nil
	case "mips", "mipsle":
		return 4354, nil
	case "mips64", "mips64le":
		return 5314, nil
	case "ppc64", "ppc64le":
		return 360, nil
	case "s390x":
		return 350, nil
	default:
		return 0, fmt.Errorf("unsupported Linux architecture %q for memfd pinning", runtime.GOARCH)
	}
}

func sealReleaseMemfd(file *os.File) error {
	_, _, errno := syscall.Syscall(
		syscall.SYS_FCNTL, file.Fd(), fAddSeals, releaseSealMask,
	)
	if errno != 0 {
		return fmt.Errorf("seal release-agent memfd: %w", errno)
	}
	seals, err := releaseMemfdSeals(file)
	if err != nil {
		return err
	}
	if seals&releaseSealMask != releaseSealMask {
		return fmt.Errorf("release-agent memfd seal verification failed: got %#x", seals)
	}
	return nil
}

func releaseMemfdSeals(file *os.File) (int, error) {
	seals, _, errno := syscall.Syscall(syscall.SYS_FCNTL, file.Fd(), fGetSeals, 0)
	if errno != 0 {
		return 0, fmt.Errorf("inspect release-agent memfd seals: %w", errno)
	}
	return int(seals), nil
}

func reopenSealedReleaseMemfd(file *os.File) (*os.File, error) {
	path := fmt.Sprintf("/proc/self/fd/%d", file.Fd())
	readOnly, err := os.Open(path)
	if err != nil {
		return nil, fmt.Errorf("reopen sealed release-agent memfd read-only: %w", err)
	}
	before, beforeErr := file.Stat()
	after, afterErr := readOnly.Stat()
	if beforeErr != nil || afterErr != nil || !os.SameFile(before, after) {
		readOnly.Close()
		return nil, fmt.Errorf("sealed release-agent memfd identity changed while reopening")
	}
	seals, err := releaseMemfdSeals(readOnly)
	if err != nil || seals&releaseSealMask != releaseSealMask {
		readOnly.Close()
		return nil, fmt.Errorf("reopened release-agent memfd lost required seals")
	}
	return readOnly, nil
}

func validatePinnedReleaseDigest(pinned *os.File, expectedSHA256 string) error {
	info, err := pinned.Stat()
	if err != nil || !info.Mode().IsRegular() {
		return fmt.Errorf("inspect sealed release-agent memfd: %w", err)
	}
	hash := sha256.New()
	if _, err := io.Copy(hash, io.NewSectionReader(pinned, 0, info.Size())); err != nil {
		return fmt.Errorf("hash sealed release-agent memfd: %w", err)
	}
	if fmt.Sprintf("%x", hash.Sum(nil)) != expectedSHA256 {
		return fmt.Errorf("trusted claude executable bytes changed before pinned execution")
	}
	return nil
}

func validatePinnedNativeFD(pinned *os.File) error {
	var header [4]byte
	n, err := pinned.ReadAt(header[:], 0)
	if err != nil && err != io.EOF {
		return fmt.Errorf("inspect pinned release-agent bytes: %w", err)
	}
	if n != len(header) || header != [4]byte{0x7f, 'E', 'L', 'F'} {
		return fmt.Errorf("trusted claude executable must be a native Linux ELF; scripts and binfmt payloads are not pinned")
	}
	return nil
}

func openPinnedReleaseSource(path string) (*os.File, error) {
	before, err := os.Lstat(path)
	if err != nil {
		return nil, fmt.Errorf("inspect trusted claude executable: %w", err)
	}
	if before.Mode()&os.ModeSymlink != 0 || !before.Mode().IsRegular() ||
		before.Mode().Perm()&0o111 == 0 {
		return nil, fmt.Errorf("trusted claude path is no longer a real executable file")
	}
	source, err := os.Open(path)
	if err != nil {
		return nil, fmt.Errorf("open trusted claude executable: %w", err)
	}
	opened, err := source.Stat()
	if err != nil || !os.SameFile(before, opened) ||
		!opened.Mode().IsRegular() || opened.Mode().Perm()&0o111 == 0 {
		source.Close()
		return nil, fmt.Errorf("trusted claude executable changed while opening")
	}
	return source, nil
}

func inheritPinnedFD(file *os.File) error {
	_, _, errno := syscall.Syscall(
		syscall.SYS_FCNTL, file.Fd(), uintptr(syscall.F_SETFD), 0,
	)
	if errno != 0 {
		return fmt.Errorf("preserve pinned executable FD across exec: %w", errno)
	}
	return nil
}
