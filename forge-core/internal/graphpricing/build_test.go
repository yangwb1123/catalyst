package graphpricing

import (
	"math"
	"strings"
	"testing"
)

func TestBuildFreezesRegisteredDestinationAndOperatorPolicy(t *testing.T) {
	value := mustBuild(t, Input{
		Model: "model<safe>", InputUSDMicrosPerTokenUnit: 2_000_000,
		OutputUSDMicrosPerTokenUnit: 10_000_000, MaxInputTokens: 400_000,
	})
	if value.ProviderKind != ProviderKind || value.Endpoint != RegisteredEndpoint ||
		value.Currency != Currency || value.TokenUnit != TokenUnit ||
		value.CostAlgorithm != CostAlgorithm || value.Provenance != Provenance ||
		value.VendorAttestationPresent || !isLowerHexDigest(value.DestinationSHA256) ||
		!isLowerHexDigest(value.PricingSnapshotSHA256) {
		t.Fatalf("snapshot policy is not fully frozen: %#v", value)
	}
	encoded, err := Marshal(value)
	if err != nil || strings.Contains(string(encoded), `\u003c`) ||
		!strings.Contains(string(encoded), `"model":"model<safe>"`) {
		t.Fatalf("canonical JSON changed safe model bytes: %q / %v", encoded, err)
	}
	value.VendorAttestationPresent = true
	if _, err := Marshal(value); err == nil {
		t.Fatal("vendor attestation cannot be asserted by this protocol")
	}
}

func TestBuildRejectsUnsafeOrOutOfRangeInputs(t *testing.T) {
	valid := Input{
		Model: "gpt-fixture", InputUSDMicrosPerTokenUnit: 1,
		OutputUSDMicrosPerTokenUnit: 1, MaxInputTokens: 1,
	}
	tests := []struct {
		name   string
		mutate func(*Input)
	}{
		{"empty model", func(value *Input) { value.Model = " " }},
		{"long model", func(value *Input) { value.Model = strings.Repeat("m", MaxModelBytes+1) }},
		{"control model", func(value *Input) { value.Model = "model\nsecret" }},
		{"bidi model", func(value *Input) { value.Model = "model\u202e" }},
		{"zero input rate", func(value *Input) { value.InputUSDMicrosPerTokenUnit = 0 }},
		{"high input rate", func(value *Input) { value.InputUSDMicrosPerTokenUnit = MaxRateUSDMicros + 1 }},
		{"zero output rate", func(value *Input) { value.OutputUSDMicrosPerTokenUnit = 0 }},
		{"high output rate", func(value *Input) { value.OutputUSDMicrosPerTokenUnit = MaxRateUSDMicros + 1 }},
		{"zero input tokens", func(value *Input) { value.MaxInputTokens = 0 }},
		{"high input tokens", func(value *Input) { value.MaxInputTokens = MaxInputTokens + 1 }},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			input := valid
			test.mutate(&input)
			if _, err := Build(input); err == nil {
				t.Fatal("invalid pricing input was accepted")
			}
		})
	}
}

func TestWorstCostUsesIndependentCeilAndCheckedWideArithmetic(t *testing.T) {
	rounding := mustBuild(t, Input{
		Model: "rounding", InputUSDMicrosPerTokenUnit: 1,
		OutputUSDMicrosPerTokenUnit: 1, MaxInputTokens: 1,
	})
	if cost, err := WorstCostUSDMicros(rounding, 1); err != nil || cost != 2 {
		t.Fatalf("independent ceil cost = %d / %v, want 2", cost, err)
	}
	if _, err := ceilTokenComponent(math.MaxUint64, math.MaxUint64, 1); err == nil {
		t.Fatal("component quotient outside uint64 must fail instead of wrapping")
	}
	if _, err := maximumCostUSDMicros(
		1, 1, math.MaxUint64, math.MaxUint64, 1,
	); err == nil {
		t.Fatal("out-of-protocol component addition must fail instead of wrapping")
	}
	if _, err := maximumCostUSDMicros(
		math.MaxUint64, 1, math.MaxUint64, 1, 1,
	); err == nil {
		t.Fatal("out-of-protocol component narrowing must fail instead of wrapping")
	}
	if _, err := WorstCostUSDMicros(rounding, 0); err == nil {
		t.Fatal("zero output-token bound must fail")
	}
	if _, err := WorstCostUSDMicros(rounding, MaxOutputTokens+1); err == nil {
		t.Fatal("output-token bound above the protocol maximum must fail")
	}
}

func mustBuild(t *testing.T, input Input) Snapshot {
	t.Helper()
	value, err := Build(input)
	if err != nil {
		t.Fatalf("Build: %v", err)
	}
	return value
}
