//go:build !unix

package projectsnapshot

import (
	"fmt"
	"os"
)

func openRegularLeaf(_ *os.Root, _ string) (*os.File, error) {
	return nil, fmt.Errorf("nofollow project source leaf opening is unavailable")
}
