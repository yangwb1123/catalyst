package domain

import (
	"strings"
	"unicode"
	"unicode/utf8"

	core "forgeos/forge-core/internal/platformcorecontract"
)

func invalidDomain(label string, cause error) error {
	return &relationError{relation: ErrInvalidDomain, detail: label, cause: cause}
}

func invalidTransition(label string, cause error) error {
	return &relationError{relation: ErrInvalidTransition, detail: label, cause: cause}
}

func validateText(value, label string, minimum, maximum int) error {
	if len(value) < minimum || len(value) > maximum || !utf8.ValidString(value) ||
		strings.TrimSpace(value) != value {
		return invalidDomain(label+" is not bounded canonical text", nil)
	}
	for _, character := range value {
		if unicode.IsControl(character) || unicode.In(
			character, unicode.Cf, unicode.Zl, unicode.Zp,
		) {
			return invalidDomain(label+" contains a control or format character", nil)
		}
	}
	return nil
}

func validateToken(value, label string, maximum int) error {
	if len(value) < 1 || len(value) > maximum || !isLowerAlphaNumeric(value[0]) ||
		!isLowerAlphaNumeric(value[len(value)-1]) {
		return invalidDomain(label+" is not a bounded token", nil)
	}
	for _, character := range []byte(value) {
		if !isLowerAlphaNumeric(character) && !strings.ContainsRune("._-", rune(character)) {
			return invalidDomain(label+" contains an unsupported byte", nil)
		}
	}
	return nil
}

func validateEntityID(value, entityType, label string) error {
	ref := core.EntityRef{EntityID: value, EntityType: core.EntityType(entityType)}
	if err := core.ValidateReference(ref); err != nil {
		return invalidDomain(label+" is not a typed Platform ID", err)
	}
	return nil
}

func validateActor(value core.ActorRef, label string) error {
	if !isBoundedClosedValue(string(value.ActorType)) {
		return invalidDomain(label+" actor type is not bounded", nil)
	}
	if err := core.ValidateReference(value); err != nil {
		return invalidDomain(label+" is invalid", err)
	}
	return nil
}

func validateUnixMS(value int64, label string) error {
	if value < 0 || value > maxUnixMilliseconds {
		return invalidDomain(label+" is not a bounded timestamp", nil)
	}
	return nil
}

func validateVersion(value int64, label string) error {
	if value < 1 {
		return invalidDomain(label+" must be positive", nil)
	}
	return nil
}

func isLowerAlphaNumeric(value byte) bool {
	return value >= 'a' && value <= 'z' || value >= '0' && value <= '9'
}

func isBoundedClosedValue(value string) bool {
	return len(value) >= 1 && len(value) <= 32
}
