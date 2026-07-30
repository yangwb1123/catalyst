//go:build !unix

package runlock

import "os"

// Supported is false when this build cannot provide a real cross-process lock.
func Supported() bool { return false }

// tryLock is an honest no-op on non-unix platforms: it never locks, so it
// always succeeds and never contends with another process. This repo runs
// unix-only in practice (CI is ubuntu-only; see command_executor_other.go
// for the same documented-gap precedent), and Windows would need a Job-
// Object dance with no stdlib analogue — left as deliberate future work
// rather than faked. Keeping the signature identical lets Acquire call it
// unconditionally with no build-tagged branching at the call site.
func tryLock(_ *os.File) error { return nil }

// unlock mirrors tryLock's no-op: nothing was locked, so there is nothing
// to release.
func unlock(_ *os.File) error { return nil }
