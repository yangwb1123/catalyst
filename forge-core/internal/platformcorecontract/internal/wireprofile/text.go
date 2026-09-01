package wireprofile

import (
	"fmt"
	"strings"
	"unicode/utf8"
)

// ValidateText enforces the shared bounded Unicode profile.
func ValidateText(value, label string, maximum int, nonempty bool) error {
	if len(value) > maximum || !utf8.ValidString(value) || nonempty && value == "" {
		return fmt.Errorf("%s must be valid UTF-8 text within 1..%d bytes", label, maximum)
	}
	for _, character := range value {
		if forbiddenScalar(character) {
			return fmt.Errorf("%s contains forbidden Unicode U+%04X", label, character)
		}
	}
	return nil
}

func forbiddenScalar(value rune) bool {
	if value <= 0x1f || value >= 0x7f && value <= 0x9f {
		return true
	}
	return value == 0x061c || value == 0x200e || value == 0x200f ||
		value >= 0x2028 && value <= 0x202e || value >= 0x2066 && value <= 0x2069
}

// ValidateLowerToken enforces the shared lowercase token profile.
func ValidateLowerToken(value, label string, maximum int) error {
	if err := ValidateText(value, label, maximum, true); err != nil {
		return err
	}
	for index, character := range []byte(value) {
		valid := character >= 'a' && character <= 'z' ||
			index > 0 && (character >= '0' && character <= '9' || character == '_')
		if !valid {
			return fmt.Errorf("%s must be lowercase ASCII token text", label)
		}
	}
	return nil
}

// ValidateSchemaName enforces the shared namespaced schema profile.
func ValidateSchemaName(value, label string) error {
	if err := ValidateText(value, label, 128, true); err != nil {
		return err
	}
	parts := strings.Split(value, ".")
	if len(parts) < 3 || len(parts) > 6 {
		return fmt.Errorf("%s must contain three to six namespace segments", label)
	}
	for _, part := range parts {
		if len(part) < 1 || len(part) > 32 {
			return fmt.Errorf("%s namespace segment length is invalid", label)
		}
		if err := ValidateLowerToken(part, label, 32); err != nil {
			return err
		}
	}
	return nil
}

// ValidateHash requires one lowercase SHA-256 hex value.
func ValidateHash(value, label string) error {
	if len(value) != 64 {
		return fmt.Errorf("%s must be a lowercase bare SHA-256", label)
	}
	for _, character := range []byte(value) {
		if !(character >= '0' && character <= '9') && !(character >= 'a' && character <= 'f') {
			return fmt.Errorf("%s must be a lowercase bare SHA-256", label)
		}
	}
	return nil
}

// ValidateUnixMS enforces the shared positive Unix-millisecond range.
func ValidateUnixMS(value int64, label string) error {
	if value < 1 || value > maxUnixMilliseconds {
		return fmt.Errorf("%s must be in 1..%d", label, maxUnixMilliseconds)
	}
	return nil
}
