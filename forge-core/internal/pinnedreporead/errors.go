package pinnedreporead

import "errors"

// Code is the stable content-free failure classification persisted by the
// single-use execution authority after effect intent becomes durable.
type Code string

const (
	CodeContentMismatch Code = "content_mismatch"
	CodeIdentityChanged Code = "repository_identity_changed"
	CodeReadFailed      Code = "repository_read_failed"
	CodeTimeoutExceeded Code = "cooperative_timeout_exceeded"
	CodeUnsupported     Code = "unsupported"
)

type codedError struct {
	code Code
	err  error
}

func (e *codedError) Error() string { return e.err.Error() }
func (e *codedError) Unwrap() error { return e.err }

// ErrorCode returns a stable failure classification without exposing a path,
// content, digest, or underlying syscall diagnostic.
func ErrorCode(err error) Code {
	var target *codedError
	if errors.As(err, &target) {
		return target.code
	}
	return ""
}

func withCode(code Code, err error) error {
	if err == nil || ErrorCode(err) != "" {
		return err
	}
	return &codedError{code: code, err: err}
}
