package graphscheduledcontract

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/graphdispatch"
)

type sourceFixture struct {
	V     uint16 `json:"v"`
	Input struct {
		CanonicalControlSnapshotJSON string         `json:"canonical_control_snapshot_json"`
		ExecutionOptions             fixtureOptions `json:"execution_options"`
	} `json:"input"`
}

type scheduleFixture struct {
	V              uint16 `json:"v"`
	ScheduleSHA256 string `json:"schedule_sha256"`
}

type scheduledContractFixture struct {
	V               uint16          `json:"v"`
	ControlFixture  string          `json:"control_fixture"`
	ScheduleFixture string          `json:"schedule_fixture"`
	Input           candidateInput  `json:"input"`
	Expected        candidateGolden `json:"expected"`
}

type candidateInput struct {
	CanonicalControlSnapshotJSON string         `json:"canonical_control_snapshot_json"`
	ScheduleSHA256               string         `json:"schedule_sha256"`
	ExecutionOptions             fixtureOptions `json:"execution_options"`
}

type candidateGolden struct {
	SelectedNodeID               string `json:"selected_node_id"`
	CanonicalUserPromptJSON      string `json:"canonical_user_prompt_json"`
	CanonicalRequestPayloadJSON  string `json:"canonical_request_payload_json"`
	RequestSHA256                string `json:"request_sha256"`
	RequestID                    string `json:"request_id"`
	CanonicalRequestJSON         string `json:"canonical_request_json"`
	CanonicalContractPayloadJSON string `json:"canonical_contract_payload_json"`
	ContractSHA256               string `json:"contract_sha256"`
	ContractID                   string `json:"contract_id"`
	CanonicalContractJSON        string `json:"canonical_contract_json"`
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

func fixturePath(name string) string {
	return filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", name)
}

func readSourceFixture(t *testing.T) sourceFixture {
	t.Helper()
	var fixture sourceFixture
	readFixture(t, "group-agent-node-execution-contract-v1.json", &fixture)
	if fixture.V != 1 {
		t.Fatalf("source fixture version = %d", fixture.V)
	}
	return fixture
}

func readScheduleFixture(t *testing.T) scheduleFixture {
	t.Helper()
	var fixture scheduleFixture
	readFixture(t, "group-agent-graph-execution-schedule-v1.json", &fixture)
	if fixture.V != 1 {
		t.Fatalf("schedule fixture version = %d", fixture.V)
	}
	return fixture
}

func readCandidateFixture(t *testing.T) scheduledContractFixture {
	t.Helper()
	var fixture scheduledContractFixture
	readFixture(t, "group-agent-scheduled-node-contract-v2.json", &fixture)
	if fixture.V != 2 {
		t.Fatalf("candidate fixture version = %d", fixture.V)
	}
	return fixture
}

func readFixture(t *testing.T, name string, destination any) {
	t.Helper()
	data, err := os.ReadFile(fixturePath(name))
	if err != nil {
		t.Fatalf("read fixture %s: %v", name, err)
	}
	if err := json.Unmarshal(data, destination); err != nil {
		t.Fatalf("decode fixture %s: %v", name, err)
	}
}

func fixtureSnapshot(t *testing.T) graphdispatch.ControlSnapshot {
	t.Helper()
	source := readSourceFixture(t)
	snapshot, err := graphdispatch.DecodeControl(strings.NewReader(source.Input.CanonicalControlSnapshotJSON))
	if err != nil {
		t.Fatalf("decode source control: %v", err)
	}
	return snapshot
}

func (value fixtureOptions) options() graphdispatch.ExecutionOptions {
	return graphdispatch.ExecutionOptions{
		Endpoint: value.Endpoint, Model: value.Model, MaxOutputTokens: value.MaxOutputTokens,
		MaxModelOutputBytes: value.MaxModelOutputBytes, MaxModelEvents: value.MaxModelEvents,
		TimeoutMilliseconds: value.TimeoutMilliseconds, MaxCostUSDMicros: value.MaxCostUSDMicros,
		PricingSnapshotSHA256: value.PricingSnapshotSHA256, MaxResultBytes: value.MaxResultBytes,
	}
}

func mustCandidate(t *testing.T) ScheduledNodeContractCandidate {
	t.Helper()
	value, err := BuildInitial(
		fixtureSnapshot(t), readScheduleFixture(t).ScheduleSHA256,
		readSourceFixture(t).Input.ExecutionOptions.options(),
	)
	if err != nil {
		t.Fatalf("BuildInitial: %v", err)
	}
	return value
}

func TestEmitSharedScheduledNodeContractFixture(t *testing.T) {
	if os.Getenv("FORGE_EMIT_SCHEDULED_NODE_CONTRACT_FIXTURE") != "1" {
		t.Skip("fixture emission is opt-in")
	}
	source, schedule, value := readSourceFixture(t), readScheduleFixture(t), mustCandidate(t)
	fixture := buildSharedFixture(t, source, schedule, value)
	encoded, err := json.MarshalIndent(fixture, "", "  ")
	if err != nil {
		t.Fatalf("encode shared fixture: %v", err)
	}
	fmt.Printf("FIXTURE_BEGIN\n%s\nFIXTURE_END\n", encoded)
}

func buildSharedFixture(
	t *testing.T,
	source sourceFixture,
	schedule scheduleFixture,
	value ScheduledNodeContractCandidate,
) scheduledContractFixture {
	t.Helper()
	requestPayloadJSON, err := canonicalBytes(requestPayloadFrom(value.Request))
	if err != nil {
		t.Fatalf("request payload: %v", err)
	}
	contractPayloadJSON, err := canonicalBytes(candidatePayloadFrom(value))
	if err != nil {
		t.Fatalf("contract payload: %v", err)
	}
	contractJSON, err := MarshalCandidate(value)
	if err != nil {
		t.Fatalf("candidate JSON: %v", err)
	}
	requestJSON, err := canonicalBytes(value.Request)
	if err != nil {
		t.Fatalf("request JSON: %v", err)
	}
	return scheduledContractFixture{
		V: 2, ControlFixture: "group-agent-node-execution-contract-v1.json",
		ScheduleFixture: "group-agent-graph-execution-schedule-v1.json",
		Input: candidateInput{
			CanonicalControlSnapshotJSON: source.Input.CanonicalControlSnapshotJSON,
			ScheduleSHA256:               schedule.ScheduleSHA256, ExecutionOptions: source.Input.ExecutionOptions,
		},
		Expected: candidateGolden{
			SelectedNodeID: value.Node.NodeID, CanonicalUserPromptJSON: value.Request.UserPrompt,
			CanonicalRequestPayloadJSON: string(requestPayloadJSON), RequestSHA256: value.Request.RequestSHA256,
			RequestID: value.Request.RequestID, CanonicalRequestJSON: string(requestJSON),
			CanonicalContractPayloadJSON: string(contractPayloadJSON), ContractSHA256: value.ContractSHA256,
			ContractID: value.ContractID, CanonicalContractJSON: string(contractJSON),
		},
	}
}
