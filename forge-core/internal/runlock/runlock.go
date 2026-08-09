// Package runlock provides two small, independent facilities used by Forge
// repository-mutating commands:
//
//   - Acquire/Release: a single-host advisory lock on <root>/.forge/run.lock
//     that makes a competing command fail FAST — never block, retry, or wait —
//     so run, evolve, and migration transactions cannot race on repository or
//     .forge/ state.
//   - NewRunID: a process-scoped, roughly time-ordered id for trace
//     correlation, independent of the lock (see runid.go).
//
// LOCAL-FILESYSTEM ONLY: the lock is implemented with flock(2) (unix build
// tag; see lock_unix.go), which is a real cross-process mutex on local
// filesystems but is well known to be unreliable-to-absent on some NFS
// configurations. ForgeOS's own non-goals scope to single-host/single-
// checkout operation, so this is an accepted, honestly-documented
// limitation, not a defect — a future reader should not be surprised if
// .forge/ ever ends up on a network mount.
package runlock

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"time"

	"forgeos/forge-core/internal/statefs"
)

// errLockHeld is the sentinel tryLock (lock_unix.go/lock_other.go) returns
// when the lock is already held by another open file description. Acquire
// wraps it into an actionable, path-naming error; it is unexported because
// callers only ever need Acquire's wrapped message, never to type-switch on
// this directly.
var errLockHeld = errors.New("runlock: lock already held")

// Lock is a held advisory lock on <root>/.forge/run.lock. The zero value is
// not meaningful; obtain one via Acquire.
type Lock struct {
	f    *os.File
	Path string
}

// Acquire claims the exclusive, NON-BLOCKING advisory lock on
// <root>/.forge/run.lock. It never blocks, retries, or waits: if another
// forge process already holds the lock, it returns immediately with an
// actionable error naming the lock path. A contended inode always has a live
// holder: flock is released automatically on process exit, including SIGKILL.
// The path must not be unlinked while contended because doing so would create a
// second lock namespace and permit split-brain mutation.
//
// It creates <root>/.forge if missing (MkdirAll), mirroring the trace/
// checkpoint writers' own directory-creation behavior, so Acquire can be
// the very first .forge/ touch in a run without requiring the directory to
// already exist.
//
// Callers must defer Release() on success.
func Acquire(root string) (*Lock, error) {
	dir := filepath.Join(root, ".forge")
	if err := statefs.EnsurePrivateDir(dir); err != nil {
		return nil, fmt.Errorf("runlock: secure %s: %w", dir, err)
	}
	path := filepath.Join(dir, "run.lock")
	f, err := statefs.OpenRegular(path, os.O_RDWR|os.O_CREATE, 0o600)
	if err != nil {
		return nil, fmt.Errorf("runlock: open %s: %w", path, err)
	}
	if err := tryLock(f); err != nil {
		_ = f.Close()
		if errors.Is(err, errLockHeld) {
			return nil, fmt.Errorf(
				"runlock: %s is already active — another Forge mutation holds it; "+
					"wait for that command to finish or terminate its verified holder; "+
					"do not unlink a contended lock file", path)
		}
		return nil, fmt.Errorf("runlock: lock %s: %w", path, err)
	}
	stampLockMeta(f) // best-effort; never fails Acquire
	return &Lock{f: f, Path: path}, nil
}

// Busy performs a read-only, non-creating contention probe. A missing .forge
// directory or run.lock means not busy. An existing lock file is opened without
// O_CREATE and briefly locked; success is immediately released, while a held
// flock reports busy. On platforms without a real lock implementation the probe
// honestly reports not busy so dry-run callers remain available.
func Busy(root string) (bool, error) {
	if !Supported() {
		return false, nil
	}
	dir := filepath.Join(root, ".forge")
	if _, present, err := statefs.InspectDir(dir); err != nil || !present {
		return false, err
	}
	path := filepath.Join(dir, "run.lock")
	if _, present, err := statefs.InspectRegular(path); err != nil || !present {
		return false, err
	}
	file, err := statefs.OpenRegularReadOnly(path)
	if err != nil {
		return false, fmt.Errorf("runlock: probe %s: %w", path, err)
	}
	defer func() { _ = file.Close() }()
	if err := tryLock(file); err != nil {
		if errors.Is(err, errLockHeld) {
			return true, nil
		}
		return false, fmt.Errorf("runlock: probe lock %s: %w", path, err)
	}
	_ = unlock(file)
	return false, nil
}

// Release releases the lock and closes the underlying file descriptor.
// Nil-safe and idempotent: calling it on a nil *Lock, or calling it twice,
// never panics or returns an error — a drop-in `defer lock.Release()` needs
// no extra guarding at the call site.
func (l *Lock) Release() error {
	if l == nil || l.f == nil {
		return nil
	}
	f := l.f
	l.f = nil
	// Explicit unlock is best-effort and, strictly, redundant: Close alone
	// already releases the flock (it is tied to the open file description,
	// not the fd number), but unlocking first keeps the intent readable and
	// costs nothing.
	_ = unlock(f)
	return f.Close()
}

// stampLockMeta best-effort writes "pid=%d started=%s" into the held lock
// file so a human inspecting a held run.lock can identify its likely holder.
// Purely informational — never load-bearing for correctness, which comes
// entirely from flock. Failures are silently ignored.
func stampLockMeta(f *os.File) {
	_ = f.Truncate(0)
	_, _ = f.Seek(0, 0)
	_, _ = fmt.Fprintf(f, "pid=%d started=%s\n", os.Getpid(), time.Now().UTC().Format(time.RFC3339))
}
