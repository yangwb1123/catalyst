package graphpricing

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

type sharedFixture struct {
	V           uint16       `json:"v"`
	CostVectors []costVector `json:"cost_vectors"`
	Input       struct {
		Model                       string `json:"model"`
		InputUSDMicrosPerTokenUnit  uint64 `json:"input_usd_micros_per_token_unit"`
		OutputUSDMicrosPerTokenUnit uint64 `json:"output_usd_micros_per_token_unit"`
		MaxInputTokens              uint64 `json:"max_input_tokens"`
	} `json:"input"`
	Expected struct {
		CanonicalDestinationPayloadJSON string `json:"canonical_destination_payload_json"`
		DestinationSHA256               string `json:"destination_sha256"`
		CanonicalPricingPayloadJSON     string `json:"canonical_pricing_payload_json"`
		CanonicalPricingSnapshotJSON    string `json:"canonical_pricing_snapshot_json"`
		PricingSnapshotSHA256           string `json:"pricing_snapshot_sha256"`
		WorstCost                       struct {
			MaxOutputTokens uint64 `json:"max_output_tokens"`
			USDMicros       uint64 `json:"usd_micros"`
		} `json:"worst_cost"`
	} `json:"expected"`
}

type costVector struct {
	Name                        string `json:"name"`
	InputUSDMicrosPerTokenUnit  uint64 `json:"input_usd_micros_per_token_unit"`
	OutputUSDMicrosPerTokenUnit uint64 `json:"output_usd_micros_per_token_unit"`
	MaxInputTokens              uint64 `json:"max_input_tokens"`
	MaxOutputTokens             uint64 `json:"max_output_tokens"`
	ExpectedUSDMicros           uint64 `json:"expected_usd_micros"`
}

func TestSharedPricingSnapshotGolden(t *testing.T) {
	fixture := readSharedFixture(t)
	value := mustBuild(t, fixtureInput(fixture))
	encoded, err := Marshal(value)
	if err != nil || string(encoded) != fixture.Expected.CanonicalPricingSnapshotJSON {
		t.Fatalf("canonical snapshot differs: %s / %v", encoded, err)
	}
	payload, err := canonicalBytes(payloadFrom(value))
	if err != nil || string(payload) != fixture.Expected.CanonicalPricingPayloadJSON ||
		value.PricingSnapshotSHA256 != fixture.Expected.PricingSnapshotSHA256 {
		t.Fatalf("pricing payload or digest differs: %s / %#v / %v", payload, value, err)
	}
	assertDestinationGolden(t, value, fixture)
	cost, err := WorstCostUSDMicros(value, fixture.Expected.WorstCost.MaxOutputTokens)
	if err != nil || cost != fixture.Expected.WorstCost.USDMicros {
		t.Fatalf("worst cost = %d / %v, want %d", cost, err, fixture.Expected.WorstCost.USDMicros)
	}
}

func TestSharedPricingCostVectors(t *testing.T) {
	fixture := readSharedFixture(t)
	assertRequiredCostVectors(t, fixture.CostVectors)
	for _, vector := range fixture.CostVectors {
		t.Run(vector.Name, func(t *testing.T) {
			value := mustBuild(t, Input{
				Model: fixture.Input.Model, MaxInputTokens: vector.MaxInputTokens,
				InputUSDMicrosPerTokenUnit:  vector.InputUSDMicrosPerTokenUnit,
				OutputUSDMicrosPerTokenUnit: vector.OutputUSDMicrosPerTokenUnit,
			})
			cost, err := WorstCostUSDMicros(value, vector.MaxOutputTokens)
			if err != nil || cost != vector.ExpectedUSDMicros {
				t.Fatalf("cost = %d / %v, want %d", cost, err, vector.ExpectedUSDMicros)
			}
		})
	}
}

func assertRequiredCostVectors(t *testing.T, vectors []costVector) {
	t.Helper()
	for _, required := range []string{
		"exact_division", "input_round_up_one_unit", "output_round_up_one_unit",
		"both_components_round_up_one_unit", "current_golden",
		"protocol_maximum_supported_values",
	} {
		found := false
		for _, vector := range vectors {
			found = found || vector.Name == required
		}
		if !found {
			t.Fatalf("shared cost matrix omits %q", required)
		}
	}
}

func assertDestinationGolden(t *testing.T, value Snapshot, fixture sharedFixture) {
	t.Helper()
	payload, err := canonicalBytes(destinationPayload{
		V: SnapshotVersion, ProviderKind: ProviderKind,
		Endpoint: RegisteredEndpoint, Model: value.Model,
	})
	if err != nil || string(payload) != fixture.Expected.CanonicalDestinationPayloadJSON ||
		value.DestinationSHA256 != fixture.Expected.DestinationSHA256 {
		t.Fatalf("destination payload or v1 digest differs: %s / %s / %v",
			payload, value.DestinationSHA256, err)
	}
}

func fixtureInput(fixture sharedFixture) Input {
	return Input{
		Model:                       fixture.Input.Model,
		InputUSDMicrosPerTokenUnit:  fixture.Input.InputUSDMicrosPerTokenUnit,
		OutputUSDMicrosPerTokenUnit: fixture.Input.OutputUSDMicrosPerTokenUnit,
		MaxInputTokens:              fixture.Input.MaxInputTokens,
	}
}

func readSharedFixture(t *testing.T) sharedFixture {
	t.Helper()
	path := filepath.Join("..", "..", "..", "docs", "contracts", "fixtures",
		"group-agent-node-pricing-snapshot-v1.json")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read shared pricing fixture: %v", err)
	}
	var fixture sharedFixture
	if err := json.Unmarshal(data, &fixture); err != nil || fixture.V != 1 {
		t.Fatalf("decode shared pricing fixture: %v / v=%d", err, fixture.V)
	}
	return fixture
}
