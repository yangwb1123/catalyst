package outputbinding

import (
	"encoding/hex"
	"fmt"
	"strings"
	"unicode"
	"unicode/utf8"
)

func validateWireText(value string, allowEmpty bool, maximum int) error {
	if !utf8.ValidString(value) || len(value) > maximum || (!allowEmpty && value == "") {
		return fmt.Errorf("text must be valid UTF-8 and contain at most %d bytes", maximum)
	}
	for _, character := range value {
		if forbiddenCharacter(character) {
			return fmt.Errorf("text contains forbidden Unicode U+%04X", character)
		}
	}
	return nil
}

func validateIdentifier(label, value string) error {
	if err := validateWireText(value, false, maxIdentifierBytes); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	if strings.TrimSpace(value) != value {
		return fmt.Errorf("%s must not have surrounding whitespace", label)
	}
	return nil
}

func validateOptionalIdentifier(label, value string) error {
	if value == "" {
		return nil
	}
	return validateIdentifier(label, value)
}

func forbiddenCharacter(value rune) bool {
	if unicode.IsControl(value) || value == 0x2028 || value == 0x2029 {
		return true
	}
	return value == 0x061c || value == 0x200e || value == 0x200f ||
		(value >= 0x202a && value <= 0x202e) || (value >= 0x2066 && value <= 0x2069)
}

func validDigest(value string) bool {
	if len(value) != 64 || value != strings.ToLower(value) {
		return false
	}
	_, err := hex.DecodeString(value)
	return err == nil
}

func requireDigest(label, value string) error {
	if !validDigest(value) {
		return fmt.Errorf("%s must be 64 lowercase hexadecimal characters", label)
	}
	return nil
}

func validSourceRevision(value string) bool {
	if strings.HasPrefix(value, "git-sha1:") {
		digest := strings.TrimPrefix(value, "git-sha1:")
		return len(digest) == 40 && validLowerHex(digest)
	}
	if strings.HasPrefix(value, "git-sha256:") {
		digest := strings.TrimPrefix(value, "git-sha256:")
		return len(digest) == 64 && validLowerHex(digest)
	}
	return false
}

func validLowerHex(value string) bool {
	if value == "" {
		return false
	}
	for _, character := range []byte(value) {
		if (character < '0' || character > '9') && (character < 'a' || character > 'f') {
			return false
		}
	}
	return true
}

func oneOf(value string, allowed ...string) bool {
	for _, candidate := range allowed {
		if value == candidate {
			return true
		}
	}
	return false
}
