package appserver

import (
	"errors"
	"testing"
)

type errorCloser struct{ err error }

func (value errorCloser) Close() error { return value.err }

func TestCloseControlStorePreservesCloseAndPrimaryErrors(t *testing.T) {
	primary := errors.New("serve failed")
	closeErr := errors.New("database close failed")
	for _, test := range []struct {
		name    string
		initial error
	}{
		{name: "close only"},
		{name: "primary and close", initial: primary},
	} {
		t.Run(test.name, func(t *testing.T) {
			result := test.initial
			closeControlStore(errorCloser{err: closeErr}, &result)
			if !errors.Is(result, closeErr) {
				t.Fatalf("result %v lost close error", result)
			}
			if test.initial != nil && !errors.Is(result, primary) {
				t.Fatalf("result %v lost primary error", result)
			}
		})
	}
}
