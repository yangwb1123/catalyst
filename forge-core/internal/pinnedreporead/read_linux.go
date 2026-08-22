//go:build linux && (amd64 || arm64)

package pinnedreporead

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"io"
	"os"
	"syscall"
	"unsafe"
)

const (
	sysOpenat2          = 437
	resolveNoXDev       = 0x01
	resolveNoMagicLinks = 0x02
	resolveNoSymlinks   = 0x04
	resolveBeneath      = 0x08
	fsExt               = int64(0xEF53)
	fsXFS               = int64(0x58465342)
	fsBtrfs             = int64(0x9123683E)
	fsTmpfs             = int64(0x01021994)
	fsOverlay           = int64(0x794C7630)
	fsZFS               = int64(0x2FC12FC1)
	openPathOnly        = 0x200000
)

type openHow struct {
	flags   uint64
	mode    uint64
	resolve uint64
}

func checkPlatform() error {
	return nil
}

func preflightRepository(repository *os.File) error {
	_, err := validateRepository(context.Background(), repository)
	return withCode(CodeReadFailed, err)
}

func readPlatform(
	ctx context.Context,
	repository *os.File,
	entries []ExpectedEntry,
	limits Limits,
) ([]File, error) {
	if err := cooperativeDeadline(ctx); err != nil {
		return nil, err
	}
	rootInfo, err := validateRepository(ctx, repository)
	if err != nil {
		return nil, preferDeadline(ctx, withCode(CodeReadFailed, err))
	}
	if err := cooperativeDeadline(ctx); err != nil {
		return nil, err
	}
	result := make([]File, 0, len(entries))
	for index, entry := range entries {
		if err := ctx.Err(); err != nil {
			clearFiles(result)
			return nil, withCode(CodeTimeoutExceeded,
				fmt.Errorf("repository read canceled before entry %d", index))
		}
		value, err := readEntry(ctx, repository, entry, limits.MaxFileBytes)
		if err != nil {
			clearFiles(result)
			return nil, fmt.Errorf("repository entry %d failed closed: %w", index, err)
		}
		result = append(result, value)
	}
	if err := verifyRepository(ctx, repository, rootInfo); err != nil {
		clearFiles(result)
		return nil, preferDeadline(ctx, withCode(CodeIdentityChanged, err))
	}
	if err := cooperativeDeadline(ctx); err != nil {
		clearFiles(result)
		return nil, err
	}
	cloned := cloneFiles(result)
	clearFiles(result)
	return cloned, nil
}

func validateRepository(ctx context.Context, repository *os.File) (os.FileInfo, error) {
	if err := cooperativeDeadline(ctx); err != nil {
		return nil, err
	}
	if repository == nil {
		return nil, fmt.Errorf("bound repository handle is unavailable")
	}
	info, err := repository.Stat()
	if err != nil || !info.IsDir() {
		return nil, preferDeadline(ctx,
			fmt.Errorf("bound repository handle is not a directory"))
	}
	if err := cooperativeDeadline(ctx); err != nil {
		return nil, err
	}
	if err := requireLocalFilesystem(ctx, repository); err != nil {
		return nil, err
	}
	if err := cooperativeDeadline(ctx); err != nil {
		return nil, err
	}
	probe, err := openExact(repository, ".")
	if err == nil {
		_ = probe.Close()
	}
	if err != nil {
		return nil, preferDeadline(ctx,
			fmt.Errorf("openat2 is unavailable for the bound repository"))
	}
	if err := cooperativeDeadline(ctx); err != nil {
		return nil, err
	}
	return info, nil
}

func requireLocalFilesystem(ctx context.Context, repository *os.File) error {
	if err := cooperativeDeadline(ctx); err != nil {
		return err
	}
	var facts syscall.Statfs_t
	if err := syscall.Fstatfs(int(repository.Fd()), &facts); err != nil {
		return preferDeadline(ctx, fmt.Errorf("inspect repository filesystem: %w", err))
	}
	if err := cooperativeDeadline(ctx); err != nil {
		return err
	}
	if !allowedFilesystem(int64(facts.Type)) {
		return fmt.Errorf("repository filesystem is outside the local v1 allowlist")
	}
	return nil
}

func allowedFilesystem(magic int64) bool {
	switch magic {
	case fsExt, fsXFS, fsBtrfs, fsTmpfs, fsOverlay, fsZFS:
		return true
	default:
		return false
	}
}

func readEntry(
	ctx context.Context,
	repository *os.File,
	entry ExpectedEntry,
	max int64,
) (File, error) {
	return readEntryWith(ctx, repository, entry, max, nil)
}

func readEntryWith(
	ctx context.Context,
	repository *os.File,
	entry ExpectedEntry,
	max int64,
	afterContent func(),
) (File, error) {
	if err := cooperativeDeadline(ctx); err != nil {
		return File{}, err
	}
	file, before, err := openVerifiedLeaf(ctx, repository, entry)
	if err != nil {
		return File{}, preferDeadline(ctx, withCode(CodeReadFailed, err))
	}
	defer func() { _ = file.Close() }()
	if err := cooperativeDeadline(ctx); err != nil {
		return File{}, err
	}
	content, err := readContent(ctx, file, entry.Bytes, max)
	if err != nil {
		return File{}, err
	}
	if afterContent != nil {
		afterContent()
	}
	if err := cooperativeDeadline(ctx); err != nil {
		clearBytes(content)
		return File{}, err
	}
	if err := verifyRead(ctx, repository, file, before, entry, content); err != nil {
		clearBytes(content)
		return File{}, err
	}
	return File{Content: content, ContentSHA256: entry.ContentSHA256, Path: entry.Path}, nil
}

type activeLeafOpener func(*os.File, string) (*os.File, error)

func openVerifiedLeaf(ctx context.Context, repository *os.File,
	entry ExpectedEntry) (*os.File, os.FileInfo, error) {
	return openVerifiedLeafWith(ctx, repository, entry, openLeafExact, nil)
}

func openVerifiedLeafWith(ctx context.Context, repository *os.File, entry ExpectedEntry,
	openActive activeLeafOpener, afterPathCheck func()) (*os.File, os.FileInfo, error) {
	if err := cooperativeDeadline(ctx); err != nil {
		return nil, nil, err
	}
	pathOnly, err := openPathExact(repository, entry.Path)
	if err != nil {
		return nil, nil, preferDeadline(ctx, err)
	}
	defer func() { _ = pathOnly.Close() }()
	if err = cooperativeDeadline(ctx); err != nil {
		return nil, nil, err
	}
	pathInfo, err := validateRegular(pathOnly, entry)
	if err != nil {
		return nil, nil, preferDeadline(ctx, err)
	}
	if afterPathCheck != nil {
		afterPathCheck()
	}
	if err = cooperativeDeadline(ctx); err != nil {
		return nil, nil, err
	}
	active, err := openActive(repository, entry.Path)
	if err != nil {
		return nil, nil, preferDeadline(ctx, err)
	}
	if err = cooperativeDeadline(ctx); err != nil {
		_ = active.Close()
		return nil, nil, err
	}
	activeInfo, err := validateRegular(active, entry)
	if err != nil || !os.SameFile(pathInfo, activeInfo) {
		_ = active.Close()
		return nil, nil, fmt.Errorf("regular file changed between path and active open")
	}
	if err = cooperativeDeadline(ctx); err != nil {
		_ = active.Close()
		return nil, nil, err
	}
	return active, activeInfo, nil
}

func validateRegular(file *os.File, entry ExpectedEntry) (os.FileInfo, error) {
	info, err := file.Stat()
	if err != nil || !info.Mode().IsRegular() || info.Size() != entry.Bytes {
		return nil, fmt.Errorf("opened object is not the expected regular file")
	}
	stat, ok := info.Sys().(*syscall.Stat_t)
	if !ok || stat.Nlink != 1 {
		return nil, fmt.Errorf("regular file must have exactly one link")
	}
	return info, nil
}

func readContent(ctx context.Context, file *os.File, expected, max int64) ([]byte, error) {
	limit := expected
	if max < limit {
		limit = max
	}
	buffer := make([]byte, 0, limit)
	chunk := make([]byte, 32*1024)
	defer clearBytes(chunk)
	for int64(len(buffer)) <= limit {
		if err := ctx.Err(); err != nil {
			clearBytes(buffer)
			return nil, withCode(CodeTimeoutExceeded,
				fmt.Errorf("cooperative read deadline elapsed"))
		}
		count, err := file.Read(chunk)
		if count > 0 {
			buffer = append(buffer, chunk[:count]...)
		}
		if ctxErr := ctx.Err(); ctxErr != nil {
			clearBytes(buffer)
			return nil, withCode(CodeTimeoutExceeded,
				fmt.Errorf("cooperative read deadline elapsed after syscall"))
		}
		if int64(len(buffer)) > limit {
			clearBytes(buffer)
			return nil, withCode(CodeContentMismatch,
				fmt.Errorf("regular content exceeds expected bytes"))
		}
		if err == io.EOF {
			break
		}
		if err != nil {
			clearBytes(buffer)
			return nil, withCode(CodeReadFailed, fmt.Errorf("read regular content: %w", err))
		}
		if count == 0 {
			clearBytes(buffer)
			return nil, withCode(CodeReadFailed,
				fmt.Errorf("regular content read made no progress"))
		}
	}
	if int64(len(buffer)) != expected {
		clearBytes(buffer)
		return nil, withCode(CodeContentMismatch,
			fmt.Errorf("regular content byte count drifted"))
	}
	return buffer, nil
}

func verifyRead(
	ctx context.Context,
	repository *os.File,
	file *os.File,
	before os.FileInfo,
	entry ExpectedEntry,
	content []byte,
) error {
	if err := cooperativeDeadline(ctx); err != nil {
		return err
	}
	after, err := file.Stat()
	if err != nil || !os.SameFile(before, after) || after.Size() != entry.Bytes {
		return preferDeadline(ctx, withCode(CodeIdentityChanged,
			fmt.Errorf("regular file identity changed while reading")))
	}
	if err := cooperativeDeadline(ctx); err != nil {
		return err
	}
	digest := sha256.Sum256(content)
	if err := cooperativeDeadline(ctx); err != nil {
		return err
	}
	if hex.EncodeToString(digest[:]) != entry.ContentSHA256 {
		return withCode(CodeContentMismatch,
			fmt.Errorf("regular file content differs from expected manifest"))
	}
	reopened, current, err := openVerifiedLeaf(ctx, repository, entry)
	if err != nil {
		return preferDeadline(ctx, withCode(CodeIdentityChanged,
			fmt.Errorf("reopen regular file after read: %w", err)))
	}
	defer func() { _ = reopened.Close() }()
	if err := cooperativeDeadline(ctx); err != nil {
		return err
	}
	if !os.SameFile(before, current) {
		return preferDeadline(ctx, withCode(CodeIdentityChanged,
			fmt.Errorf("named regular file changed after read")))
	}
	return cooperativeDeadline(ctx)
}

func cooperativeDeadline(ctx context.Context) error {
	if err := ctx.Err(); err != nil {
		return withCode(CodeTimeoutExceeded, fmt.Errorf("cooperative read deadline elapsed"))
	}
	return nil
}

func preferDeadline(ctx context.Context, fallback error) error {
	if err := cooperativeDeadline(ctx); err != nil {
		return err
	}
	return fallback
}

func verifyRepository(ctx context.Context, repository *os.File, expected os.FileInfo) error {
	if err := cooperativeDeadline(ctx); err != nil {
		return err
	}
	current, err := repository.Stat()
	if err != nil || !current.IsDir() || !os.SameFile(expected, current) {
		return preferDeadline(ctx, fmt.Errorf("bound repository identity changed during read"))
	}
	if err := cooperativeDeadline(ctx); err != nil {
		return err
	}
	return requireLocalFilesystem(ctx, repository)
}

func openExact(repository *os.File, path string) (*os.File, error) {
	return openExactWith(repository, path,
		syscall.O_RDONLY|syscall.O_NONBLOCK|syscall.O_CLOEXEC)
}

func openLeafExact(repository *os.File, path string) (*os.File, error) {
	return openExactWith(repository, path, syscall.O_RDONLY|syscall.O_NONBLOCK|
		syscall.O_CLOEXEC|syscall.O_NOATIME|syscall.O_NOCTTY)
}

func openPathExact(repository *os.File, path string) (*os.File, error) {
	return openExactWith(repository, path, openPathOnly|syscall.O_CLOEXEC)
}

func openExactWith(repository *os.File, path string, flags int) (*os.File, error) {
	name, err := syscall.BytePtrFromString(path)
	if err != nil {
		return nil, err
	}
	how := openHow{
		flags:   uint64(flags),
		resolve: resolveBeneath | resolveNoSymlinks | resolveNoMagicLinks | resolveNoXDev,
	}
	fd, _, errno := syscall.Syscall6(sysOpenat2, repository.Fd(),
		uintptr(unsafe.Pointer(name)), uintptr(unsafe.Pointer(&how)), unsafe.Sizeof(how), 0, 0)
	if errno != 0 {
		return nil, errno
	}
	file := os.NewFile(fd, "pinned-repository-object")
	if file == nil {
		_ = syscall.Close(int(fd))
		return nil, fmt.Errorf("construct opened repository object")
	}
	return file, nil
}
