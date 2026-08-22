package projectsnapshot

import (
	"fmt"
	"strings"
	"unicode"
	"unicode/utf8"
)

func validateIdentifier(label, value string) error {
	if value == "" || len(value) > 160 || !utf8.ValidString(value) {
		return fmt.Errorf("%s violates identifier bounds", label)
	}
	for index, character := range []byte(value) {
		letter := character >= 'a' && character <= 'z'
		digit := character >= '0' && character <= '9'
		if index == 0 && !letter && !digit || index > 0 && !letter && !digit &&
			!strings.ContainsRune("._:/-", rune(character)) {
			return fmt.Errorf("%s is not a canonical identifier", label)
		}
	}
	return nil
}

func validDigest(value string) bool { return len(value) == 64 && lowerHex(value) }

func validBoundedText(value string, maximum int) bool {
	if value == "" || len(value) > maximum || !utf8.ValidString(value) {
		return false
	}
	for _, character := range value {
		if unicode.Is(unicode.Cc, character) || character == '\ufeff' || forbiddenCharacter(character) {
			return false
		}
	}
	return true
}

func validRevision(value string) bool {
	if strings.HasPrefix(value, "git-sha1:") {
		return len(value) == len("git-sha1:")+40 && lowerHex(strings.TrimPrefix(value, "git-sha1:"))
	}
	if strings.HasPrefix(value, "git-sha256:") {
		return len(value) == len("git-sha256:")+64 && lowerHex(strings.TrimPrefix(value, "git-sha256:"))
	}
	return false
}
