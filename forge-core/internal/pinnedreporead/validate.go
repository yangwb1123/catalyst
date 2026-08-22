package pinnedreporead

import (
	"fmt"
	"strings"
	"unicode/utf8"
)

func validateRequest(entries []ExpectedEntry, limits Limits) ([]ExpectedEntry, error) {
	if err := validateLimits(limits); err != nil {
		return nil, err
	}
	if len(entries) < 1 || len(entries) > limits.MaxFiles {
		return nil, fmt.Errorf("expected entry count is outside the closed limit")
	}
	result := append([]ExpectedEntry(nil), entries...)
	var total int64
	for index, entry := range result {
		if index > 0 && result[index-1].Path >= entry.Path {
			return nil, fmt.Errorf("expected entries must be strictly path sorted")
		}
		if err := validateEntry(entry, limits, total); err != nil {
			return nil, fmt.Errorf("expected entry %d is invalid: %w", index, err)
		}
		total += entry.Bytes
	}
	return result, nil
}

func validateLimits(limits Limits) error {
	if limits.MaxFiles < 1 || limits.MaxFiles > MaxFiles || limits.MaxFileBytes < 1 ||
		limits.MaxFileBytes > MaxFileBytes || limits.MaxTotalBytes < 0 ||
		limits.MaxTotalBytes > MaxTotalBytes {
		return fmt.Errorf("read limits exceed the ADR-0058 ceiling")
	}
	return nil
}

func validateEntry(entry ExpectedEntry, limits Limits, total int64) error {
	if entry.Kind != "regular" || entry.Bytes < 0 || entry.Bytes > limits.MaxFileBytes ||
		entry.Bytes > limits.MaxTotalBytes-total {
		return fmt.Errorf("kind or byte count is invalid")
	}
	if !lowerHash(entry.ContentSHA256) {
		return fmt.Errorf("content digest is invalid")
	}
	return validatePath(entry.Path)
}

func validatePath(value string) error {
	if value == "" || value == "." || len(value) > 4096 || !utf8.ValidString(value) ||
		strings.HasPrefix(value, "/") || strings.HasSuffix(value, "/") ||
		strings.Contains(value, "\\") {
		return fmt.Errorf("path is not canonical repository-relative text")
	}
	components := strings.Split(value, "/")
	if len(components) > 256 || controlDirectory(components[0]) {
		return fmt.Errorf("path depth or control directory is forbidden")
	}
	for _, component := range components {
		if component == "" || component == "." || component == ".." ||
			strings.ContainsAny(component, "*?[]{}") {
			return fmt.Errorf("path contains a forbidden segment")
		}
		for _, character := range component {
			if forbiddenCharacter(character) {
				return fmt.Errorf("path contains forbidden Unicode")
			}
		}
	}
	return nil
}

func controlDirectory(value string) bool {
	return strings.EqualFold(value, ".git") || strings.EqualFold(value, ".forge")
}

func forbiddenCharacter(value rune) bool {
	if value <= 0x1f || value == 0x7f || value == 0x2028 || value == 0x2029 {
		return true
	}
	return value == 0x061c || value == 0x200e || value == 0x200f ||
		(value >= 0x202a && value <= 0x202e) || (value >= 0x2066 && value <= 0x2069)
}

func lowerHash(value string) bool {
	if len(value) != 64 {
		return false
	}
	for _, character := range value {
		if character < '0' || character > '9' {
			if character < 'a' || character > 'f' {
				return false
			}
		}
	}
	return true
}
