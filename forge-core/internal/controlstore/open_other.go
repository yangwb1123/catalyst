//go:build !linux || android

package controlstore

import (
	"context"
	"fmt"
	"os"
)

// OpenBound fails closed where the App Server has no descriptor-bound state model.
func OpenBound(context.Context, *os.Root) (*Store, error) {
	return nil, fmt.Errorf("control store is unsupported on this platform")
}
