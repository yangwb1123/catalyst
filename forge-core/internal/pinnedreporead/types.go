// Package pinnedreporead implements the Linux-only, exact-path repository
// reader used by the ADR-0058 bootstrap effect profile. It never invokes Git.
package pinnedreporead

import (
	"context"
	"os"
)

const (
	MaxFiles      = 16
	MaxFileBytes  = int64(1_048_576)
	MaxTotalBytes = int64(1_048_576)
)

// ExpectedEntry is one policy-and-request-pinned raw content precondition.
type ExpectedEntry struct {
	Bytes         int64
	ContentSHA256 string
	Kind          string
	Path          string
}

type Limits struct {
	MaxFiles      int
	MaxFileBytes  int64
	MaxTotalBytes int64
}

type File struct {
	Content       []byte
	ContentSHA256 string
	Path          string
}

// CheckPlatform performs the build-tag platform check without filesystem I/O.
// Kernel and repository capabilities are checked only after durable reservation.
func CheckPlatform() error {
	return checkPlatform()
}

// PreflightRepository checks the bound repository filesystem and openat2
// primitive without reading a manifest leaf.
func PreflightRepository(repository *os.File) error {
	return preflightRepository(repository)
}

// Read reads all entries or returns no partial result. Context deadlines are
// cooperative: they are checked between syscalls and do not claim to interrupt
// an already-blocked kernel filesystem operation.
func Read(
	ctx context.Context,
	repository *os.File,
	entries []ExpectedEntry,
	limits Limits,
) ([]File, error) {
	validated, err := validateRequest(entries, limits)
	if err != nil {
		return nil, err
	}
	return readPlatform(ctx, repository, validated, limits)
}

func cloneFiles(values []File) []File {
	result := make([]File, len(values))
	for index, value := range values {
		value.Content = append([]byte(nil), value.Content...)
		result[index] = value
	}
	return result
}

func clearFiles(values []File) {
	for index := range values {
		clearBytes(values[index].Content)
		values[index].Content = nil
	}
}

func clearBytes(value []byte) {
	for index := range value {
		value[index] = 0
	}
}
