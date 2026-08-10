package gitworktreesource

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"unicode"
	"unicode/utf8"
)

const (
	maxManifestBytes = 16 << 20
	maxTextBytes     = 16_384
	maxTextScalars   = 4_096
)

func CanonicalManifestJSON(manifest SourceManifest) ([]byte, error) {
	if err := validateManifest(manifest); err != nil {
		return nil, err
	}
	return canonicalManifestJSON(manifest)
}

func canonicalManifestJSON(manifest SourceManifest) ([]byte, error) {
	buffer := bytes.NewBuffer(nil)
	encoder := json.NewEncoder(buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(manifest); err != nil {
		return nil, fmt.Errorf("encode canonical source manifest: %w", err)
	}
	encoded := buffer.Bytes()
	if len(encoded) == 0 || encoded[len(encoded)-1] != '\n' {
		return nil, fmt.Errorf("canonical source manifest encoder did not terminate predictably")
	}
	encoded = encoded[:len(encoded)-1]
	if len(encoded) > maxManifestBytes {
		return nil, fmt.Errorf("canonical source manifest exceeds %d bytes", maxManifestBytes)
	}
	return append([]byte(nil), encoded...), nil
}

func Digest(manifest SourceManifest) (string, error) {
	encoded, err := CanonicalManifestJSON(manifest)
	if err != nil {
		return "", err
	}
	hasher := sha256.New()
	_, _ = hasher.Write([]byte(DigestDomain))
	_, _ = hasher.Write([]byte{0})
	_, _ = hasher.Write(encoded)
	return hex.EncodeToString(hasher.Sum(nil)), nil
}

func sha256Bytes(value []byte) string {
	digest := sha256.Sum256(value)
	return hex.EncodeToString(digest[:])
}

func validateText(label, value string, allowEmpty bool) error {
	if !utf8.ValidString(value) {
		return fmt.Errorf("%s must be valid UTF-8", label)
	}
	if (!allowEmpty && value == "") || len(value) > maxTextBytes || utf8.RuneCountInString(value) > maxTextScalars {
		return fmt.Errorf("%s violates bounded text limits", label)
	}
	for _, character := range value {
		if forbiddenCharacter(character) {
			return fmt.Errorf("%s contains forbidden Unicode U+%04X", label, character)
		}
	}
	return nil
}

func forbiddenCharacter(value rune) bool {
	if unicode.Is(unicode.Cc, value) || value == 0x2028 || value == 0x2029 {
		return true
	}
	return value == 0x061c || value == 0x200e || value == 0x200f ||
		(value >= 0x202a && value <= 0x202e) || (value >= 0x2066 && value <= 0x2069)
}

func lowerHex(value string) bool {
	for _, character := range value {
		if character < '0' || character > '9' {
			if character < 'a' || character > 'f' {
				return false
			}
		}
	}
	return value != ""
}

func stringPointer(value string) *string { return &value }
func boolPointer(value bool) *bool       { return &value }
