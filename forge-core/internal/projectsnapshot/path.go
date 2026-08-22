package projectsnapshot

import (
	"fmt"
	"path/filepath"
	"strings"
	"unicode"
	"unicode/utf8"
)

func validateInventoryPath(value string) error {
	if value == "" || value == "." || len(value) > maxPathBytes || !utf8.ValidString(value) ||
		filepath.IsAbs(value) || strings.HasPrefix(value, "/") || strings.HasSuffix(value, "/") ||
		strings.Contains(value, `\`) || hasDrivePrefix(value) {
		return fmt.Errorf("Git inventory path %q is not canonical repository-relative UTF-8", value)
	}
	components := strings.Split(value, "/")
	if len(components) > maxPathComponents || utf8.RuneCountInString(value) > maxPathScalars {
		return fmt.Errorf("Git inventory path exceeds %d components", maxPathComponents)
	}
	for _, component := range components {
		if component == "" || component == "." || component == ".." {
			return fmt.Errorf("Git inventory path contains a forbidden component")
		}
		for _, character := range component {
			if forbiddenCharacter(character) {
				return fmt.Errorf("Git inventory path contains forbidden Unicode")
			}
		}
	}
	return nil
}

func hasDrivePrefix(value string) bool {
	return len(value) >= 2 && ((value[0] >= 'a' && value[0] <= 'z') ||
		(value[0] >= 'A' && value[0] <= 'Z')) && value[1] == ':'
}

func forbiddenCharacter(value rune) bool {
	if unicode.Is(unicode.Cc, value) || value == 0xfeff || value == 0x2028 || value == 0x2029 {
		return true
	}
	return value == 0x061c || value == 0x200e || value == 0x200f ||
		(value >= 0x202a && value <= 0x202e) || (value >= 0x2066 && value <= 0x2069)
}

func foldASCII(value string) string {
	result := []byte(value)
	for index, character := range result {
		if character >= 'A' && character <= 'Z' {
			result[index] = character + ('a' - 'A')
		}
	}
	return string(result)
}

func pathWithin(value, prefix string) bool {
	return value == prefix || strings.HasPrefix(value, prefix+"/")
}
