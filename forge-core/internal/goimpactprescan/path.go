package goimpactprescan

import (
	"path"
	"strings"
	"unicode"
	"unicode/utf8"
)

func validRepoPath(value string) bool {
	hasDrive := len(value) >= 2 && asciiLetter(value[0]) && value[1] == ':'
	if value == "" || value == "." || !utf8.ValidString(value) ||
		strings.HasPrefix(value, "/") || hasDrive ||
		strings.Contains(value, `\`) || path.Clean(value) != value ||
		len(value) > 16_384 || utf8.RuneCountInString(value) > 4_096 {
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
	for _, character := range value {
		if unicode.Is(unicode.Cc, character) || forbiddenDirectional(character) {
			return false
		}
	}
	return true
}

func forbiddenDirectional(value rune) bool {
	return value == 0x061c || value == 0x200e || value == 0x200f ||
		value == 0x2028 || value == 0x2029 || value >= 0x202a && value <= 0x202e ||
		value >= 0x2066 && value <= 0x2069
}

func asciiLetter(value byte) bool {
	return value >= 'A' && value <= 'Z' || value >= 'a' && value <= 'z'
}

func pathWithin(value, directory string) bool {
	return directory == "." || value == directory || strings.HasPrefix(value, directory+"/")
}
