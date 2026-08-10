package gitworktreesource

import (
	"fmt"
	"path/filepath"
	"strings"
)

func validateInventoryPath(value string, tracked bool) error {
	if err := validateText("source path", value, false); err != nil {
		return err
	}
	hasDrivePrefix := len(value) >= 2 && asciiLetter(value[0]) && value[1] == ':'
	if filepath.IsAbs(value) || hasDrivePrefix || strings.Contains(value, `\`) ||
		filepath.ToSlash(filepath.Clean(value)) != value || value == "." {
		return fmt.Errorf("source path %q is not canonical repo-relative forward-slash form", value)
	}
	folded := foldASCII(value)
	if pathWithin(folded, ".git") || tracked && pathWithin(folded, ".forge") ||
		pathWithin(folded, ".forge") && !pathWithin(value, ".forge") {
		return fmt.Errorf("tracked or control source path %q is forbidden", value)
	}
	return nil
}

func pathWithin(value, directory string) bool {
	return value == directory || strings.HasPrefix(value, directory+"/")
}

func foldASCII(value string) string {
	folded := []byte(value)
	for index, character := range folded {
		if character >= 'A' && character <= 'Z' {
			folded[index] = character + ('a' - 'A')
		}
	}
	return string(folded)
}

func asciiLetter(value byte) bool {
	return value >= 'A' && value <= 'Z' || value >= 'a' && value <= 'z'
}
