package graphpricing

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"math/big"
)

// Marshal returns exact compact canonical JSON without a trailing LF.
func Marshal(value Snapshot) ([]byte, error) {
	if validate(value) != nil {
		return nil, errInvalidSnapshot
	}
	encoded, err := canonicalBytes(value)
	if err != nil || len(encoded) == 0 || len(encoded) > MaxSnapshotBytes {
		return nil, errInvalidSnapshot
	}
	return encoded, nil
}

// WorstCostUSDMicros computes the conservative maximum using checked integer
// arithmetic and independent ceiling for input and output token components.
func WorstCostUSDMicros(value Snapshot, maxOutputTokens uint64) (uint64, error) {
	if validate(value) != nil || !inRange(maxOutputTokens, MaxOutputTokens) {
		return 0, errInvalidSnapshot
	}
	return maximumCostUSDMicros(
		value.MaxInputTokens, maxOutputTokens, value.InputUSDMicrosPerTokenUnit,
		value.OutputUSDMicrosPerTokenUnit, value.TokenUnit,
	)
}

func maximumCostUSDMicros(inputTokens, outputTokens, inputRate, outputRate, unit uint64) (uint64, error) {
	input, err := ceilTokenComponent(
		inputRate, inputTokens, unit,
	)
	if err != nil {
		return 0, errInvalidSnapshot
	}
	output, err := ceilTokenComponent(
		outputRate, outputTokens, unit,
	)
	if err != nil || ^uint64(0)-input < output {
		return 0, errInvalidSnapshot
	}
	return input + output, nil
}

func ceilTokenComponent(rate, tokens, unit uint64) (uint64, error) {
	if rate == 0 || tokens == 0 || unit == 0 {
		return 0, errInvalidSnapshot
	}
	product := new(big.Int).Mul(new(big.Int).SetUint64(rate), new(big.Int).SetUint64(tokens))
	quotient, remainder := new(big.Int), new(big.Int)
	quotient.QuoRem(product, new(big.Int).SetUint64(unit), remainder)
	if remainder.Sign() != 0 {
		quotient.Add(quotient, big.NewInt(1))
	}
	if !quotient.IsUint64() {
		return 0, errInvalidSnapshot
	}
	return quotient.Uint64(), nil
}

func destinationSHA256(model string) (string, error) {
	return domainDigest(destinationDigestDomain, destinationPayload{
		V: SnapshotVersion, ProviderKind: ProviderKind,
		Endpoint: RegisteredEndpoint, Model: model,
	})
}

func payloadFrom(value Snapshot) snapshotPayload {
	return snapshotPayload{
		V: value.V, PricingProtocolVersion: value.PricingProtocolVersion,
		ProviderKind: value.ProviderKind, Endpoint: value.Endpoint, Model: value.Model,
		DestinationSHA256: value.DestinationSHA256, Currency: value.Currency,
		TokenUnit:                   value.TokenUnit,
		InputUSDMicrosPerTokenUnit:  value.InputUSDMicrosPerTokenUnit,
		OutputUSDMicrosPerTokenUnit: value.OutputUSDMicrosPerTokenUnit,
		MaxInputTokens:              value.MaxInputTokens, CostAlgorithm: value.CostAlgorithm,
		Provenance:               value.Provenance,
		VendorAttestationPresent: value.VendorAttestationPresent,
	}
}

func canonicalBytes(value any) ([]byte, error) {
	var buffer bytes.Buffer
	encoder := json.NewEncoder(&buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(value); err != nil {
		return nil, errInvalidSnapshot
	}
	encoded := buffer.Bytes()
	if len(encoded) == 0 || encoded[len(encoded)-1] != '\n' {
		return nil, errInvalidSnapshot
	}
	return append([]byte(nil), encoded[:len(encoded)-1]...), nil
}

func domainDigest(domain string, value any) (string, error) {
	encoded, err := canonicalBytes(value)
	if err != nil {
		return "", errInvalidSnapshot
	}
	digest := sha256.New()
	_, _ = digest.Write([]byte(domain))
	_, _ = digest.Write(encoded)
	return hex.EncodeToString(digest.Sum(nil)), nil
}
