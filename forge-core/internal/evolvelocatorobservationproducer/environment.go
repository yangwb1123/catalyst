package evolvelocatorobservationproducer

import (
	"fmt"
	"strings"
)

const maxParentEnvironmentEntries = 256

func minimalSourceEnvironment(environ []string) ([]string, error) {
	if len(environ) > maxParentEnvironmentEntries {
		return nil, fmt.Errorf("parent environment exceeds %d entries", maxParentEnvironmentEntries)
	}
	pathValue := ""
	pathSeen := false
	for _, entry := range environ {
		name, value, ok := strings.Cut(entry, "=")
		if !ok || name == "" {
			return nil, fmt.Errorf("parent environment contains malformed entry")
		}
		if name != "PATH" {
			continue
		}
		if pathSeen {
			return nil, fmt.Errorf("parent environment contains duplicate %s", name)
		}
		if err := validateCanonicalText(value); err != nil {
			return nil, fmt.Errorf("parent environment %s: %w", name, err)
		}
		pathSeen, pathValue = true, value
	}
	if pathValue == "" {
		return nil, fmt.Errorf("parent environment requires non-empty PATH")
	}
	return []string{"PATH=" + pathValue}, nil
}
