//go:build !unix

package appserver

import (
	"fmt"
	"os"
)

func openAppState(string) (*os.Root, error) {
	return nil, fmt.Errorf("forge-server state directory is unsupported on this platform")
}

func openStateLock(*os.Root) (*os.File, error) {
	return nil, fmt.Errorf("forge-server state lock is unsupported on this platform")
}

func verifyControlStateLayout(*os.Root) error {
	return fmt.Errorf("forge-server control state is unsupported on this platform")
}
