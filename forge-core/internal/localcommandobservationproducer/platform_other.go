//go:build !unix

package localcommandobservationproducer

import (
	"fmt"
	"runtime"
)

func ensureSupportedPlatform() error {
	return fmt.Errorf("local gate command observation v1 is unsupported on %s", runtime.GOOS)
}
