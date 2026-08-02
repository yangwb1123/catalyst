package graphterminal

import (
	"encoding/json"
	"testing"

	"forgeos/forge-core/internal/graphdispatch"
	"forgeos/forge-core/internal/graphplan"
	"forgeos/forge-core/internal/graphrelease"
)

type controlSnapshotPayloadFixture struct {
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

type providerRequestFixture struct {
	Include         []string               `json:"include"`
	Input           []providerInputFixture `json:"input"`
	Instructions    string                 `json:"instructions"`
	MaxOutputTokens uint64                 `json:"max_output_tokens"`
	Model           string                 `json:"model"`
	Store           bool                   `json:"store"`
	Stream          bool                   `json:"stream"`
	Tools           []string               `json:"tools"`
}

type providerInputFixture struct {
	Content string `json:"content"`
	Role    string `json:"role"`
	Type    string `json:"type"`
}

type destinationPayloadFixture struct {
	V            uint16 `json:"v"`
	ProviderKind string `json:"provider_kind"`
	Endpoint     string `json:"endpoint"`
	Model        string `json:"model"`
}

type dispatchPayloadFixture struct {
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

type releasePayloadFixture struct {
	V                             uint16                                   `json:"v"`
	SchedulerProtocolVersion      uint16                                   `json:"scheduler_protocol_version"`
	ReleaseControlProtocolVersion uint16                                   `json:"release_control_protocol_version"`
	GraphRun                      graphrelease.GraphRunRecord              `json:"graph_run"`
	Plan                          graphplan.Plan                           `json:"plan"`
	Manifest                      graphdispatch.GraphManifest              `json:"manifest"`
	JournalEvents                 []json.RawMessage                        `json:"journal_events"`
	ContractRecord                graphrelease.NodeExecutionContractRecord `json:"contract_record"`
	Contract                      graphdispatch.NodeExecutionContract      `json:"contract"`
	DispatchRequest               graphrelease.NodeDispatchRequestRecord   `json:"dispatch_request"`
	ProviderRequestJSON           string                                   `json:"provider_request_json"`
}

func buildControlSnapshot(t *testing.T, plan graphplan.Plan, manifest graphdispatch.GraphManifest, prepared []byte) graphdispatch.ControlSnapshot {
	t.Helper()
	value := graphdispatch.ControlSnapshot{
		V: 1, SchedulerProtocolVersion: 1, GraphRunVersion: 1,
		GraphRunID: "graph-run-terminal-fixture", GraphID: plan.GraphID,
		SourceSnapshotSHA256: manifest.Source.SnapshotSHA256,
		GraphManifestSHA256:  plan.GraphManifestSHA256, CorePlanSHA256: plan.PlanSHA256,
		LastEventSeq: 1, LastEventSHA256: rawDomainDigest(preparedEventDomain, prepared),
		ExecutionContractPresent: false, DispatchAuthorityReleased: false,
		Plan: plan, Manifest: manifest,
	}
	payload := controlSnapshotPayloadFixture{
		V: value.V, SchedulerProtocolVersion: value.SchedulerProtocolVersion,
		GraphRunVersion: value.GraphRunVersion, GraphRunID: value.GraphRunID, GraphID: value.GraphID,
		SourceSnapshotSHA256: value.SourceSnapshotSHA256, GraphManifestSHA256: value.GraphManifestSHA256,
		CorePlanSHA256: value.CorePlanSHA256, LastEventSeq: value.LastEventSeq,
		LastEventSHA256: value.LastEventSHA256, ExecutionContractPresent: value.ExecutionContractPresent,
		DispatchAuthorityReleased: value.DispatchAuthorityReleased, Plan: value.Plan, Manifest: value.Manifest,
	}
	value.SnapshotSHA256 = mustDigestFixture(t, "forge.group-agent-graph-control-snapshot.v1\x00", payload)
	if err := graphdispatch.ValidateControlSnapshot(value); err != nil {
		t.Fatalf("validate fixture snapshot: %v", err)
	}
	return value
}

func buildProviderBodyFixture(t *testing.T, contract graphdispatch.NodeExecutionContract) []byte {
	t.Helper()
	return mustCanonicalFixture(t, providerRequestFixture{
		Include:      []string{"reasoning.encrypted_content"},
		Input:        []providerInputFixture{{Content: contract.Request.UserPrompt, Role: "user", Type: "message"}},
		Instructions: contract.Request.SystemPrompt, MaxOutputTokens: contract.Budgets.MaxOutputTokens,
		Model: contract.Provider.Model, Store: false, Stream: true, Tools: []string{},
	})
}

func buildDispatchRecordFixture(t *testing.T, contract graphdispatch.NodeExecutionContract, body, admission []byte) graphrelease.NodeDispatchRequestRecord {
	t.Helper()
	destination := mustDigestFixture(t, "forge.group-agent-node-destination.v1\x00", destinationPayloadFixture{
		V: 1, ProviderKind: contract.Provider.Kind, Endpoint: contract.Provider.Endpoint, Model: contract.Provider.Model,
	})
	value := graphrelease.NodeDispatchRequestRecord{
		V: 1, GraphRunID: contract.GraphRunID, ContractID: contract.ContractID,
		NodeID: contract.Node.NodeID, Attempt: 1, ContractSHA256: contract.ContractSHA256,
		RequestSHA256: contract.Request.RequestSHA256, ProjectLaneSHA256: contract.Node.ProjectLaneSHA256,
		Provider: contract.Provider.Kind, Endpoint: contract.Provider.Endpoint, Model: contract.Provider.Model,
		PricingSnapshotSHA256: contract.Budgets.PricingSnapshotSHA256,
		ProviderRequestSHA256: rawDomainDigest("forge.group-agent-node-provider-request.v1\x00", body),
		ProviderRequestBytes:  uint64(len(body)), DestinationSHA256: destination,
		CodecProtocolVersion: 1, ExpectedLastEventSeq: 2,
		ExpectedLastEventSHA256: rawDomainDigest(controlEventDomain, admission), CreatedAtMS: 90,
	}
	payload := dispatchPayloadFixture{
		V: value.V, CodecProtocolVersion: value.CodecProtocolVersion, GraphRunID: value.GraphRunID,
		ContractID: value.ContractID, ContractSHA256: value.ContractSHA256,
		ExpectedLastEventSeq: value.ExpectedLastEventSeq, ExpectedLastEventSHA256: value.ExpectedLastEventSHA256,
		NodeID: value.NodeID, Attempt: value.Attempt, ProjectLaneSHA256: value.ProjectLaneSHA256,
		ProviderKind: value.Provider, Endpoint: value.Endpoint, Model: value.Model,
		DestinationSHA256: value.DestinationSHA256, LogicalRequestSHA256: value.RequestSHA256,
		PricingSnapshotSHA256: value.PricingSnapshotSHA256, RequestBodyBytes: value.ProviderRequestBytes,
		RequestBodySHA256: value.ProviderRequestSHA256,
	}
	value.DispatchRequestSHA256 = mustDigestFixture(t, "forge.group-agent-node-dispatch-request.v1\x00", payload)
	value.DispatchRequestID = "node-dispatch-request-" + value.DispatchRequestSHA256
	return value
}

func buildDispatchEventFixture(t *testing.T, value graphrelease.NodeDispatchRequestRecord, previous []byte) []byte {
	t.Helper()
	event := dispatchEventFixture{
		V: 3, GraphRunID: value.GraphRunID, Seq: 3, Type: "node_dispatch_request_prepared",
		PreviousEventSHA256: rawDomainDigest(controlEventDomain, previous),
		ContractID:          value.ContractID, ContractSHA256: value.ContractSHA256,
		DispatchRequestID: value.DispatchRequestID, DispatchRequestSHA256: value.DispatchRequestSHA256,
		RequestBodySHA256: value.ProviderRequestSHA256, RequestBodyBytes: value.ProviderRequestBytes,
		LogicalRequestSHA256: value.RequestSHA256, NodeID: value.NodeID, Attempt: value.Attempt,
		ProjectLaneSHA256: value.ProjectLaneSHA256, CodecProtocolVersion: value.CodecProtocolVersion,
		ProviderKind: value.Provider, DestinationSHA256: value.DestinationSHA256,
		PricingSnapshotSHA256: value.PricingSnapshotSHA256, PreparedAtMS: value.CreatedAtMS,
	}
	return mustCanonicalFixture(t, event)
}

func releaseSHAForFixture(t *testing.T, value graphrelease.ReleaseControl) string {
	t.Helper()
	payload := releasePayloadFixture{
		V: value.V, SchedulerProtocolVersion: value.SchedulerProtocolVersion,
		ReleaseControlProtocolVersion: value.ReleaseControlProtocolVersion,
		GraphRun:                      value.GraphRun, Plan: value.Plan, Manifest: value.Manifest,
		JournalEvents: value.JournalEvents, ContractRecord: value.ContractRecord,
		Contract: value.Contract, DispatchRequest: value.DispatchRequest,
		ProviderRequestJSON: value.ProviderRequestJSON,
	}
	return mustDigestFixture(t, "forge.group-agent-node-dispatch-release-control.v1\x00", payload)
}
