package grantstate

import (
	"fmt"
	"path"
	"path/filepath"
	"strings"
)

func validateRelative(value string, field string) error {
	hasDrive := len(value) >= 2 && asciiLetter(value[0]) && value[1] == ':'
	if value == "" || value == "." || strings.Contains(value, "\\") ||
		strings.HasPrefix(value, "/") || hasDrive || path.Clean(value) != value {
		return fmt.Errorf("%s must be a closed slash-relative path", field)
	}
	for _, part := range strings.Split(value, "/") {
		if part == "" || part == "." || part == ".." || strings.IndexByte(part, 0) >= 0 {
			return fmt.Errorf("%s must be a closed slash-relative path", field)
		}
	}
	return nil
}

func validateAbsolute(value string, field string) error {
	if value == "" || !filepath.IsAbs(value) || filepath.Clean(value) != value ||
		strings.IndexByte(value, 0) >= 0 {
		return fmt.Errorf("%s must be an absolute canonical path", field)
	}
	return nil
}

func asciiLetter(value byte) bool {
	return value >= 'a' && value <= 'z' || value >= 'A' && value <= 'Z'
}
