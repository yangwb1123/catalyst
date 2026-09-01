package receipt

import "strings"

func validateAdapterVersion(value string) error {
	if len(value) < 5 || len(value) > 32 {
		return reject(rejectionValueInvalid, "adapter_version must be a bounded major.minor.patch value")
	}
	parts := strings.Split(value, ".")
	if len(parts) != 3 {
		return reject(rejectionValueInvalid, "adapter_version must contain major.minor.patch")
	}
	for _, part := range parts {
		if !validVersionPart(part) {
			return reject(rejectionValueInvalid, "adapter_version components must be canonical decimal integers")
		}
	}
	return nil
}

func validVersionPart(value string) bool {
	if value == "" || len(value) > 9 || len(value) > 1 && value[0] == '0' {
		return false
	}
	for _, character := range []byte(value) {
		if character < '0' || character > '9' {
			return false
		}
	}
	return true
}
