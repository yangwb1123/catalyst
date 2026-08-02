package graphpricing

import (
	"strings"
	"unicode"
	"unicode/utf8"
)

// Build validates operator-pinned rates and emits an effect-free snapshot.
func Build(input Input) (Snapshot, error) {
	if !validInput(input) {
		return Snapshot{}, errInvalidSnapshot
	}
	destination, err := destinationSHA256(input.Model)
	if err != nil {
		return Snapshot{}, errInvalidSnapshot
	}
	value := Snapshot{
		V: SnapshotVersion, PricingProtocolVersion: PricingProtocolVersion,
		ProviderKind: ProviderKind, Endpoint: RegisteredEndpoint, Model: input.Model,
		DestinationSHA256: destination, Currency: Currency, TokenUnit: TokenUnit,
		InputUSDMicrosPerTokenUnit:  input.InputUSDMicrosPerTokenUnit,
		OutputUSDMicrosPerTokenUnit: input.OutputUSDMicrosPerTokenUnit,
		MaxInputTokens:              input.MaxInputTokens, CostAlgorithm: CostAlgorithm,
		Provenance: Provenance, VendorAttestationPresent: false,
	}
	digest, err := domainDigest(snapshotDigestDomain, payloadFrom(value))
	if err != nil {
		return Snapshot{}, errInvalidSnapshot
	}
	value.PricingSnapshotSHA256 = digest
	if validate(value) != nil {
		return Snapshot{}, errInvalidSnapshot
	}
	return value, nil
}

func validate(value Snapshot) error {
	input := Input{
		Model:                       value.Model,
		InputUSDMicrosPerTokenUnit:  value.InputUSDMicrosPerTokenUnit,
		OutputUSDMicrosPerTokenUnit: value.OutputUSDMicrosPerTokenUnit,
		MaxInputTokens:              value.MaxInputTokens,
	}
	destination, err := destinationSHA256(value.Model)
	if err != nil || !validInput(input) || value.V != SnapshotVersion ||
		value.PricingProtocolVersion != PricingProtocolVersion ||
		value.ProviderKind != ProviderKind || value.Endpoint != RegisteredEndpoint ||
		value.DestinationSHA256 != destination || value.Currency != Currency ||
		value.TokenUnit != TokenUnit || value.CostAlgorithm != CostAlgorithm ||
		value.Provenance != Provenance || value.VendorAttestationPresent ||
		!isLowerHexDigest(value.PricingSnapshotSHA256) {
		return errInvalidSnapshot
	}
	digest, err := domainDigest(snapshotDigestDomain, payloadFrom(value))
	if err != nil || digest != value.PricingSnapshotSHA256 {
		return errInvalidSnapshot
	}
	return nil
}

func validInput(value Input) bool {
	return validModel(value.Model) && inRange(value.InputUSDMicrosPerTokenUnit, MaxRateUSDMicros) &&
		inRange(value.OutputUSDMicrosPerTokenUnit, MaxRateUSDMicros) &&
		inRange(value.MaxInputTokens, MaxInputTokens)
}

func validModel(value string) bool {
	return utf8.ValidString(value) && strings.TrimSpace(value) != "" &&
		len(value) <= MaxModelBytes && !strings.ContainsFunc(value, unsupportedCharacter)
}

func unsupportedCharacter(value rune) bool {
	return unicode.IsControl(value) || value == '\u061c' || value == '\u200e' ||
		value == '\u200f' || value >= '\u2028' && value <= '\u202e' ||
		value >= '\u2066' && value <= '\u2069'
}

func inRange(value, maximum uint64) bool {
	return value >= 1 && value <= maximum
}

func isLowerHexDigest(value string) bool {
	if len(value) != 64 {
		return false
	}
	for _, character := range []byte(value) {
		if (character < '0' || character > '9') &&
			(character < 'a' || character > 'f') {
			return false
		}
	}
	return true
}
