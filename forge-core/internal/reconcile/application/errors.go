package application

import (
	"errors"
	"fmt"
)

// ErrInvalidControlSnapshot identifies malformed or internally mixed input.
var ErrInvalidControlSnapshot = errors.New("reconcile control snapshot is invalid")

func invalidSnapshot(detail string, cause error) error {
	if cause == nil {
		return fmt.Errorf("%w: %s", ErrInvalidControlSnapshot, detail)
	}
	return fmt.Errorf("%w: %s: %v", ErrInvalidControlSnapshot, detail, cause)
}
