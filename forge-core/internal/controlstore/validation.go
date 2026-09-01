package controlstore

import (
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"fmt"
	"strings"
	"unicode/utf8"

	core "forgeos/forge-core/internal/platformcorecontract"
)

const maxUnixMilliseconds = int64(253402300799999)

func digestBytes(value []byte) string {
	digest := sha256.Sum256(value)
	return hex.EncodeToString(digest[:])
}

func verifyDigest(value []byte, want, label string) error {
	if digestBytes(value) != want {
		return fmt.Errorf("%w: %s digest mismatch", ErrCorruptStore, label)
	}
	return nil
}

func validateBody(value []byte, label string, nonempty bool) error {
	if len(value) > maxBodyBytes || nonempty && len(value) == 0 {
		return fmt.Errorf("%s must contain %d..%d bytes", label, boolMinimum(nonempty), maxBodyBytes)
	}
	return nil
}

func boolMinimum(nonempty bool) int {
	if nonempty {
		return 1
	}
	return 0
}

func validateUnixMS(value int64, label string) error {
	if value < 0 || value > maxUnixMilliseconds {
		return fmt.Errorf("%s must be a bounded Unix millisecond timestamp", label)
	}
	return nil
}

func validatePage(after int64, limit int) error {
	if after < 0 || limit < 1 || limit > maxPageItems {
		return fmt.Errorf("page requires after >= 0 and limit in 1..%d", maxPageItems)
	}
	return nil
}

func validatePlatformID(value, prefix, label string) error {
	if err := core.ValidatePlatformID(value); err != nil || !strings.HasPrefix(value, prefix+"_") {
		return fmt.Errorf("%s must be a %s_ Platform ID", label, prefix)
	}
	return nil
}

func validateToken(value, label string, maximum int) error {
	if value == "" || len(value) > maximum || !utf8.ValidString(value) {
		return fmt.Errorf("%s must contain 1..%d UTF-8 bytes", label, maximum)
	}
	for _, character := range value {
		if !isTokenCharacter(character) {
			return fmt.Errorf("%s contains an unsupported character", label)
		}
	}
	return nil
}

func isTokenCharacter(value rune) bool {
	return value >= 'a' && value <= 'z' || value >= '0' && value <= '9' ||
		strings.ContainsRune("._:/-", value)
}

func cloneBytes(value []byte) []byte {
	result := make([]byte, len(value))
	copy(result, value)
	return result
}

func isControlSource(value core.SourceComponent) bool {
	switch string(value) {
	case "app_server", "control_plane", "legacy_importer":
		return true
	default:
		return false
	}
}

func isControlAggregate(value core.EntityType) bool {
	switch string(value) {
	case "space", "project", "project_snapshot", "objective", "change", "work_graph", "work_item":
		return true
	default:
		return false
	}
}

func sameNullableString(value *string, stored sql.NullString) bool {
	if value == nil {
		return !stored.Valid
	}
	return stored.Valid && *value == stored.String
}

func sameStringPointer(left, right *string) bool {
	if left == nil || right == nil {
		return left == nil && right == nil
	}
	return *left == *right
}
