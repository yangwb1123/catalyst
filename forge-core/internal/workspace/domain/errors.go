package domain

import "errors"

// ErrInvalidHistory marks a Workspace event history that cannot be folded.
var ErrInvalidHistory = errors.New("workspace history is invalid")
