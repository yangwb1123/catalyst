package graphterminal

import (
	"encoding/json"
	"testing"

	"forgeos/forge-core/internal/graphdispatch"
	"forgeos/forge-core/internal/graphplan"
	"forgeos/forge-core/internal/graphpricing"
	"forgeos/forge-core/internal/graphrelease"
)

type releaseFixture struct {
	Control       graphrelease.ReleaseControl
	Authorization graphrelease.Authorization
	Pricing       graphpricing.Snapshot
}

type manifestDigestFixture struct {
	Edges   []graphplan.Edge      `json:"edges"`
	Manager graphplan.Manager     `json:"manager"`
	Nodes   []manifestNodeFixture `json:"nodes"`
	Source  manifestSourceFixture `json:"source"`
	V       uint16                `json:"v"`
	Waves   [][]string            `json:"waves"`
}

type manifestNodeFixture struct {
	Acceptance   string `json:"acceptance"`
	AgentProfile string `json:"agent_profile"`
	MemberRole   string `json:"member_role"`
	NodeID       string `json:"node_id"`
	ProjectID    string `json:"project_id"`
	Task         string `json:"task"`
}

type manifestSourceFixture struct {
	ContextSliceSHA256 string `json:"context_slice_sha256"`
	ContextVersion     uint16 `json:"context_version"`
	GroupID            string `json:"group_id"`
	GroupRunID         string `json:"group_run_id"`
	GroupRunVersion    uint16 `json:"group_run_version"`
	SnapshotBytes      uint64 `json:"snapshot_bytes"`
	SnapshotSHA256     string `json:"snapshot_sha256"`
}

type preparedEventFixture struct {
	V                        uint16 `json:"v"`
	GraphRunID               string `json:"graph_run_id"`
	Seq                      uint64 `json:"seq"`
	Type                     string `json:"type"`
	GraphID                  string `json:"graph_id"`
	GraphManifestSHA256      string `json:"graph_manifest_sha256"`
	PlanSHA256               string `json:"plan_sha256"`
	SchedulerProtocolVersion uint16 `json:"scheduler_protocol_version"`
	PreparedAtMS             uint64 `json:"prepared_at_ms"`
}

type contractEventFixture struct {
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

type dispatchEventFixture struct {
	V                     uint16 `json:"v"`
	GraphRunID            string `json:"graph_run_id"`
	Seq                   uint64 `json:"seq"`
	Type                  string `json:"type"`
	PreviousEventSHA256   string `json:"previous_event_sha256"`
	ContractID            string `json:"contract_id"`
	ContractSHA256        string `json:"contract_sha256"`
	DispatchRequestID     string `json:"dispatch_request_id"`
	DispatchRequestSHA256 string `json:"dispatch_request_sha256"`
	RequestBodySHA256     string `json:"request_body_sha256"`
	RequestBodyBytes      uint64 `json:"request_body_bytes"`
	LogicalRequestSHA256  string `json:"logical_request_sha256"`
	NodeID                string `json:"node_id"`
	Attempt               uint16 `json:"attempt"`
	ProjectLaneSHA256     string `json:"project_lane_sha256"`
	CodecProtocolVersion  uint16 `json:"codec_protocol_version"`
	ProviderKind          string `json:"provider_kind"`
	DestinationSHA256     string `json:"destination_sha256"`
	PricingSnapshotSHA256 string `json:"pricing_snapshot_sha256"`
	PreparedAtMS          uint64 `json:"prepared_at_ms"`
}

func buildReleaseFixture(t *testing.T) releaseFixture {
	t.Helper()
	manifest := singleNodeManifest()
	manifestSHA := manifestSHAForFixture(t, manifest)
	plan, err := graphplan.Build(singleNodeSpec(manifest), "graph-terminal-fixture", manifestSHA)
	if err != nil {
		t.Fatalf("build plan: %v", err)
	}
	manifest.Waves = plan.Waves
	prepared := preparedEventFixture{
		V: 1, GraphRunID: "graph-run-terminal-fixture", Seq: 1, Type: "graph_run_prepared",
		GraphID: plan.GraphID, GraphManifestSHA256: manifestSHA, PlanSHA256: plan.PlanSHA256,
		SchedulerProtocolVersion: 1, PreparedAtMS: 73,
	}
	preparedJSON := mustCanonicalFixture(t, prepared)
	snapshot := buildControlSnapshot(t, plan, manifest, preparedJSON)
	pricing := buildPricingFixture(t)
	contract := buildContractFixture(t, snapshot, pricing)
	release := buildReleaseControlFixture(t, snapshot, contract, preparedJSON)
	authorization, err := graphrelease.BuildAuthorization(release)
	if err != nil {
		t.Fatalf("build authorization: %v", err)
	}
	return releaseFixture{release, authorization, pricing}
}

func singleNodeManifest() graphdispatch.GraphManifest {
	return graphdispatch.GraphManifest{
		V: 1,
		Source: graphdispatch.GraphSource{
			GroupRunVersion: 1, GroupRunID: "group-run-terminal-fixture",
			GroupID: "group-terminal-fixture", ContextVersion: 1,
			ContextSliceSHA256: repeatedDigest('1'), SnapshotSHA256: repeatedDigest('2'), SnapshotBytes: 128,
		},
		Manager: graphplan.Manager{AgentProfile: "manager", Instruction: "Return the frozen result."},
		Nodes: []graphplan.Node{{
			NodeID: "node-terminal-fixture", ProjectID: "project-terminal-fixture",
			MemberRole: "implementer", AgentProfile: "worker", Task: "Produce done.", Acceptance: "Output is done.",
		}}, Edges: []graphplan.Edge{}, Waves: [][]string{{"node-terminal-fixture"}},
	}
}

func singleNodeSpec(manifest graphdispatch.GraphManifest) graphplan.Spec {
	return graphplan.Spec{V: 1, Manager: manifest.Manager, Nodes: manifest.Nodes, Edges: manifest.Edges}
}

func manifestSHAForFixture(t *testing.T, manifest graphdispatch.GraphManifest) string {
	t.Helper()
	node := manifest.Nodes[0]
	view := manifestDigestFixture{
		Edges: manifest.Edges, Manager: manifest.Manager,
		Nodes: []manifestNodeFixture{{node.Acceptance, node.AgentProfile, node.MemberRole, node.NodeID, node.ProjectID, node.Task}},
		Source: manifestSourceFixture{
			manifest.Source.ContextSliceSHA256, manifest.Source.ContextVersion,
			manifest.Source.GroupID, manifest.Source.GroupRunID, manifest.Source.GroupRunVersion,
			manifest.Source.SnapshotBytes, manifest.Source.SnapshotSHA256,
		}, V: manifest.V, Waves: manifest.Waves,
	}
	return mustDigestFixture(t, "forge.group-agent-graph-manifest.v1\x00", view)
}

func buildPricingFixture(t *testing.T) graphpricing.Snapshot {
	t.Helper()
	value, err := graphpricing.Build(graphpricing.Input{
		Model: "gpt-5-mini", InputUSDMicrosPerTokenUnit: 1_000,
		OutputUSDMicrosPerTokenUnit: 2_000, MaxInputTokens: 1_000,
	})
	if err != nil {
		t.Fatalf("build pricing: %v", err)
	}
	return value
}

func buildContractFixture(t *testing.T, snapshot graphdispatch.ControlSnapshot, pricing graphpricing.Snapshot) graphdispatch.NodeExecutionContract {
	t.Helper()
	value, err := graphdispatch.Build(snapshot, graphdispatch.ExecutionOptions{
		Endpoint: graphpricing.RegisteredEndpoint, Model: pricing.Model, MaxOutputTokens: 100,
		MaxModelOutputBytes: 1_024, MaxModelEvents: 64, TimeoutMilliseconds: 10_000,
		MaxCostUSDMicros: 100, PricingSnapshotSHA256: pricing.PricingSnapshotSHA256, MaxResultBytes: 2_048,
	})
	if err != nil {
		t.Fatalf("build contract: %v", err)
	}
	return value
}

func buildReleaseControlFixture(t *testing.T, snapshot graphdispatch.ControlSnapshot, contract graphdispatch.NodeExecutionContract, prepared []byte) graphrelease.ReleaseControl {
	t.Helper()
	contractJSON, err := graphdispatch.MarshalContract(contract)
	if err != nil {
		t.Fatalf("marshal contract: %v", err)
	}
	second := buildAdmissionFixture(t, contract, contractJSON, prepared)
	body := buildProviderBodyFixture(t, contract)
	dispatch := buildDispatchRecordFixture(t, contract, body, second)
	third := buildDispatchEventFixture(t, dispatch, second)
	planJSON, _ := graphplan.MarshalPlan(snapshot.Plan)
	control := graphrelease.ReleaseControl{
		V: 1, SchedulerProtocolVersion: 1, ReleaseControlProtocolVersion: 1,
		GraphRun: graphrelease.GraphRunRecord{
			V: 3, GraphRunID: snapshot.GraphRunID, GraphID: snapshot.GraphID,
			Status: "awaiting_dispatch_authorization", SourceSnapshotSHA256: snapshot.SourceSnapshotSHA256,
			GraphManifestSHA256: snapshot.GraphManifestSHA256, SchedulerProtocolVersion: 1,
			PlanSHA256: snapshot.CorePlanSHA256, PlanBytes: uint64(len(planJSON)), NodeCount: 1, WaveCount: 1,
			ExecutionContractPresent: true, DispatchRequestPresent: true, DispatchAuthorityReleased: false,
			LastEventSeq: 3, JournalBytes: uint64(len(prepared) + len(second) + len(third)), CreatedAtMS: 73,
		},
		Plan: snapshot.Plan, Manifest: snapshot.Manifest, JournalEvents: []json.RawMessage{prepared, second, third},
		ContractRecord: contractRecordFixture(contract, uint64(len(contractJSON))), Contract: contract,
		DispatchRequest: dispatch, ProviderRequestJSON: string(body),
	}
	control.SnapshotSHA256 = releaseSHAForFixture(t, control)
	return control
}

func buildAdmissionFixture(t *testing.T, contract graphdispatch.NodeExecutionContract, contractJSON, previous []byte) []byte {
	t.Helper()
	event := contractEventFixture{
		V: 2, GraphRunID: contract.GraphRunID, Seq: 2, Type: "node_execution_contract_admitted",
		PreviousEventSHA256:   rawDomainDigest(preparedEventDomain, previous),
		ControlSnapshotSHA256: contract.ControlSnapshotSHA256, ContractID: contract.ContractID,
		ContractSHA256: contract.ContractSHA256, ContractBytes: uint64(len(contractJSON)),
		NodeID: contract.Node.NodeID, Attempt: 1, RequestSHA256: contract.Request.RequestSHA256,
		ProjectLaneSHA256: contract.Node.ProjectLaneSHA256, AdmittedAtMS: 80,
	}
	return mustCanonicalFixture(t, event)
}

func contractRecordFixture(contract graphdispatch.NodeExecutionContract, bytes uint64) graphrelease.NodeExecutionContractRecord {
	return graphrelease.NodeExecutionContractRecord{
		V: 1, ContractID: contract.ContractID, GraphRunID: contract.GraphRunID,
		NodeID: contract.Node.NodeID, Attempt: 1, ControlSnapshotSHA256: contract.ControlSnapshotSHA256,
		ContractSHA256: contract.ContractSHA256, ContractBytes: bytes,
		RequestSHA256: contract.Request.RequestSHA256, ProjectLaneSHA256: contract.Node.ProjectLaneSHA256,
		ExpectedLastEventSeq: 1, ExpectedLastEventSHA256: contract.ExpectedLastEventSHA256, CreatedAtMS: 80,
	}
}

func repeatedDigest(character byte) string {
	data := make([]byte, 64)
	for index := range data {
		data[index] = character
	}
	return string(data)
}

func mustCanonicalFixture(t *testing.T, value any) []byte {
	t.Helper()
	encoded, err := canonicalBytes(value)
	if err != nil {
		t.Fatalf("canonical fixture: %v", err)
	}
	return encoded
}

func mustDigestFixture(t *testing.T, domain string, value any) string {
	t.Helper()
	digest, err := domainDigest(domain, value)
	if err != nil {
		t.Fatalf("digest fixture: %v", err)
	}
	return digest
}
