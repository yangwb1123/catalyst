package gopackagegraph

import (
	"fmt"
	"path"
	"strings"
	"unicode"
	"unicode/utf8"
)

const (
	maxTextBytes   = 16_384
	maxTextScalars = 4_096
)

var goKeywords = map[string]struct{}{
	"break": {}, "case": {}, "chan": {}, "const": {}, "continue": {},
	"default": {}, "defer": {}, "else": {}, "fallthrough": {}, "for": {},
	"func": {}, "go": {}, "goto": {}, "if": {}, "import": {},
	"interface": {}, "map": {}, "package": {}, "range": {}, "return": {},
	"select": {}, "struct": {}, "switch": {}, "type": {}, "var": {},
}

func safeDirectory(value string) bool {
	return value == "." || safeRepoPath(value)
}

func safeRepoPath(value string) bool {
	hasDrivePrefix := len(value) >= 2 &&
		(value[0] >= 'A' && value[0] <= 'Z' || value[0] >= 'a' && value[0] <= 'z') &&
		value[1] == ':'
	if value == "" || value == "." || strings.HasPrefix(value, "/") ||
		hasDrivePrefix || strings.Contains(value, `\`) || path.Clean(value) != value {
		return false
	}
	components := strings.Split(value, "/")
	if strings.EqualFold(components[0], ".git") || strings.EqualFold(components[0], ".forge") {
		return false
	}
	for _, component := range components {
		if component == "" || component == "." || component == ".." {
			return false
		}
	}
	return validText(value)
}

func pathWithin(value, directory string) bool {
	return directory == "." || value == directory || strings.HasPrefix(value, directory+"/")
}

func joinDirectory(directory, name string) string {
	if directory == "." {
		return name
	}
	return directory + "/" + name
}

func pathDirectory(value string) string {
	directory := path.Dir(value)
	if directory == "" {
		return "."
	}
	return directory
}

func relativeDirectory(value, root string) string {
	if root == "." {
		return value
	}
	return strings.TrimPrefix(value, root+"/")
}

func canonicalImportPath(value string) bool {
	if !validText(value) || value[0] == '/' || value[0] == '.' || value[0] == '-' ||
		strings.HasSuffix(value, "/") || strings.Contains(value, "//") {
		return false
	}
	for _, component := range strings.Split(value, "/") {
		if !canonicalImportPathComponent(component) {
			return false
		}
	}
	return true
}

func canonicalImportPathComponent(value string) bool {
	if value == "" || value[0] == '.' || strings.HasSuffix(value, ".") ||
		strings.Contains(value, "..") {
		return false
	}
	allDots := true
	for index := range len(value) {
		if !canonicalImportPathByte(value[index]) {
			return false
		}
		allDots = allDots && value[index] == '.'
	}
	base := value
	if index := strings.IndexByte(value, '.'); index >= 0 {
		base = value[:index]
	}
	return !allDots && !windowsReservedImportBase(base) && !shortNameImportBase(base)
}

func canonicalImportPathByte(value byte) bool {
	letter := value >= 'A' && value <= 'Z' || value >= 'a' && value <= 'z'
	digit := value >= '0' && value <= '9'
	return letter || digit || value == '-' || value == '.' || value == '_' ||
		value == '~' || value == '+'
}

func windowsReservedImportBase(value string) bool {
	upper := strings.ToUpper(value)
	if upper == "CON" || upper == "PRN" || upper == "AUX" || upper == "NUL" {
		return true
	}
	if len(upper) != 4 || upper[3] < '1' || upper[3] > '9' {
		return false
	}
	return upper[:3] == "COM" || upper[:3] == "LPT"
}

func shortNameImportBase(value string) bool {
	index := strings.LastIndexByte(value, '~')
	if index < 0 || index == len(value)-1 {
		return false
	}
	for _, character := range []byte(value[index+1:]) {
		if character < '0' || character > '9' {
			return false
		}
	}
	return true
}

func validText(value string) bool {
	if value == "" || !utf8.ValidString(value) || len(value) > maxTextBytes ||
		utf8.RuneCountInString(value) > maxTextScalars {
		return false
	}
	for _, character := range value {
		if forbiddenTextRune(character) {
			return false
		}
	}
	return true
}

func validPackageIdentifier(value string) bool {
	if value == "" {
		return false
	}
	if _, keyword := goKeywords[value]; keyword {
		return false
	}
	for index, character := range []byte(value) {
		letter := character >= 'A' && character <= 'Z' || character >= 'a' && character <= 'z'
		digit := character >= '0' && character <= '9'
		if character != '_' && !letter && (index == 0 || !digit) {
			return false
		}
	}
	return true
}

func supportedSourceText(value []byte) bool {
	for _, character := range string(value) {
		if forbiddenSourceRune(character) {
			return false
		}
	}
	return true
}

func forbiddenTextRune(value rune) bool {
	return unicode.Is(unicode.Cc, value) || forbiddenDirectionalRune(value)
}

func forbiddenSourceRune(value rune) bool {
	if value == '\t' || value == '\n' || value == '\r' || value == '\f' {
		return false
	}
	return unicode.Is(unicode.Cc, value) || forbiddenDirectionalRune(value)
}

func forbiddenDirectionalRune(value rune) bool {
	return value == 0x061c || value == 0x200e || value == 0x200f ||
		value == 0x2028 || value == 0x2029 || value >= 0x202a && value <= 0x202e ||
		value >= 0x2066 && value <= 0x2069
}

func ValidateModuleDirectory(value string) error {
	if !safeDirectory(value) || !validText(value) {
		return fmt.Errorf("module_directory is not a canonical repository directory")
	}
	return nil
}
