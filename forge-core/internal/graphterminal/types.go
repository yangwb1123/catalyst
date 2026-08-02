// Package graphterminal validates one claimed, single-node Graph terminal
// control and emits the sole passive scheduler receipt. It performs no effect.
package graphterminal

import (
	"encoding/json"

	"forgeos/forge-core/internal/graphdispatch"
	"forgeos/forge-core/internal/graphplan"
	"forgeos/forge-core/internal/graphpricing"
	"forgeos/forge-core/internal/graphrelease"
)

const (
	TerminalControlVersion   uint16 = 1
	TerminalControlProtocol  uint16 = 1
	TerminalArtifactVersion  uint16 = 1
	TerminalArtifactProtocol uint16 = 1
	TerminalReceiptVersion   uint16 = 1
	TerminalReceiptProtocol  uint16 = 1
	ClaimVersion             uint16 = 1
	ActiveLaneVersion        uint16 = 1
	MaxTerminalControlBytes         = 64 * 1024 * 1024
	maxArtifactPayloadBytes         = 1024 * 1024
	maxReceiptBytes                 = 64 * 1024
	maxEventBytes                   = 64 * 1024
)

const (
	controlDigestDomain  = "forge.group-agent-node-terminal-control.v1\x00"
	artifactDigestDomain = "forge.group-agent-node-terminal-artifact.v1\x00"
	outputDigestDomain   = "forge.group-agent-node-terminal-output.v1\x00"
	receiptDigestDomain  = "forge.group-agent-node-terminal-receipt.v1\x00"
	preparedEventDomain  = "forge.group-agent-graph-run-event.v1\x00"
	controlEventDomain   = "forge.group-agent-graph-run-control-event.v1\x00"
)

// TerminalControl is Rust's private, canonical, fully claimed terminal export.
type TerminalControl struct {
	V                              uint16                                   `json:"v"`
	SchedulerProtocolVersion       uint16                                   `json:"scheduler_protocol_version"`
	TerminalControlProtocolVersion uint16                                   `json:"terminal_control_protocol_version"`
	GraphRun                       graphrelease.GraphRunRecord              `json:"graph_run"`
	Plan                           graphplan.Plan                           `json:"plan"`
	Manifest                       graphdispatch.GraphManifest              `json:"manifest"`
	JournalEvents                  []json.RawMessage                        `json:"journal_events"`
	ContractRecord                 graphrelease.NodeExecutionContractRecord `json:"contract_record"`
	Contract                       graphdispatch.NodeExecutionContract      `json:"contract"`
	DispatchRequest                graphrelease.NodeDispatchRequestRecord   `json:"dispatch_request"`
	ProviderRequestJSON            string                                   `json:"provider_request_json"`
	Authorization                  graphrelease.Authorization               `json:"authorization"`
	Pricing                        graphpricing.Snapshot                    `json:"pricing"`
	ActiveLane                     ActiveLane                               `json:"active_lane"`
	Claim                          Claim                                    `json:"claim"`
	Artifact                       TerminalArtifact                         `json:"artifact"`
	SnapshotSHA256                 string                                   `json:"snapshot_sha256"`
}

// Claim is the immutable successful seq-3/head lane claim.
type Claim struct {
	V                       uint16 `json:"v"`
	GraphRunID              string `json:"graph_run_id"`
	DispatchID              string `json:"dispatch_id"`
	AuthorizationID         string `json:"authorization_id"`
	AuthorizationSHA256     string `json:"authorization_sha256"`
	DispatchRequestID       string `json:"dispatch_request_id"`
	DispatchRequestSHA256   string `json:"dispatch_request_sha256"`
	LogicalRequestSHA256    string `json:"logical_request_sha256"`
	RequestBodySHA256       string `json:"request_body_sha256"`
	RequestBodyBytes        uint64 `json:"request_body_bytes"`
	PricingSnapshotSHA256   string `json:"pricing_snapshot_sha256"`
	NodeID                  string `json:"node_id"`
	Attempt                 uint16 `json:"attempt"`
	MaxCostUSDMicros        uint64 `json:"max_cost_usd_micros"`
	ConsentContractVersion  uint16 `json:"consent_contract_version"`
	LaneOwnershipID         string `json:"lane_ownership_id"`
	ProjectLaneSHA256       string `json:"project_lane_sha256"`
	ExpectedLastEventSeq    uint64 `json:"expected_last_event_seq"`
	ExpectedLastEventSHA256 string `json:"expected_last_event_sha256"`
	ClaimEventSHA256        string `json:"claim_event_sha256"`
	ReleasedAtMS            uint64 `json:"released_at_ms"`
}

// ActiveLane is the exact still-held global Project lane row.
type ActiveLane struct {
	V                 uint16 `json:"v"`
	ProjectLaneSHA256 string `json:"project_lane_sha256"`
	LaneOwnershipID   string `json:"lane_ownership_id"`
	GraphRunID        string `json:"graph_run_id"`
	NodeID            string `json:"node_id"`
	Attempt           uint16 `json:"attempt"`
	DispatchID        string `json:"dispatch_id"`
	ClaimEventSHA256  string `json:"claim_event_sha256"`
	ClaimedAtMS       uint64 `json:"claimed_at_ms"`
}

// TerminalArtifact contains bounded provider evidence and no arbitrary error.
type TerminalArtifact struct {
	V                               uint16 `json:"v"`
	TerminalArtifactProtocolVersion uint16 `json:"terminal_artifact_protocol_version"`
	ArtifactKind                    string `json:"artifact_kind"`
	GraphRunID                      string `json:"graph_run_id"`
	NodeID                          string `json:"node_id"`
	Attempt                         uint16 `json:"attempt"`
	DispatchID                      string `json:"dispatch_id"`
	ClaimEventSHA256                string `json:"claim_event_sha256"`
	AuthorizationSHA256             string `json:"authorization_sha256"`
	DispatchRequestSHA256           string `json:"dispatch_request_sha256"`
	LogicalRequestSHA256            string `json:"logical_request_sha256"`
	RequestBodySHA256               string `json:"request_body_sha256"`
	PricingSnapshotSHA256           string `json:"pricing_snapshot_sha256"`
	LaneOwnershipID                 string `json:"lane_ownership_id"`
	ProjectLaneSHA256               string `json:"project_lane_sha256"`
	ProviderPollStarted             bool   `json:"provider_poll_started"`
	TerminalSeen                    bool   `json:"terminal_seen"`
	StreamEOFSeen                   bool   `json:"stream_eof_seen"`
	Classification                  string `json:"classification"`
	OutputText                      string `json:"output_text"`
	OutputBytes                     uint64 `json:"output_bytes"`
	OutputSHA256                    string `json:"output_sha256"`
	UsageObserved                   bool   `json:"usage_observed"`
	InputTokens                     uint64 `json:"input_tokens"`
	OutputTokens                    uint64 `json:"output_tokens"`
	ActualCostCalculated            bool   `json:"actual_cost_calculated"`
	ActualCostUSDMicros             uint64 `json:"actual_cost_usd_micros"`
	RetryAuthorized                 bool   `json:"retry_authorized"`
	CreatedAtMS                     uint64 `json:"created_at_ms"`
	ArtifactID                      string `json:"artifact_id"`
	ArtifactBytes                   uint64 `json:"artifact_bytes"`
	ArtifactSHA256                  string `json:"artifact_sha256"`
}

// Receipt is Core's content-addressed, effect-free terminal decision.
type Receipt struct {
	V                              uint16 `json:"v"`
	SchedulerProtocolVersion       uint16 `json:"scheduler_protocol_version"`
	TerminalReceiptProtocolVersion uint16 `json:"terminal_receipt_protocol_version"`
	TerminalControlSHA256          string `json:"terminal_control_sha256"`
	ExpectedLastEventSeq           uint64 `json:"expected_last_event_seq"`
	ExpectedLastEventSHA256        string `json:"expected_last_event_sha256"`
	GraphRunID                     string `json:"graph_run_id"`
	GraphID                        string `json:"graph_id"`
	NodeID                         string `json:"node_id"`
	Attempt                        uint16 `json:"attempt"`
	DispatchID                     string `json:"dispatch_id"`
	LaneOwnershipID                string `json:"lane_ownership_id"`
	ProjectLaneSHA256              string `json:"project_lane_sha256"`
	ArtifactKind                   string `json:"artifact_kind"`
	ArtifactID                     string `json:"artifact_id"`
	ArtifactSHA256                 string `json:"artifact_sha256"`
	NodeOutcome                    string `json:"node_outcome"`
	WaveIndex                      uint16 `json:"wave_index"`
	WaveOutcome                    string `json:"wave_outcome"`
	GraphStatus                    string `json:"graph_status"`
	RetryAuthorized                bool   `json:"retry_authorized"`
	LaneReleaseAuthorized          bool   `json:"lane_release_authorized"`
	ReceiptID                      string `json:"receipt_id"`
	ReceiptSHA256                  string `json:"receipt_sha256"`
}
