package graphterminal

// nodeDispatchReleasedEvent is the exact canonical seq-4 claim event.
type nodeDispatchReleasedEvent struct {
	V                      uint16 `json:"v"`
	GraphRunID             string `json:"graph_run_id"`
	Seq                    uint64 `json:"seq"`
	Type                   string `json:"type"`
	PreviousEventSHA256    string `json:"previous_event_sha256"`
	DispatchID             string `json:"dispatch_id"`
	AuthorizationID        string `json:"authorization_id"`
	AuthorizationSHA256    string `json:"authorization_sha256"`
	DispatchRequestID      string `json:"dispatch_request_id"`
	DispatchRequestSHA256  string `json:"dispatch_request_sha256"`
	LogicalRequestSHA256   string `json:"logical_request_sha256"`
	RequestBodySHA256      string `json:"request_body_sha256"`
	RequestBodyBytes       uint64 `json:"request_body_bytes"`
	PricingSnapshotSHA256  string `json:"pricing_snapshot_sha256"`
	NodeID                 string `json:"node_id"`
	Attempt                uint16 `json:"attempt"`
	MaxCostUSDMicros       uint64 `json:"max_cost_usd_micros"`
	ConsentContractVersion uint16 `json:"consent_contract_version"`
	LaneOwnershipID        string `json:"lane_ownership_id"`
	ProjectLaneSHA256      string `json:"project_lane_sha256"`
	ReleasedAtMS           uint64 `json:"released_at_ms"`
}

type nodeLifecycleTerminalizedEvent struct {
	V                     uint16 `json:"v"`
	GraphRunID            string `json:"graph_run_id"`
	Seq                   uint64 `json:"seq"`
	Type                  string `json:"type"`
	PreviousEventSHA256   string `json:"previous_event_sha256"`
	DispatchID            string `json:"dispatch_id"`
	LaneOwnershipID       string `json:"lane_ownership_id"`
	ProjectLaneSHA256     string `json:"project_lane_sha256"`
	ArtifactID            string `json:"artifact_id"`
	ArtifactSHA256        string `json:"artifact_sha256"`
	TerminalReceiptID     string `json:"terminal_receipt_id"`
	TerminalReceiptSHA256 string `json:"terminal_receipt_sha256"`
	NodeID                string `json:"node_id"`
	Attempt               uint16 `json:"attempt"`
	NodeOutcome           string `json:"node_outcome"`
	WaveIndex             uint16 `json:"wave_index"`
	WaveOutcome           string `json:"wave_outcome"`
	GraphStatus           string `json:"graph_status"`
	RetryAuthorized       bool   `json:"retry_authorized"`
	LaneReleased          bool   `json:"lane_released"`
	TerminalizedAtMS      uint64 `json:"terminalized_at_ms"`
}

func eventFromClaim(claim Claim) nodeDispatchReleasedEvent {
	return nodeDispatchReleasedEvent{
		V: 4, GraphRunID: claim.GraphRunID, Seq: 4, Type: "node_dispatch_released",
		PreviousEventSHA256: claim.ExpectedLastEventSHA256, DispatchID: claim.DispatchID,
		AuthorizationID: claim.AuthorizationID, AuthorizationSHA256: claim.AuthorizationSHA256,
		DispatchRequestID:     claim.DispatchRequestID,
		DispatchRequestSHA256: claim.DispatchRequestSHA256,
		LogicalRequestSHA256:  claim.LogicalRequestSHA256,
		RequestBodySHA256:     claim.RequestBodySHA256, RequestBodyBytes: claim.RequestBodyBytes,
		PricingSnapshotSHA256: claim.PricingSnapshotSHA256, NodeID: claim.NodeID,
		Attempt: claim.Attempt, MaxCostUSDMicros: claim.MaxCostUSDMicros,
		ConsentContractVersion: claim.ConsentContractVersion,
		LaneOwnershipID:        claim.LaneOwnershipID, ProjectLaneSHA256: claim.ProjectLaneSHA256,
		ReleasedAtMS: claim.ReleasedAtMS,
	}
}
