package graphdispatch

import (
	"fmt"
	"strings"
	"testing"
)

const (
	providerRequestDigestDomain = "forge.group-agent-node-provider-request.v1\x00"
	destinationDigestDomain     = "forge.group-agent-node-destination.v1\x00"
	controlEventDigestDomain    = "forge.group-agent-graph-run-control-event.v1\x00"
	dispatchRequestDigestDomain = "forge.group-agent-node-dispatch-request.v1\x00"
)

type providerRequestGolden struct {
	Include         []string              `json:"include"`
	Input           []providerInputGolden `json:"input"`
	Instructions    string                `json:"instructions"`
	MaxOutputTokens uint64                `json:"max_output_tokens"`
	Model           string                `json:"model"`
	Store           bool                  `json:"store"`
	Stream          bool                  `json:"stream"`
	Tools           []string              `json:"tools"`
}

type providerInputGolden struct {
	Content string `json:"content"`
	Role    string `json:"role"`
	Type    string `json:"type"`
}

type destinationGolden struct {
	V            uint16 `json:"v"`
	ProviderKind string `json:"provider_kind"`
	Endpoint     string `json:"endpoint"`
	Model        string `json:"model"`
}

type admissionEventGolden struct {
	V                     uint16 `json:"v"`
	GraphRunID            string `json:"graph_run_id"`
	Seq                   uint64 `json:"seq"`
	Type                  string `json:"type"`
	PreviousEventSHA256   string `json:"previous_event_sha256"`
	ControlSnapshotSHA256 string `json:"control_snapshot_sha256"`
	ContractID            string `json:"contract_id"`
	ContractSHA256        string `json:"contract_sha256"`
	ContractBytes         uint64 `json:"contract_bytes"`
	NodeID                string `json:"node_id"`
	Attempt               uint16 `json:"attempt"`
	RequestSHA256         string `json:"request_sha256"`
	ProjectLaneSHA256     string `json:"project_lane_sha256"`
	AdmittedAtMS          uint64 `json:"admitted_at_ms"`
}

type dispatchRequestGolden struct {
	V                       uint16 `json:"v"`
	CodecProtocolVersion    uint16 `json:"codec_protocol_version"`
	GraphRunID              string `json:"graph_run_id"`
	ContractID              string `json:"contract_id"`
	ContractSHA256          string `json:"contract_sha256"`
	ExpectedLastEventSeq    uint64 `json:"expected_last_event_seq"`
	ExpectedLastEventSHA256 string `json:"expected_last_event_sha256"`
	NodeID                  string `json:"node_id"`
	Attempt                 uint16 `json:"attempt"`
	ProjectLaneSHA256       string `json:"project_lane_sha256"`
	ProviderKind            string `json:"provider_kind"`
	Endpoint                string `json:"endpoint"`
	Model                   string `json:"model"`
	DestinationSHA256       string `json:"destination_sha256"`
	LogicalRequestSHA256    string `json:"logical_request_sha256"`
	PricingSnapshotSHA256   string `json:"pricing_snapshot_sha256"`
	RequestBodyBytes        uint64 `json:"request_body_bytes"`
	RequestBodySHA256       string `json:"request_body_sha256"`
}

func TestSharedNodeDispatchRequestGolden(t *testing.T) {
	fixture := readSharedFixture(t)
	snapshot := decodeFixtureSnapshot(t, fixture)
	contract, err := Build(snapshot, fixture.Input.ExecutionOptions.options())
	if err != nil {
		t.Fatalf("Build dispatch golden: %v", err)
	}
	bodySHA256 := assertProviderRequestGolden(t, contract, fixture)
	headSHA256 := assertAdmissionGolden(t, contract, fixture)
	assertDispatchGolden(t, contract, fixture, bodySHA256, headSHA256)
}

func assertProviderRequestGolden(t *testing.T, contract NodeExecutionContract, fixture sharedFixture) string {
	t.Helper()
	body := providerRequestGolden{
		Include:      []string{"reasoning.encrypted_content"},
		Input:        []providerInputGolden{{Content: contract.Request.UserPrompt, Role: "user", Type: "message"}},
		Instructions: contract.Request.SystemPrompt, MaxOutputTokens: contract.Budgets.MaxOutputTokens,
		Model: contract.Provider.Model, Store: contract.Provider.Store,
		Stream: contract.Provider.Stream, Tools: contract.Request.Tools,
	}
	encoded := mustCanonical(t, body)
	assertExactGolden(t, encoded, fixture.Expected.CanonicalProviderRequestBodyJSON)
	if uint64(len(encoded)) != fixture.Expected.ProviderRequestBytes {
		t.Fatalf("provider request bytes = %d, want %d", len(encoded), fixture.Expected.ProviderRequestBytes)
	}
	digest := rawDomainDigest(providerRequestDigestDomain, encoded)
	if digest != fixture.Expected.ProviderRequestSHA256 {
		t.Fatalf("provider request digest = %s", digest)
	}
	return digest
}

func assertAdmissionGolden(t *testing.T, contract NodeExecutionContract, fixture sharedFixture) string {
	t.Helper()
	event := admissionEventGolden{
		V: 2, GraphRunID: contract.GraphRunID, Seq: 2, Type: "node_execution_contract_admitted",
		PreviousEventSHA256:   contract.ExpectedLastEventSHA256,
		ControlSnapshotSHA256: contract.ControlSnapshotSHA256,
		ContractID:            contract.ContractID, ContractSHA256: contract.ContractSHA256,
		ContractBytes: uint64(len(fixture.Expected.CanonicalContractJSON)),
		NodeID:        contract.Node.NodeID, Attempt: contract.Node.Attempt,
		RequestSHA256:     contract.Request.RequestSHA256,
		ProjectLaneSHA256: contract.Node.ProjectLaneSHA256,
		AdmittedAtMS:      fixture.Expected.AdmittedAtMilliseconds,
	}
	encoded := mustCanonical(t, event)
	assertExactGolden(t, encoded, fixture.Expected.CanonicalAdmissionEventJSON)
	digest := rawDomainDigest(controlEventDigestDomain, encoded)
	if digest != fixture.Expected.AdmissionEventSHA256 {
		t.Fatalf("admission event digest = %s", digest)
	}
	return digest
}

func assertDispatchGolden(t *testing.T, contract NodeExecutionContract, fixture sharedFixture, body, head string) {
	t.Helper()
	destination := destinationGolden{V: 1, ProviderKind: contract.Provider.Kind, Endpoint: contract.Provider.Endpoint, Model: contract.Provider.Model}
	destinationJSON := mustCanonical(t, destination)
	assertExactGolden(t, destinationJSON, fixture.Expected.CanonicalDestinationPayloadJSON)
	destinationSHA256 := rawDomainDigest(destinationDigestDomain, destinationJSON)
	request := dispatchRequestGolden{
		V: 1, CodecProtocolVersion: 1, GraphRunID: contract.GraphRunID,
		ContractID: contract.ContractID, ContractSHA256: contract.ContractSHA256,
		ExpectedLastEventSeq: 2, ExpectedLastEventSHA256: head,
		NodeID: contract.Node.NodeID, Attempt: contract.Node.Attempt,
		ProjectLaneSHA256: contract.Node.ProjectLaneSHA256, ProviderKind: contract.Provider.Kind,
		Endpoint: contract.Provider.Endpoint, Model: contract.Provider.Model,
		DestinationSHA256: destinationSHA256, LogicalRequestSHA256: contract.Request.RequestSHA256,
		PricingSnapshotSHA256: contract.Budgets.PricingSnapshotSHA256,
		RequestBodyBytes:      fixture.Expected.ProviderRequestBytes, RequestBodySHA256: body,
	}
	encoded := mustCanonical(t, request)
	assertExactGolden(t, encoded, fixture.Expected.CanonicalDispatchRequestJSON)
	assertDispatchIdentity(t, encoded, destinationSHA256, fixture)
}

func assertDispatchIdentity(t *testing.T, encoded, destinationSHA256 string, fixture sharedFixture) {
	t.Helper()
	if destinationSHA256 != fixture.Expected.DestinationSHA256 {
		t.Fatalf("destination digest = %s", destinationSHA256)
	}
	digest := rawDomainDigest(dispatchRequestDigestDomain, encoded)
	if digest != fixture.Expected.DispatchRequestSHA256 {
		t.Fatalf("dispatch request digest = %s", digest)
	}
	identifier := fmt.Sprintf("node-dispatch-request-%s", digest)
	if identifier != fixture.Expected.DispatchRequestID {
		t.Fatalf("dispatch request identity = %s", identifier)
	}
}

func mustCanonical(t *testing.T, value any) string {
	t.Helper()
	encoded, err := canonicalBytes(value)
	if err != nil {
		t.Fatalf("canonical JSON: %v", err)
	}
	return string(encoded)
}

func assertExactGolden(t *testing.T, actual, expected string) {
	t.Helper()
	if actual != expected {
		t.Fatalf("exact golden differs:\nactual: %s\nexpected: %s", actual, expected)
	}
	if strings.HasSuffix(actual, "\n") {
		t.Fatal("exact golden has a trailing LF")
	}
}
