package graphdispatch

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

type sharedFixture struct {
	V     uint16 `json:"v"`
	Input struct {
		CanonicalControlSnapshotJSON string         `json:"canonical_control_snapshot_json"`
		ExecutionOptions             fixtureOptions `json:"execution_options"`
	} `json:"input"`
	Expected struct {
		SelectedNodeID                   string `json:"selected_node_id"`
		CanonicalUserPromptJSON          string `json:"canonical_user_prompt_json"`
		CanonicalRequestPayloadJSON      string `json:"canonical_request_payload_json"`
		RequestSHA256                    string `json:"request_sha256"`
		CanonicalContractPayloadJSON     string `json:"canonical_contract_payload_json"`
		ContractSHA256                   string `json:"contract_sha256"`
		ContractID                       string `json:"contract_id"`
		CanonicalProviderRequestBodyJSON string `json:"canonical_provider_request_body_json"`
		ProviderRequestBytes             uint64 `json:"provider_request_bytes"`
		ProviderRequestSHA256            string `json:"provider_request_sha256"`
		CanonicalDestinationPayloadJSON  string `json:"canonical_destination_payload_json"`
		DestinationSHA256                string `json:"destination_sha256"`
		AdmittedAtMilliseconds           uint64 `json:"admitted_at_ms"`
		CanonicalAdmissionEventJSON      string `json:"canonical_admission_event_json"`
		AdmissionEventSHA256             string `json:"admission_event_sha256"`
		CanonicalDispatchRequestJSON     string `json:"canonical_dispatch_request_payload_json"`
		DispatchRequestSHA256            string `json:"dispatch_request_sha256"`
		DispatchRequestID                string `json:"dispatch_request_id"`
		CanonicalContractJSON            string `json:"canonical_contract_json"`
	} `json:"expected"`
}

type fixtureOptions struct {
	Endpoint              string `json:"endpoint"`
	Model                 string `json:"model"`
	MaxOutputTokens       uint64 `json:"max_output_tokens"`
	MaxModelOutputBytes   uint64 `json:"max_model_output_bytes"`
	MaxModelEvents        uint64 `json:"max_model_events"`
	TimeoutMilliseconds   uint64 `json:"timeout_ms"`
	MaxCostUSDMicros      uint64 `json:"max_cost_usd_micros"`
	PricingSnapshotSHA256 string `json:"pricing_snapshot_sha256"`
	MaxResultBytes        uint64 `json:"max_result_bytes"`
}

func TestSharedNodeExecutionContractGolden(t *testing.T) {
	fixture := readSharedFixture(t)
	snapshot := decodeFixtureSnapshot(t, fixture)
	contract, err := Build(snapshot, fixture.Input.ExecutionOptions.options())
	if err != nil {
		t.Fatalf("Build golden: %v", err)
	}
	assertGoldenRequest(t, contract, fixture)
	assertGoldenContract(t, contract, fixture)
}

func assertGoldenRequest(
	t *testing.T,
	contract NodeExecutionContract,
	fixture sharedFixture,
) {
	t.Helper()
	if contract.Node.NodeID != fixture.Expected.SelectedNodeID {
		t.Fatalf("selected node = %q, want %q", contract.Node.NodeID, fixture.Expected.SelectedNodeID)
	}
	if contract.Request.UserPrompt != fixture.Expected.CanonicalUserPromptJSON {
		t.Fatalf("user Prompt = %q", contract.Request.UserPrompt)
	}
	payload, err := canonicalBytes(requestPayloadFrom(contract.Request))
	if err != nil || string(payload) != fixture.Expected.CanonicalRequestPayloadJSON {
		t.Fatalf("request payload = %s; err=%v", payload, err)
	}
	if contract.Request.RequestSHA256 != fixture.Expected.RequestSHA256 {
		t.Fatalf("request digest = %s", contract.Request.RequestSHA256)
	}
}

func assertGoldenContract(
	t *testing.T,
	contract NodeExecutionContract,
	fixture sharedFixture,
) {
	t.Helper()
	payload, err := canonicalBytes(contractPayloadFrom(contract))
	if err != nil || string(payload) != fixture.Expected.CanonicalContractPayloadJSON {
		t.Fatalf("contract payload differs; err=%v\n%s", err, payload)
	}
	encoded, err := MarshalContract(contract)
	if err != nil || string(encoded) != fixture.Expected.CanonicalContractJSON {
		t.Fatalf("contract differs; err=%v\n%s", err, encoded)
	}
	if contract.ContractSHA256 != fixture.Expected.ContractSHA256 ||
		contract.ContractID != fixture.Expected.ContractID {
		t.Fatalf("contract identity = %s / %s", contract.ContractID, contract.ContractSHA256)
	}
	if strings.Contains(string(encoded), `\u003c`) || strings.HasSuffix(string(encoded), "\n") {
		t.Fatalf("contract is not exact non-HTML-escaped canonical JSON: %q", encoded)
	}
}

func readSharedFixture(t *testing.T) sharedFixture {
	t.Helper()
	path := filepath.Join(
		"..", "..", "..", "docs", "contracts", "fixtures",
		"group-agent-node-execution-contract-v1.json",
	)
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read fixture: %v", err)
	}
	var fixture sharedFixture
	if err := json.Unmarshal(data, &fixture); err != nil || fixture.V != 1 {
		t.Fatalf("decode fixture: v=%d err=%v", fixture.V, err)
	}
	return fixture
}

func decodeFixtureSnapshot(t *testing.T, fixture sharedFixture) ControlSnapshot {
	t.Helper()
	snapshot, err := DecodeControl(strings.NewReader(fixture.Input.CanonicalControlSnapshotJSON))
	if err != nil {
		t.Fatalf("DecodeControl fixture: %v", err)
	}
	return snapshot
}

func (options fixtureOptions) options() ExecutionOptions {
	return ExecutionOptions{
		Endpoint: options.Endpoint, Model: options.Model,
		MaxOutputTokens:       options.MaxOutputTokens,
		MaxModelOutputBytes:   options.MaxModelOutputBytes,
		MaxModelEvents:        options.MaxModelEvents,
		TimeoutMilliseconds:   options.TimeoutMilliseconds,
		MaxCostUSDMicros:      options.MaxCostUSDMicros,
		PricingSnapshotSHA256: options.PricingSnapshotSHA256,
		MaxResultBytes:        options.MaxResultBytes,
	}
}
