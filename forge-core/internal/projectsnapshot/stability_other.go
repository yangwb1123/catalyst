//go:build !unix && !windows

package projectsnapshot

import (
	"fmt"
	"os"
)

func stableChangeIdentity(_, _ os.FileInfo) bool { return true }

func requireSingleLink(_ *os.File, _ os.FileInfo) error {
	return fmt.Errorf("project snapshot link-count verification is unavailable")
}
