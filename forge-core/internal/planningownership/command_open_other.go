//go:build !unix && !windows

package planningownership

import (
	"fmt"
	"os"
)

func openRegularNoFollow(string) (*os.File, error) {
	return nil, fmt.Errorf("regular no-follow input reads are unsupported on this platform")
}

func samePlatformChangeTime(left, right os.FileInfo) bool { return false }
