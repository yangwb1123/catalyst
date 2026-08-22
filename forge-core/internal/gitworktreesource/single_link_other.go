//go:build !unix && !windows

package gitworktreesource

import (
	"fmt"
	"os"
)

func requireSingleLinkSourceFile(path string, _ *os.File, _ os.FileInfo) error {
	return fmt.Errorf("source path %q link-count verification is unavailable on this platform", path)
}
