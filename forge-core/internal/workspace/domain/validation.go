package domain

import (
	"path"
	"strings"
	"unicode"
	"unicode/utf8"

	core "forgeos/forge-core/internal/platformcorecontract"
)

const maxUnixMilliseconds = int64(253402300799999)

func validateName(value string) error {
	if len(value) < 1 || len(value) > 128 || !utf8.ValidString(value) ||
		strings.TrimSpace(value) != value {
		return invalidHistory("space name is not bounded canonical text")
	}
	for _, character := range value {
		if unicode.IsControl(character) || isDirectionalControl(character) {
			return invalidHistory("space name contains a control character")
		}
	}
	return nil
}

func validateAlias(value string) error {
	if len(value) < 1 || len(value) > 64 || !isLowerAlphaNumeric(value[0]) ||
		!isLowerAlphaNumeric(value[len(value)-1]) {
		return invalidHistory("project alias is not a bounded display token")
	}
	for _, character := range []byte(value) {
		if !isLowerAlphaNumeric(character) && !strings.ContainsRune("._-", rune(character)) {
			return invalidHistory("project alias contains an unsupported byte")
		}
	}
	return nil
}

func validateRootPath(value string) error {
	if len(value) < 2 || len(value) > 4096 || !utf8.ValidString(value) ||
		!strings.HasPrefix(value, "/") || path.Clean(value) != value {
		return invalidHistory("project root is not a canonical absolute POSIX path")
	}
	for _, character := range value {
		if unicode.IsControl(character) || isDirectionalControl(character) {
			return invalidHistory("project root contains a control character")
		}
	}
	return nil
}

func validateObservation(value core.RecordRef) error {
	if err := core.ValidateReference(value); err != nil {
		return invalidHistory("snapshot observation_ref is invalid")
	}
	return nil
}

func validateUnixMS(value int64, label string) error {
	if value < 0 || value > maxUnixMilliseconds {
		return invalidHistory(label + " is not a bounded timestamp")
	}
	return nil
}

func isLowerAlphaNumeric(value byte) bool {
	return value >= 'a' && value <= 'z' || value >= '0' && value <= '9'
}

func isDirectionalControl(value rune) bool {
	return value == '\u061c' || value == '\u200e' || value == '\u200f' ||
		value >= '\u202a' && value <= '\u202e' ||
		value >= '\u2066' && value <= '\u2069'
}
