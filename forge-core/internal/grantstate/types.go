// Package grantstate provides protected, closed-namespace byte ledgers for
// grant issuance and single-use execution state. It deliberately knows
// nothing about grant, receipt, or execution document shapes.
package grantstate

import (
	"errors"
	"fmt"
	"io/fs"
	"os"
	"sync"
)

const (
	AbsoluteMaxBytes = int64(16 << 20)
)

type Code string

const (
	CodeBusy                 Code = "BUSY"
	CodeClosed               Code = "CLOSED"
	CodeConflict             Code = "CONFLICT"
	CodeInvalid              Code = "INVALID"
	CodePersistence          Code = "PERSISTENCE"
	CodePersistenceUncertain Code = "PERSISTENCE_UNCERTAIN"
	CodeUnsafe               Code = "UNSAFE"
	CodeUnsupported          Code = "UNSUPPORTED"
)

var (
	ErrBusy                 = errors.New("grant state is busy")
	ErrPersistenceUncertain = errors.New("grant state persistence is uncertain")
)

type Error struct {
	Code Code
	Op   string
	Err  error
}

func (e *Error) Error() string {
	if e.Err == nil {
		return fmt.Sprintf("grantstate: %s: %s", e.Op, e.Code)
	}
	return fmt.Sprintf("grantstate: %s: %s: %v", e.Op, e.Code, e.Err)
}

func (e *Error) Unwrap() error { return e.Err }

func (e *Error) Is(target error) bool {
	return target == ErrBusy && e.Code == CodeBusy ||
		target == ErrPersistenceUncertain && e.Code == CodePersistenceUncertain
}

func ErrorCode(err error) Code {
	var target *Error
	if errors.As(err, &target) {
		return target.Code
	}
	return ""
}

type Config struct {
	AuthorityRoot  string
	RepositoryRoot string
	StateDir       string
	MaxBytes       int64
}

type Snapshot struct {
	Data    []byte
	Present bool
}

type commitPort interface {
	fillRandom([]byte) error
	createExclusive(*os.Root, string, fs.FileMode) (*os.File, error)
	writeAll(*os.File, []byte) error
	syncFile(*os.File) error
	closeFile(*os.File) error
	rename(*os.Root, string, string) error
	syncDirectory(*os.File) error
	remove(*os.Root, string) error
}

type sessionBackend interface {
	current() (Snapshot, error)
	commit(Snapshot, []byte) error
	readLeaf(string, int64, fs.FileMode) ([]byte, error)
	close() error
}

type repositoryBackend interface {
	bindRepository(string) error
	duplicateRepositoryRoot() (*os.File, error)
	verifyRepository() error
}

type Session struct {
	backend sessionBackend
	mu      sync.Mutex
}

func Open(config Config) (*Session, error) {
	return openPlatform(config, nil)
}

// OpenUsage opens only protected authority and usage state. It deliberately
// performs no repository operation so terminal receipt replay remains possible
// when a repository is absent, replaced, or unavailable.
func OpenUsage(config Config) (*Session, error) {
	return openUsagePlatform(config, nil)
}

func openWithPort(config Config, port commitPort) (*Session, error) {
	if port == nil {
		return nil, newError(CodeInvalid, "open", "commit port is nil", nil)
	}
	return openPlatform(config, port)
}

func openUsageWithPort(config Config, port commitPort) (*Session, error) {
	if port == nil {
		return nil, newError(CodeInvalid, "open usage", "commit port is nil", nil)
	}
	return openUsagePlatform(config, port)
}

func (s *Session) Current() (Snapshot, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.backend == nil {
		return Snapshot{}, newError(CodeClosed, "read ledger", "session is closed", nil)
	}
	value, err := s.backend.current()
	value.Data = cloneBytes(value.Data)
	return value, err
}

// Commit durably publishes caller-prepared, non-secret ledger bytes. ReadLeaf
// may return private key material; callers must never pass those bytes here.
func (s *Session) Commit(expected Snapshot, next []byte) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.backend == nil {
		return newError(CodeClosed, "commit ledger", "session is closed", nil)
	}
	expected.Data = cloneBytes(expected.Data)
	return s.backend.commit(expected, cloneBytes(next))
}

func (s *Session) ReadLeaf(relative string, max int64, mode fs.FileMode) ([]byte, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.backend == nil {
		return nil, newError(CodeClosed, "read leaf", "session is closed", nil)
	}
	value, err := s.backend.readLeaf(relative, max, mode)
	if err != nil {
		clearBytes(value)
		return nil, err
	}
	result := cloneBytes(value)
	clearBytes(value)
	return result, nil
}

// BindRepository lazily binds one repository source, proves it is disjoint
// from authority state, and returns a close-on-exec duplicate directory handle.
// A session cannot be rebound; callers own the returned handle.
func (s *Session) BindRepository(source string) (*os.File, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.backend == nil {
		return nil, newError(CodeClosed, "bind repository", "session is closed", nil)
	}
	backend, ok := s.backend.(repositoryBackend)
	if !ok {
		return nil, newError(CodeUnsupported, "bind repository", "backend cannot bind a repository", nil)
	}
	if err := backend.bindRepository(source); err != nil {
		return nil, err
	}
	return backend.duplicateRepositoryRoot()
}

// VerifyRepository revalidates source resolution, the opened directory
// identity, and authority disjointness without reading repository content.
func (s *Session) VerifyRepository() error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.backend == nil {
		return newError(CodeClosed, "verify repository", "session is closed", nil)
	}
	backend, ok := s.backend.(repositoryBackend)
	if !ok {
		return newError(CodeUnsupported, "verify repository", "backend has no repository", nil)
	}
	return backend.verifyRepository()
}

func (s *Session) Close() error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.backend == nil {
		return nil
	}
	err := s.backend.close()
	s.backend = nil
	return err
}

func newError(code Code, op, detail string, cause error) error {
	if cause != nil && detail != "" {
		cause = fmt.Errorf("%s: %w", detail, cause)
	} else if cause == nil && detail != "" {
		cause = errors.New(detail)
	}
	return &Error{Code: code, Op: op, Err: cause}
}

func cloneBytes(value []byte) []byte {
	if value == nil {
		return nil
	}
	return append([]byte(nil), value...)
}

func clearBytes(value []byte) {
	for index := range value {
		value[index] = 0
	}
}

func discardBytes(value []byte) []byte {
	clearBytes(value)
	return nil
}
