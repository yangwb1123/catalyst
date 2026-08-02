package graphterminal

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"

	"forgeos/forge-core/internal/graphdispatch"
	"forgeos/forge-core/internal/graphplan"
	"forgeos/forge-core/internal/graphpricing"
	"forgeos/forge-core/internal/graphrelease"
)

type terminalControlPayload struct {
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
}

type artifactPayload struct {
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
}

type receiptPayload struct {
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
}

func canonicalBytes(value any) ([]byte, error) {
	var buffer bytes.Buffer
	encoder := json.NewEncoder(&buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(value); err != nil {
		return nil, errInvalidControl
	}
	encoded := buffer.Bytes()
	if len(encoded) == 0 || encoded[len(encoded)-1] != '\n' {
		return nil, errInvalidControl
	}
	return append([]byte(nil), encoded[:len(encoded)-1]...), nil
}

func rawDomainDigest(domain string, data []byte) string {
	digest := sha256.New()
	_, _ = digest.Write([]byte(domain))
	_, _ = digest.Write(data)
	return hex.EncodeToString(digest.Sum(nil))
}

func domainDigest(domain string, value any) (string, error) {
	encoded, err := canonicalBytes(value)
	if err != nil {
		return "", errInvalidControl
	}
	return rawDomainDigest(domain, encoded), nil
}

func controlPayload(value TerminalControl) terminalControlPayload {
	return terminalControlPayload{
		V: value.V, SchedulerProtocolVersion: value.SchedulerProtocolVersion,
		TerminalControlProtocolVersion: value.TerminalControlProtocolVersion,
		GraphRun:                       value.GraphRun, Plan: value.Plan, Manifest: value.Manifest,
		JournalEvents: value.JournalEvents, ContractRecord: value.ContractRecord,
		Contract: value.Contract, DispatchRequest: value.DispatchRequest,
		ProviderRequestJSON: value.ProviderRequestJSON, Authorization: value.Authorization,
		Pricing: value.Pricing, ActiveLane: value.ActiveLane, Claim: value.Claim,
		Artifact: value.Artifact,
	}
}

func payloadFromArtifact(value TerminalArtifact) artifactPayload {
	return artifactPayload{
		V: value.V, TerminalArtifactProtocolVersion: value.TerminalArtifactProtocolVersion,
		ArtifactKind: value.ArtifactKind, GraphRunID: value.GraphRunID, NodeID: value.NodeID,
		Attempt: value.Attempt, DispatchID: value.DispatchID, ClaimEventSHA256: value.ClaimEventSHA256,
		AuthorizationSHA256: value.AuthorizationSHA256, DispatchRequestSHA256: value.DispatchRequestSHA256,
		LogicalRequestSHA256: value.LogicalRequestSHA256, RequestBodySHA256: value.RequestBodySHA256,
		PricingSnapshotSHA256: value.PricingSnapshotSHA256, LaneOwnershipID: value.LaneOwnershipID,
		ProjectLaneSHA256: value.ProjectLaneSHA256, ProviderPollStarted: value.ProviderPollStarted,
		TerminalSeen: value.TerminalSeen, StreamEOFSeen: value.StreamEOFSeen,
		Classification: value.Classification, OutputText: value.OutputText, OutputBytes: value.OutputBytes,
		OutputSHA256: value.OutputSHA256, UsageObserved: value.UsageObserved, InputTokens: value.InputTokens,
		OutputTokens: value.OutputTokens, ActualCostCalculated: value.ActualCostCalculated,
		ActualCostUSDMicros: value.ActualCostUSDMicros, RetryAuthorized: value.RetryAuthorized,
		CreatedAtMS: value.CreatedAtMS,
	}
}
