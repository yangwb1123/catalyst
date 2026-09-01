package platformcorecontract

import contractwire "forgeos/forge-core/internal/platformcorecontract/internal/wireprofile"

func validateText(value, label string, maximum int, nonempty bool) error {
	return contractwire.ValidateText(value, label, maximum, nonempty)
}

func validateLowerToken(value, label string, maximum int) error {
	return contractwire.ValidateLowerToken(value, label, maximum)
}

func validateSchemaName(value, label string) error {
	return contractwire.ValidateSchemaName(value, label)
}

func validateHash(value, label string) error {
	return contractwire.ValidateHash(value, label)
}

func validateUnixMS(value int64, label string) error {
	return contractwire.ValidateUnixMS(value, label)
}
