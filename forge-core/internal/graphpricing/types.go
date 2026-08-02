// Package graphpricing builds one effect-free, operator-asserted pricing
// snapshot for the registered Group Agent node destination.
package graphpricing

import "errors"

const (
	SnapshotVersion        uint16 = 1
	PricingProtocolVersion uint16 = 1
	MaxSnapshotBytes              = 16 * 1024
	MaxModelBytes                 = 128
	MaxRateUSDMicros       uint64 = 1_000_000_000_000
	MaxInputTokens         uint64 = 1_000_000_000
	MaxOutputTokens        uint64 = 32_768
	TokenUnit              uint64 = 1_000_000
)

const (
	ProviderKind            = "openai_responses"
	RegisteredEndpoint      = "https://api.openai.com/v1/responses"
	Currency                = "usd_micros"
	CostAlgorithm           = "ceil_each_token_component_v1"
	Provenance              = "operator_asserted"
	snapshotDigestDomain    = "forge.group-agent-node-pricing-snapshot.v1\x00"
	destinationDigestDomain = "forge.group-agent-node-destination.v1\x00"
)

var errInvalidSnapshot = errors.New("invalid Group Agent node pricing snapshot")

// Input is the complete caller-pinned pricing input. Provider and endpoint are
// deliberately absent because this protocol has one registered destination.
type Input struct {
	Model                       string
	InputUSDMicrosPerTokenUnit  uint64
	OutputUSDMicrosPerTokenUnit uint64
	MaxInputTokens              uint64
}

// Snapshot is the exact canonical version-1 pricing artifact.
type Snapshot struct {
	V                           uint16 `json:"v"`
	PricingProtocolVersion      uint16 `json:"pricing_protocol_version"`
	ProviderKind                string `json:"provider_kind"`
	Endpoint                    string `json:"endpoint"`
	Model                       string `json:"model"`
	DestinationSHA256           string `json:"destination_sha256"`
	Currency                    string `json:"currency"`
	TokenUnit                   uint64 `json:"token_unit"`
	InputUSDMicrosPerTokenUnit  uint64 `json:"input_usd_micros_per_token_unit"`
	OutputUSDMicrosPerTokenUnit uint64 `json:"output_usd_micros_per_token_unit"`
	MaxInputTokens              uint64 `json:"max_input_tokens"`
	CostAlgorithm               string `json:"cost_algorithm"`
	Provenance                  string `json:"provenance"`
	VendorAttestationPresent    bool   `json:"vendor_attestation_present"`
	PricingSnapshotSHA256       string `json:"pricing_snapshot_sha256"`
}

type snapshotPayload struct {
	V                           uint16 `json:"v"`
	PricingProtocolVersion      uint16 `json:"pricing_protocol_version"`
	ProviderKind                string `json:"provider_kind"`
	Endpoint                    string `json:"endpoint"`
	Model                       string `json:"model"`
	DestinationSHA256           string `json:"destination_sha256"`
	Currency                    string `json:"currency"`
	TokenUnit                   uint64 `json:"token_unit"`
	InputUSDMicrosPerTokenUnit  uint64 `json:"input_usd_micros_per_token_unit"`
	OutputUSDMicrosPerTokenUnit uint64 `json:"output_usd_micros_per_token_unit"`
	MaxInputTokens              uint64 `json:"max_input_tokens"`
	CostAlgorithm               string `json:"cost_algorithm"`
	Provenance                  string `json:"provenance"`
	VendorAttestationPresent    bool   `json:"vendor_attestation_present"`
}

type destinationPayload struct {
	V            uint16 `json:"v"`
	ProviderKind string `json:"provider_kind"`
	Endpoint     string `json:"endpoint"`
	Model        string `json:"model"`
}
