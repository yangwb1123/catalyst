package platformcorecontract

import (
	"strings"
)

var entityPrefixes = map[EntityType]string{
	entityAction: "act", entityActor: "acr", entityArtifact: "art",
	entityAttempt: "atm", entityChange: "chg", entityObjective: "obj",
	entityProject: "prj", entityProjectSnapshot: "psn", entityReceipt: "rcp",
	entitySession: "ses", entitySpace: "spc", entityTurn: "trn",
	entityWorkGraph: "wgr", entityWorkItem: "wki",
}

var platformPrefixes = map[string]struct{}{
	"act": {}, "acr": {}, "art": {}, "atm": {}, "chg": {}, "cmd": {},
	"cor": {}, "evt": {}, "msg": {}, "obj": {}, "prj": {}, "psn": {},
	"rcp": {}, "ses": {}, "spc": {}, "trn": {}, "ver": {}, "wgr": {},
	"wki": {},
}

// ValidatePlatformID checks the frozen typed opaque Platform ID grammar.
func ValidatePlatformID(value string) error {
	if len(value) != 30 || value[3] != '_' {
		return reject(rejectionIdentifierInvalid, "platform ID must have a three-byte prefix and 26-byte suffix")
	}
	if _, ok := platformPrefixes[value[:3]]; !ok {
		return rejectf(rejectionIdentifierInvalid, "platform ID prefix %q is unsupported", value[:3])
	}
	return validateIDSuffix(value[4:])
}

// parseEntityType validates one protocol entity vocabulary value.
func parseEntityType(value string) (EntityType, error) {
	typed := EntityType(value)
	if _, ok := entityPrefixes[typed]; !ok {
		return "", rejectf(rejectionValueInvalid, "entity type %q is unsupported", value)
	}
	return typed, nil
}

// parseActorType validates one caller-declared actor vocabulary value.
func parseActorType(value string) (ActorType, error) {
	typed := ActorType(value)
	switch typed {
	case actorAgent, actorHarness, actorHuman, actorService, actorSystem:
		return typed, nil
	default:
		return "", rejectf(rejectionValueInvalid, "actor type %q is unsupported", value)
	}
}

// parseSourceComponent validates one event source vocabulary value.
func parseSourceComponent(value string) (SourceComponent, error) {
	typed := SourceComponent(value)
	switch typed {
	case sourceAppServer, sourceControlPlane, sourceHarness, sourceLegacyImporter, sourceRuntime:
		return typed, nil
	default:
		return "", rejectf(rejectionValueInvalid, "source component %q is unsupported", value)
	}
}

func validateTypedID(value, prefix, label string) error {
	if err := ValidatePlatformID(value); err != nil {
		return err
	}
	if !strings.HasPrefix(value, prefix+"_") {
		return rejectf(rejectionIdentifierInvalid, "%s must use %s_ namespace", label, prefix)
	}
	return nil
}

func validateIDSuffix(value string) error {
	if len(value) != 26 || value[0] < '0' || value[0] > '7' {
		return reject(rejectionIdentifierInvalid, "platform ID suffix is not a 128-bit Crockford value")
	}
	for _, character := range []byte(value[1:]) {
		if !isCrockford(character) {
			return reject(rejectionIdentifierInvalid, "platform ID suffix contains a non-Crockford byte")
		}
	}
	return nil
}

func isCrockford(value byte) bool {
	if value >= '0' && value <= '9' {
		return true
	}
	return value >= 'a' && value <= 'z' &&
		value != 'i' && value != 'l' && value != 'o' && value != 'u'
}

func validateEntityID(entityType EntityType, value, label string) error {
	prefix, ok := entityPrefixes[entityType]
	if !ok {
		return rejectf(rejectionValueInvalid, "%s entity_type %q is unsupported", label, entityType)
	}
	return validateTypedID(value, prefix, label+".entity_id")
}

func sameMessageSuffix(messageID, specializedID string) bool {
	return len(messageID) == 30 && len(specializedID) == 30 &&
		messageID[4:] == specializedID[4:]
}
