//go:build !linux || (!amd64 && !arm64)

package pinnedreporead

import (
	"context"
	"fmt"
	"os"
)

func checkPlatform() error {
	return withCode(CodeUnsupported,
		fmt.Errorf("pinned repository read requires Linux amd64 or arm64 openat2"))
}

func preflightRepository(*os.File) error {
	return checkPlatform()
}

func readPlatform(
	context.Context,
	*os.File,
	[]ExpectedEntry,
	Limits,
) ([]File, error) {
	return nil, checkPlatform()
}
