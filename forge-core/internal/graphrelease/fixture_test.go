package graphrelease

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/graphdispatch"
	"forgeos/forge-core/internal/graphplan"
)

type releaseFixtureFile struct {
	Input struct {
		CanonicalControlSnapshotJSON string                `json:"canonical_control_snapshot_json"`
		ExecutionOptions             releaseFixtureOptions `json:"execution_options"`
	} `json:"input"`
}

type releaseFixtureOptions struct {
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

func validReleaseFixture(t *testing.T) (ReleaseControl, []byte) {
	t.Helper()
	fixture := readReleaseFixture(t)
	snapshot, err := graphdispatch.DecodeControl(
		strings.NewReader(fixture.Input.CanonicalControlSnapshotJSON),
	)
	if err != nil {
		t.Fatalf("decode shared control: %v", err)
	}
	prepared := preparedEvent{
		V: 1, GraphRunID: snapshot.GraphRunID, Seq: 1, Type: "graph_run_prepared",
		GraphID: snapshot.GraphID, GraphManifestSHA256: snapshot.GraphManifestSHA256,
		PlanSHA256:               snapshot.CorePlanSHA256,
		SchedulerProtocolVersion: snapshot.SchedulerProtocolVersion, PreparedAtMS: 73,
	}
	preparedJSON := mustCanonicalTest(t, prepared)
	snapshot.LastEventSHA256 = rawDomainDigest(preparedEventDigestDomain, preparedJSON)
	snapshot.SnapshotSHA256 = snapshotDigestForTest(t, snapshot)
	contract, err := graphdispatch.Build(snapshot, fixture.Input.ExecutionOptions.options())
	if err != nil {
		t.Fatalf("build release contract: %v", err)
	}
	control := releaseControlForContract(t, snapshot, contract, preparedJSON)
	encoded := mustCanonicalTest(t, control)
	decoded, err := DecodeControl(bytes.NewReader(encoded))
	if err != nil {
		t.Fatalf("decode valid release fixture: %v\n%s", err, encoded)
	}
	return decoded, encoded
}

func releaseControlForContract(
	t *testing.T,
	snapshot graphdispatch.ControlSnapshot,
	contract graphdispatch.NodeExecutionContract,
	preparedJSON []byte,
) ReleaseControl {
	t.Helper()
	contractJSON, err := graphdispatch.MarshalContract(contract)
	if err != nil {
		t.Fatalf("marshal contract: %v", err)
	}
	contractHead := rawDomainDigest(preparedEventDigestDomain, preparedJSON)
	admission := admissionEventFor(contract, uint64(len(contractJSON)), contractHead)
	admissionJSON := mustCanonicalTest(t, admission)
	admissionHead := rawDomainDigest(controlEventDigestDomain, admissionJSON)
	body := providerBodyFor(t, contract)
	dispatch := dispatchRecordFor(t, contract, body, admissionHead)
	preparation := preparationEventFor(dispatch)
	preparationJSON := mustCanonicalTest(t, preparation)
	planJSON, err := graphplan.MarshalPlan(snapshot.Plan)
	if err != nil {
		t.Fatalf("marshal plan: %v", err)
	}
	control := ReleaseControl{
		V: 1, SchedulerProtocolVersion: 1, ReleaseControlProtocolVersion: 1,
		GraphRun: graphRunFor(snapshot, planJSON, preparedJSON, admissionJSON, preparationJSON),
		Plan:     snapshot.Plan, Manifest: snapshot.Manifest,
		JournalEvents:  []json.RawMessage{preparedJSON, admissionJSON, preparationJSON},
		ContractRecord: contractRecordFor(contract, uint64(len(contractJSON)), 80),
		Contract:       contract, DispatchRequest: dispatch, ProviderRequestJSON: string(body),
	}
	control.SnapshotSHA256 = mustDomainDigestTest(t, releaseControlDigestDomain, releasePayload(control))
	return control
}

func admissionEventFor(
	contract graphdispatch.NodeExecutionContract,
	contractBytes uint64,
	previous string,
) contractEvent {
	return contractEvent{
		V: 2, GraphRunID: contract.GraphRunID, Seq: 2,
		Type: "node_execution_contract_admitted", PreviousEventSHA256: previous,
		ControlSnapshotSHA256: contract.ControlSnapshotSHA256,
		ContractID:            contract.ContractID, ContractSHA256: contract.ContractSHA256,
		ContractBytes: contractBytes, NodeID: contract.Node.NodeID, Attempt: contract.Node.Attempt,
		RequestSHA256:     contract.Request.RequestSHA256,
		ProjectLaneSHA256: contract.Node.ProjectLaneSHA256, AdmittedAtMS: 80,
	}
}

func providerBodyFor(t *testing.T, contract graphdispatch.NodeExecutionContract) []byte {
	t.Helper()
	return mustCanonicalTest(t, providerRequest{
		Include: []string{"reasoning.encrypted_content"},
		Input: []providerRequestInput{{
			Content: contract.Request.UserPrompt, Role: "user", Type: "message",
		}},
		Instructions:    contract.Request.SystemPrompt,
		MaxOutputTokens: contract.Budgets.MaxOutputTokens, Model: contract.Provider.Model,
		Store: false, Stream: true, Tools: []string{},
	})
}

func dispatchRecordFor(
	t *testing.T,
	contract graphdispatch.NodeExecutionContract,
	body []byte,
	previous string,
) NodeDispatchRequestRecord {
	t.Helper()
	destination, err := domainDigest(destinationDigestDomain, destinationPayload{
		V: 1, ProviderKind: contract.Provider.Kind,
		Endpoint: contract.Provider.Endpoint, Model: contract.Provider.Model,
	})
	if err != nil {
		t.Fatalf("destination digest: %v", err)
	}
	record := NodeDispatchRequestRecord{
		V: 1, GraphRunID: contract.GraphRunID, ContractID: contract.ContractID,
		NodeID: contract.Node.NodeID, Attempt: contract.Node.Attempt,
		ContractSHA256: contract.ContractSHA256, RequestSHA256: contract.Request.RequestSHA256,
		ProjectLaneSHA256: contract.Node.ProjectLaneSHA256, Provider: contract.Provider.Kind,
		Endpoint: contract.Provider.Endpoint, Model: contract.Provider.Model,
		PricingSnapshotSHA256: contract.Budgets.PricingSnapshotSHA256,
		ProviderRequestSHA256: rawDomainDigest(providerRequestDomain, body),
		ProviderRequestBytes:  uint64(len(body)), DestinationSHA256: destination,
		CodecProtocolVersion: 1, ExpectedLastEventSeq: 2,
		ExpectedLastEventSHA256: previous, CreatedAtMS: 90,
	}
	record.DispatchRequestSHA256 = mustDomainDigestTest(t, dispatchRequestDomain, dispatchPayloadFrom(record))
	record.DispatchRequestID = "node-dispatch-request-" + record.DispatchRequestSHA256
	return record
}

func preparationEventFor(record NodeDispatchRequestRecord) dispatchEvent {
	return dispatchEvent{
		V: 3, GraphRunID: record.GraphRunID, Seq: 3, Type: "node_dispatch_request_prepared",
		PreviousEventSHA256: record.ExpectedLastEventSHA256,
		ContractID:          record.ContractID, ContractSHA256: record.ContractSHA256,
		DispatchRequestID:     record.DispatchRequestID,
		DispatchRequestSHA256: record.DispatchRequestSHA256,
		RequestBodySHA256:     record.ProviderRequestSHA256,
		RequestBodyBytes:      record.ProviderRequestBytes,
		LogicalRequestSHA256:  record.RequestSHA256, NodeID: record.NodeID,
		Attempt: record.Attempt, ProjectLaneSHA256: record.ProjectLaneSHA256,
		CodecProtocolVersion: record.CodecProtocolVersion, ProviderKind: record.Provider,
		DestinationSHA256:     record.DestinationSHA256,
		PricingSnapshotSHA256: record.PricingSnapshotSHA256, PreparedAtMS: record.CreatedAtMS,
	}
}

func contractRecordFor(
	contract graphdispatch.NodeExecutionContract,
	contractBytes, createdAt uint64,
) NodeExecutionContractRecord {
	return NodeExecutionContractRecord{
		V: 1, ContractID: contract.ContractID, GraphRunID: contract.GraphRunID,
		NodeID: contract.Node.NodeID, Attempt: contract.Node.Attempt,
		ControlSnapshotSHA256: contract.ControlSnapshotSHA256,
		ContractSHA256:        contract.ContractSHA256, ContractBytes: contractBytes,
		RequestSHA256:           contract.Request.RequestSHA256,
		ProjectLaneSHA256:       contract.Node.ProjectLaneSHA256,
		ExpectedLastEventSeq:    contract.ExpectedLastEventSeq,
		ExpectedLastEventSHA256: contract.ExpectedLastEventSHA256, CreatedAtMS: createdAt,
	}
}

func graphRunFor(
	snapshot graphdispatch.ControlSnapshot,
	planJSON, first, second, third []byte,
) GraphRunRecord {
	return GraphRunRecord{
		V: 3, GraphRunID: snapshot.GraphRunID, GraphID: snapshot.GraphID,
		Status:                   "awaiting_dispatch_authorization",
		SourceSnapshotSHA256:     snapshot.SourceSnapshotSHA256,
		GraphManifestSHA256:      snapshot.GraphManifestSHA256,
		SchedulerProtocolVersion: snapshot.SchedulerProtocolVersion,
		PlanSHA256:               snapshot.CorePlanSHA256, PlanBytes: uint64(len(planJSON)),
		NodeCount: uint64(len(snapshot.Plan.AuthoredNodeIDs)),
		WaveCount: uint64(len(snapshot.Plan.Waves)), ExecutionContractPresent: true,
		DispatchRequestPresent: true, DispatchAuthorityReleased: false, LastEventSeq: 3,
		JournalBytes: uint64(len(first) + len(second) + len(third)), CreatedAtMS: 73,
	}
}

func readReleaseFixture(t *testing.T) releaseFixtureFile {
	t.Helper()
	path := filepath.Join("..", "..", "..", "docs", "contracts", "fixtures",
		"group-agent-node-execution-contract-v1.json")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read shared fixture: %v", err)
	}
	var fixture releaseFixtureFile
	if err := json.Unmarshal(data, &fixture); err != nil {
		t.Fatalf("decode shared fixture: %v", err)
	}
	return fixture
}

func (value releaseFixtureOptions) options() graphdispatch.ExecutionOptions {
	return graphdispatch.ExecutionOptions{
		Endpoint: value.Endpoint, Model: value.Model, MaxOutputTokens: value.MaxOutputTokens,
		MaxModelOutputBytes: value.MaxModelOutputBytes, MaxModelEvents: value.MaxModelEvents,
		TimeoutMilliseconds:   value.TimeoutMilliseconds,
		MaxCostUSDMicros:      value.MaxCostUSDMicros,
		PricingSnapshotSHA256: value.PricingSnapshotSHA256,
		MaxResultBytes:        value.MaxResultBytes,
	}
}

type controlSnapshotPayloadTest struct {
	V                         uint16                      `json:"v"`
	SchedulerProtocolVersion  uint16                      `json:"scheduler_protocol_version"`
	GraphRunVersion           uint16                      `json:"graph_run_version"`
	GraphRunID                string                      `json:"graph_run_id"`
	GraphID                   string                      `json:"graph_id"`
	SourceSnapshotSHA256      string                      `json:"source_snapshot_sha256"`
	GraphManifestSHA256       string                      `json:"graph_manifest_sha256"`
	CorePlanSHA256            string                      `json:"core_plan_sha256"`
	LastEventSeq              uint64                      `json:"last_event_seq"`
	LastEventSHA256           string                      `json:"last_event_sha256"`
	ExecutionContractPresent  bool                        `json:"execution_contract_present"`
	DispatchAuthorityReleased bool                        `json:"dispatch_authority_released"`
	Plan                      graphplan.Plan              `json:"plan"`
	Manifest                  graphdispatch.GraphManifest `json:"manifest"`
}

func snapshotDigestForTest(t *testing.T, snapshot graphdispatch.ControlSnapshot) string {
	t.Helper()
	payload := controlSnapshotPayloadTest{
		V: snapshot.V, SchedulerProtocolVersion: snapshot.SchedulerProtocolVersion,
		GraphRunVersion: snapshot.GraphRunVersion, GraphRunID: snapshot.GraphRunID,
		GraphID: snapshot.GraphID, SourceSnapshotSHA256: snapshot.SourceSnapshotSHA256,
		GraphManifestSHA256: snapshot.GraphManifestSHA256,
		CorePlanSHA256:      snapshot.CorePlanSHA256, LastEventSeq: snapshot.LastEventSeq,
		LastEventSHA256:           snapshot.LastEventSHA256,
		ExecutionContractPresent:  snapshot.ExecutionContractPresent,
		DispatchAuthorityReleased: snapshot.DispatchAuthorityReleased,
		Plan:                      snapshot.Plan, Manifest: snapshot.Manifest,
	}
	return mustDomainDigestTest(t, "forge.group-agent-graph-control-snapshot.v1\x00", payload)
}

func mustCanonicalTest(t *testing.T, value any) []byte {
	t.Helper()
	encoded, err := canonicalBytes(value)
	if err != nil {
		t.Fatalf("canonical JSON: %v", err)
	}
	return encoded
}

func mustDomainDigestTest(t *testing.T, domain string, value any) string {
	t.Helper()
	digest, err := domainDigest(domain, value)
	if err != nil {
		t.Fatalf("domain digest: %v", err)
	}
	return digest
}
