package gopackagedependencyobservationproducer

import (
	"fmt"
	"strings"
	"unicode"
	"unicode/utf8"
)

const maxParentEnvironmentEntries = 256

func minimalSourceEnvironment(environ []string) ([]string, error) {
	if len(environ) > maxParentEnvironmentEntries {
		return nil, fmt.Errorf("parent environment exceeds %d entries", maxParentEnvironmentEntries)
	}
	pathValue, found := "", false
	for _, entry := range environ {
		name, value, ok := strings.Cut(entry, "=")
		if !ok || name == "" {
			return nil, fmt.Errorf("parent environment contains malformed entry")
		}
		if name != "PATH" {
			continue
		}
		if found {
			return nil, fmt.Errorf("parent environment contains duplicate PATH")
		}
		if !boundedEnvironmentText(value) {
			return nil, fmt.Errorf("parent environment PATH is not bounded canonical text")
		}
		pathValue, found = value, true
	}
	if !found || pathValue == "" {
		return nil, fmt.Errorf("parent environment requires non-empty PATH")
	}
	return []string{"PATH=" + pathValue}, nil
}

func boundedEnvironmentText(value string) bool {
	if !utf8.ValidString(value) || len(value) > 16_384 || utf8.RuneCountInString(value) > 4_096 {
		return false
	}
	for _, character := range value {
		if unicode.Is(unicode.Cc, character) || character == 0x061c ||
			character == 0x200e || character == 0x200f ||
			character == 0x2028 || character == 0x2029 ||
			character >= 0x202a && character <= 0x202e ||
			character >= 0x2066 && character <= 0x2069 {
			return false
		}
	}
	return true
}
