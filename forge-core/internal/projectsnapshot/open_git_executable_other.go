//go:build !linux

package projectsnapshot

import (
	"fmt"
	"os"
)

func openGitExecutable(_ string) (*os.File, error) {
	return nil, fmt.Errorf("nofollow Git executable opening requires Linux")
}
