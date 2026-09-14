package deviceplacement

import (
	"strings"
	"unicode"
	"unicode/utf8"
)

func validateRequest(value Request) error {
	if value.SchemaVersion != RequestSchemaVersion || value.EvaluatedAtMS <= 0 ||
		!validOwner(value.Owner) || value.MaxSnapshotAgeMS <= 0 || value.MaxSnapshotAgeMS > MaxSnapshotAgeMS ||
		len(value.Devices) > MaxDevices || validateRequirements(value.Requirements) != nil {
		return errInvalidRequest
	}
	seen := make(map[string]struct{}, len(value.Devices))
	for _, device := range value.Devices {
		if !validDeviceDeclaration(device) {
			return errInvalidRequest
		}
		if _, exists := seen[device.DeviceID]; exists {
			return errInvalidRequest
		}
		seen[device.DeviceID] = struct{}{}
	}
	return nil
}

func validateRequirements(value Requirements) error {
	if !validToken(value.OS) || !validToken(value.Architecture) || value.MinCPUCores == 0 ||
		value.MinMemoryBytes == 0 || value.MinStorageBytes == 0 || !validToken(value.Runtime) ||
		!validTrustZone(value.MinimumTrustZone) || !validSandboxLevel(value.SandboxFloor) ||
		value.ConcurrencySlots == 0 || len(value.DataResidencyZones) == 0 ||
		!validUniqueTokens(value.DataResidencyZones, MaxArrayItems, validZone) ||
		!validGPURequirement(value.GPU) {
		return errInvalidRequest
	}
	return nil
}

func validGPURequirement(value GPURequirement) bool {
	if value.Required {
		return value.Runtime == "" || validToken(value.Runtime)
	}
	return value.MinMemoryBytes == 0 && value.Runtime == ""
}

func validDeviceDeclaration(value Device) bool {
	return validDeviceID(value.DeviceID) && validOwner(value.Owner) && validApproval(value.ApprovalState) &&
		validCordonState(value.CordonState) && validLiveness(value.Liveness) && value.SnapshotObservedAtMS >= 0 &&
		value.LeaseExpiresAtMS >= 0 && validToken(value.OS) && validToken(value.Architecture) &&
		validUniqueTokens(value.Runtimes, MaxArrayItems, validToken) && validGPUDeclaration(value.GPU) &&
		validUniqueTokens(value.DataResidencyZones, MaxArrayItems, validZone) &&
		validDeviceTrustZone(value.TrustZone) && validUniqueTokens(value.SandboxLevels, MaxArrayItems, validSandboxLevel)
}

func validGPUDeclaration(value GPUDeclaration) bool {
	if value.Present {
		return value.Runtime == "" || validToken(value.Runtime)
	}
	return value.MemoryBytes == 0 && value.Runtime == ""
}

func validOwner(value Owner) bool {
	return validOwnerPart(value.Issuer) && validOwnerPart(value.Subject) && validOwnerPart(value.TenantID)
}

func validOwnerPart(value string) bool {
	return utf8.ValidString(value) && value != "" && len(value) <= MaxOwnerPartBytes &&
		strings.TrimSpace(value) == value && !strings.ContainsFunc(value, unicode.IsControl)
}

func validDeviceID(value string) bool {
	if len(value) == 0 || len(value) > MaxTokenBytes {
		return false
	}
	for index, char := range value {
		if !(char >= 'a' && char <= 'z' || char >= 'A' && char <= 'Z' || char >= '0' && char <= '9' ||
			index > 0 && (char == '.' || char == '_' || char == ':' || char == '-')) {
			return false
		}
	}
	return true
}

func validToken(value string) bool {
	if len(value) == 0 || len(value) > MaxTokenBytes {
		return false
	}
	for _, char := range value {
		if !(char >= 'a' && char <= 'z' || char >= 'A' && char <= 'Z' || char >= '0' && char <= '9' ||
			strings.ContainsRune("._:+/-", char)) {
			return false
		}
	}
	return true
}

func validZone(value string) bool {
	if len(value) == 0 || len(value) > 64 {
		return false
	}
	for _, char := range value {
		if !(char >= 'a' && char <= 'z' || char >= 'A' && char <= 'Z' || char >= '0' && char <= '9' ||
			strings.ContainsRune("._-", char)) {
			return false
		}
	}
	return true
}

func validUniqueTokens(values []string, maximum int, valid func(string) bool) bool {
	if len(values) > maximum {
		return false
	}
	seen := make(map[string]struct{}, len(values))
	for _, value := range values {
		if !valid(value) {
			return false
		}
		if _, exists := seen[value]; exists {
			return false
		}
		seen[value] = struct{}{}
	}
	return true
}

func validTrustZone(value string) bool {
	switch value {
	case "untrusted", "low", "standard", "high", "restricted":
		return true
	default:
		return false
	}
}

func validDeviceTrustZone(value string) bool {
	return validTrustZone(value) || value == "unknown"
}

func validSandboxLevel(value string) bool {
	switch value {
	case "process", "container", "microvm":
		return true
	default:
		return false
	}
}

func validApproval(value string) bool {
	switch value {
	case "approved", "pending", "revoked", "unknown":
		return true
	default:
		return false
	}
}

func validCordonState(value string) bool {
	switch value {
	case "clear", "cordoned", "unknown":
		return true
	default:
		return false
	}
}

func validLiveness(value string) bool {
	switch value {
	case "online", "offline", "unknown":
		return true
	default:
		return false
	}
}
