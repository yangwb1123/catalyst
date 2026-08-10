package gitworktreesource

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

func resolveGitExecutable(ctx context.Context, pathValue string) (string, error) {
	directories := filepath.SplitList(pathValue)
	if len(directories) == 0 {
		return "", fmt.Errorf("PATH entry %q must be a normalized absolute path", "")
	}
	for _, directory := range directories {
		if directory == "" || !filepath.IsAbs(directory) || filepath.Clean(directory) != directory {
			return "", fmt.Errorf("PATH entry %q must be a normalized absolute path", directory)
		}
	}
	for _, directory := range directories {
		if err := ctx.Err(); err != nil {
			return "", fmt.Errorf("resolve executable: %w", err)
		}
		candidate := filepath.Join(directory, "git")
		info, err := os.Stat(candidate)
		if err == nil && info.Mode().IsRegular() && info.Mode().Perm()&0o111 != 0 {
			return candidate, nil
		}
		if err != nil && !os.IsNotExist(err) {
			return "", fmt.Errorf("inspect executable %q: %w", candidate, err)
		}
	}
	return "", fmt.Errorf("executable %q is not available on scrubbed PATH", "git")
}

func exactPathValue(environment []string) (string, error) {
	var pathValue string
	count := 0
	for _, entry := range environment {
		entryName, value, ok := strings.Cut(entry, "=")
		if ok && entryName == "PATH" {
			count++
			pathValue = value
		}
	}
	if count != 1 {
		return "", fmt.Errorf("child environment must contain exactly one PATH entry")
	}
	return pathValue, nil
}
